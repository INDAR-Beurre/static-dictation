//! Global hotkey listener: evdev (AltGr+L = dictation toggle, the ONLY shortcut) with stdin fallback.

use anyhow::Result;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HotkeyEvent {
    /// The one and only action this tool has.
    Toggle,
}

/// key codes (values from linux/input-event-codes.h, NOT X keycodes: X = kernel + 8)
const KEY_L: u16 = 38;
const KEY_RIGHTALT: u16 = 100; // AltGr on most layouts

const EV_KEY: u16 = 0x01;
const EV_SYN: u16 = 0x00;
const DEBOUNCE: Duration = Duration::from_millis(175);
/// an idle gap this long means a held meta key missed its keyup — stop trusting it
const MODIFIER_IDLE_RESET: Duration = Duration::from_secs(2);
/// bounded reopen attempts for transient device-read errors before the watcher exits
const REOPEN_ATTEMPTS: u32 = 5;
const REOPEN_SLEEP: Duration = Duration::from_millis(500);
/// raw errno values meaning the device node is permanently gone (unplug / removed)
const ENODEV: i32 = 19;
const ENOENT: i32 = 2;

// uinput / evdev ioctl request codes (values from linux/uinput.h, linux/input.h)
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;
const EVIOCGNAME_128: libc::c_ulong = 0x8080_4506;
const EVIOCGBIT_KEY_64: libc::c_ulong = 0x8040_4521;
const EVIOCGRAB: libc::c_ulong = 0x4004_4590;

/// device names we must never watch back: our own virtual keyboard and the test injector
const SELF_DEVICE_PREFIXES: [&str; 2] = ["static-dictation-kbd", "burst-inject"];

#[repr(C)]
#[allow(dead_code)] // layout documentation; events are parsed from bytes below
struct InputEvent {
    tv_sec: i64,
    tv_usec: i64,
    type_: u16,
    code: u16,
    value: i32,
}
const EVENT_SIZE: usize = 24; // two i64s + type/code/value
                              // shortcut: assumed fixed 24-byte native-endian input_event; newer time64 kernels use the same sizes
                              // type_, code, value live at offsets 16, 18, 20 regardless of the time encoding.

/// A minimal virtual keyboard on /dev/uinput, used to re-emit events we grabbed away.
/// All ioctls are fire-and-forget best-effort: if any setup step fails we mark the device
/// invalid and Drop skips UI_DEV_DESTROY (kernel cleans up an un-created fd on close anyway).
struct UinputKbd {
    file: std::fs::File,
    ok: bool,
}

#[repr(C)]
struct UinputSetup {
    id: [u16; 4], // bustype, vendor, product, version
    name: [u8; 80],
    ff_effects_max: u32,
}

impl UinputKbd {
    fn new() -> Option<Self> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .ok()?;
        let mut dev = UinputKbd { file, ok: true };
        // enable every key code 1..=248 so re-emitted events are always accepted
        for code in 1..=248u16 {
            dev.ioctl(UI_SET_KEYBIT, code as libc::c_ulong);
        }
        dev.ioctl(UI_SET_EVBIT, EV_KEY as libc::c_ulong);
        dev.ioctl(UI_SET_EVBIT, EV_SYN as libc::c_ulong);
        let name_bytes = b"static-dictation-kbd\0";
        let mut name = [0u8; 80];
        name[..name_bytes.len()].copy_from_slice(name_bytes);
        let setup = UinputSetup {
            id: [0x03, 0x1235, 0x5679, 1],
            name,
            ff_effects_max: 0,
        };
        dev.ok &= unsafe {
            libc::ioctl(
                dev.file.as_raw_fd(),
                UI_DEV_SETUP,
                &setup as *const UinputSetup,
            )
        } == 0;
        dev.ok &= unsafe { libc::ioctl(dev.file.as_raw_fd(), UI_DEV_CREATE) } == 0;
        if !dev.ok {
            return None;
        }
        Some(dev)
    }

    fn ioctl(&mut self, req: libc::c_ulong, arg: libc::c_ulong) {
        self.ok &= unsafe { libc::ioctl(self.file.as_raw_fd(), req, arg) } == 0;
    }

    /// Emit one raw event struct (as 24 bytes) followed by a SYN separator.
    fn emit(&mut self, event: &[u8; EVENT_SIZE]) {
        let mut syn = [0u8; EVENT_SIZE];
        syn[16..18].copy_from_slice(&EV_SYN.to_ne_bytes());
        if self.file.write_all(event).is_err() || self.file.write_all(&syn).is_err() {
            eprintln!("hk: uinput write failed");
        }
    }
}

impl Drop for UinputKbd {
    fn drop(&mut self) {
        if self.ok {
            unsafe { libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY) };
        }
    }
}

/// Keyboard state shared by ALL device watchers: a grab-and-re-emit chain
/// (keyd → handy clones → compositor) can deliver AltGr on one /dev/input node
/// and L on another; per-device state would split the combo. Shared also makes
/// the cross-node debounce collapse duplicated physical presses, and means a
/// modifier keyup seen by ANY watcher clears the hold for everyone.
struct KeyState {
    /// AltGr stays held until its own keyup, even after the chord fires — that
    /// is what stops a second L tap while AltGr is down from typing "l".
    modifier_held: bool,
    chord_fired: bool,
    /// True after AltGr+L fired until the matching AltGr release is swallowed.
    chord_consumed: bool,
    last_toggle: Option<Instant>,
    last_event: Instant, // any tracked stream activity, for the stale-hold safety
}

impl KeyState {
    fn new() -> Self {
        KeyState {
            modifier_held: false,
            chord_fired: false,
            chord_consumed: false,
            last_toggle: None,
            last_event: Instant::now(),
        }
    }
}

/// Build a synthetic EV_KEY event. Used to cancel the AltGr press we already
/// re-emitted, so the chord leaves no modifier state in the focused app.
fn synthetic_key(code: u16, value: i32) -> [u8; EVENT_SIZE] {
    let mut b = [0u8; EVENT_SIZE];
    b[16..18].copy_from_slice(&EV_KEY.to_ne_bytes());
    b[18..20].copy_from_slice(&code.to_ne_bytes());
    b[20..24].copy_from_slice(&value.to_ne_bytes());
    b
}

/// Any L press/repeat/release while AltGr is down belongs to the chord and must
/// never be re-emitted; otherwise holding AltGr and tapping L twice types "l".
fn l_event_hidden(value: u8, modifier_held: bool) -> bool {
    matches!(value, 0..=2) && modifier_held
}

/// Tracked keys, for the startup/keypress log line that makes recognition visible.
fn tracked(code: u16) -> bool {
    matches!(code, KEY_L | KEY_RIGHTALT)
}

fn key_name(code: u16) -> &'static str {
    match code {
        KEY_L => "L",
        KEY_RIGHTALT => "RIGHTALT",
        _ => "?",
    }
}

/// True for read errors that mean the device node is permanently gone.
fn device_gone(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(ENODEV) | Some(ENOENT))
}

/// True if this device is our own virtual keyboard (or the e2e test injector).
fn is_self_device(fd: std::os::fd::RawFd) -> bool {
    let mut name = [0u8; 128];
    let n = unsafe { libc::ioctl(fd, EVIOCGNAME_128, name.as_mut_ptr()) };
    if n <= 0 {
        return false;
    }
    let name = String::from_utf8_lossy(&name[..(n as usize).min(128)]);
    SELF_DEVICE_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// True if the device's key bitmap carries both keys of the AltGr+L combo.
fn has_altgr_l(fd: std::os::fd::RawFd) -> bool {
    let mut bits = [0u8; 64];
    let n = unsafe { libc::ioctl(fd, EVIOCGBIT_KEY_64, bits.as_mut_ptr()) };
    if n <= 0 {
        return false;
    }
    let has = |code: u16| bits[(code / 8) as usize] & (1 << (code % 8)) != 0;
    has(KEY_L) && has(KEY_RIGHTALT)
}

impl KeyState {
    fn past(&self, last: &Option<Instant>) -> bool {
        match last {
            Some(t) => t.elapsed() >= DEBOUNCE,
            None => true,
        }
    }
}

/// Map one EV_KEY event to an action; updates meta-hold tracking and debounce.
fn decide(state: &mut KeyState, code: u16, value: u8) -> Option<HotkeyEvent> {
    // safety: a missed AltGr keyup (streams out of sync across watchers, or the
    // press is consumed by a remapper like keyd) would leave the hold stuck ON
    // forever, so a later plain L would toggle. If events went quiet for
    // MODIFIER_IDLE_RESET, the hold and the pending chord consumption are stale.
    if (state.modifier_held || state.chord_consumed)
        && state.last_event.elapsed() >= MODIFIER_IDLE_RESET
    {
        state.modifier_held = false;
        state.chord_fired = false;
        state.chord_consumed = false;
    }
    state.last_event = Instant::now();
    match value {
        0 => {
            // release: any watcher seeing the AltGr keyup clears the shared hold
            if code == KEY_RIGHTALT {
                state.modifier_held = false;
                state.chord_fired = false;
            }
            None
        }
        1 => {
            // press — the only binding in the whole tool is AltGr+L
            match code {
                KEY_L
                    if state.modifier_held
                        && !state.chord_fired
                        && state.past(&state.last_toggle) =>
                {
                    state.chord_fired = true; // exactly one toggle per AltGr hold
                    state.chord_consumed = true; // hide the matching AltGr release too
                    state.last_toggle = Some(Instant::now());
                    Some(HotkeyEvent::Toggle)
                }
                KEY_RIGHTALT => {
                    state.modifier_held = true;
                    state.chord_fired = false;
                    state.chord_consumed = false;
                    None
                }
                _ => None,
            }
        }
        _ => None, // 2 = repeat, ignore
    }
}

/// Debounce for the stdin path, mirroring the evdev window: a burst inside
/// DEBOUNCE collapses to one toggle.
fn stdin_allowed(st: &mut KeyState) -> bool {
    let ok = match st.last_toggle {
        Some(t) => t.elapsed() >= DEBOUNCE,
        None => true,
    };
    if ok {
        st.last_toggle = Some(Instant::now());
    }
    ok
}

fn watch_file(
    path: &str,
    tx: &crossbeam_channel::Sender<HotkeyEvent>,
    state: &std::sync::Arc<std::sync::Mutex<KeyState>>,
    live: &std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    uinput: &std::sync::Arc<std::sync::Mutex<Option<UinputKbd>>>,
) -> Option<std::thread::JoinHandle<()>> {
    let mut file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(f) => f,
        Err(_) => File::open(path).ok()?, // read-only watcher on a write-protected node
    };
    if is_self_device(file.as_raw_fd()) {
        return None; // never watch our own virtual keyboard (or the test injector)
    }
    // Grab the device when it can produce the full AltGr+L combo, so L never
    // reaches the focused window; EBUSY (another grabber) falls back to read-only.
    let mut grabbed = false;
    if uinput.lock().unwrap().is_some() && has_altgr_l(file.as_raw_fd()) {
        let arg: libc::c_int = 1;
        grabbed = unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGRAB, arg) } == 0;
    }
    if grabbed {
        eprintln!("hk: {path} grabbed (AltGr+L will not type into windows)");
    }
    // the registry is authoritative: never two watchers on one device path
    if !live.lock().unwrap().insert(path.to_string()) {
        return None; // a live watcher already owns this path
    }
    let tx = tx.clone();
    let state = state.clone();
    let live = live.clone();
    let uinput = uinput.clone();
    let path = path.to_string(); // owned so it can move into the 'static watcher thread
    let short = path.rsplit('/').next().unwrap_or(&path).to_string();
    Some(std::thread::spawn(move || {
        let mut buf = [0u8; EVENT_SIZE];
        'watch: loop {
            match file.read_exact(&mut buf) {
                Ok(()) => {
                    // shortcut: kernel input_event is native-endian; no unsafe
                    let u16at = |o: usize| u16::from_ne_bytes([buf[o], buf[o + 1]]);
                    let (type_, code, value) = (u16at(16), u16at(18), u16at(20));
                    if type_ == EV_KEY {
                        // value: 0=release 1=press 2=autorepeat; decide() ignores repeat
                        if tracked(code) && value != 2 {
                            eprintln!(
                                "hk: {short} {} {}",
                                key_name(code),
                                if value == 1 { "press" } else { "release" }
                            );
                        }
                        // never hold the state lock across eprintln/send: a closed
                        // stderr panics eprintln and would poison the shared mutex
                        // The chord must do ONE thing: toggle dictation. It must not
                        // type "l", leave a held AltGr modifier, or trigger any other
                        // app binding. L press/repeat/release are swallowed; the AltGr
                        // press we already re-emitted is cancelled with a synthetic
                        // release, and the real keyup is swallowed too.
                        // shortcut: cancelling AltGr early means a second level-3 key
                        // pressed while AltGr is still physically held types its base
                        // character; upgrade by buffering the AltGr press if needed.
                        let (hit, hide_altgr_release, hide_l) = {
                            let mut st = state.lock().unwrap();
                            let hit = decide(&mut st, code, value as u8);
                            let hide = code == KEY_RIGHTALT && value == 0 && st.chord_consumed;
                            if hide {
                                st.chord_consumed = false;
                            }
                            (
                                hit,
                                hide,
                                code == KEY_L && l_event_hidden(value as u8, st.modifier_held),
                            )
                        };
                        let swallow = hide_altgr_release || hide_l;
                        let cancel_altgr = grabbed && hit == Some(HotkeyEvent::Toggle);
                        if cancel_altgr {
                            if let Some(dev) = uinput.lock().unwrap().as_mut() {
                                dev.emit(&synthetic_key(KEY_RIGHTALT, 0));
                            }
                        }
                        if !swallow {
                            if let Some(dev) = uinput.lock().unwrap().as_mut() {
                                dev.emit(&buf);
                            }
                        }
                        if let Some(h) = hit {
                            eprintln!("hk: {short} -> {h:?}");
                            if tx.send(h).is_err() {
                                break 'watch; // receiver gone
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // reopened a device with no data yet; keep polling
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => {
                    if device_gone(&e) {
                        // unplugged: exit cleanly; the rescan re-arms if it returns
                        break 'watch;
                    }
                    // transient error: bounded reopen attempts, then give up
                    let mut reopened = false;
                    for _ in 0..REOPEN_ATTEMPTS {
                        std::thread::sleep(REOPEN_SLEEP);
                        match File::open(&path) {
                            Ok(f) => {
                                file = f;
                                reopened = true;
                                break;
                            }
                            Err(re) if device_gone(&re) => break,
                            Err(_) => continue,
                        }
                    }
                    if !reopened {
                        break 'watch;
                    }
                }
            }
        }
        // deregister so the rescan may re-arm this path (single live watcher invariant)
        live.lock().unwrap().remove(&path);
        eprintln!("hk: {short} watcher exited (device gone or channel closed)");
    }))
}

fn stdin_fallback(tx: &crossbeam_channel::Sender<HotkeyEvent>) -> Result<()> {
    eprintln!("hk: no readable /dev/input/event* — stdin fallback: empty line or \"toggle\" = toggle dictation, \"quit\" = exit");
    let mut line = String::new();
    let mut st = KeyState::new(); // same per-kind debounce windows as the evdev path
    loop {
        line.clear();
        let n = std::io::stdin().read_line(&mut line)?;
        if n == 0 {
            return Ok(()); // EOF
        }
        let t = line.trim_end_matches(['\n', '\r']).trim();
        let ev = match t {
            "" | "toggle" => Some(HotkeyEvent::Toggle),
            "quit" => return Ok(()),
            _ => None,
        };
        if let Some(ev) = ev {
            if stdin_allowed(&mut st) {
                tx.send(ev)?;
            }
        }
    }
}

/// Blocks the calling thread forever, sending events. evdev if readable, else stdin fallback.
/// Rescans /dev/input periodically so hot-plugged keyboards (and uinput test devices)
/// that appear after startup are picked up. A device path is watched by at most one
/// live thread (the `live` registry); watchers exit on unplug and are re-armed by the
/// rescan if the node returns.
pub fn listen(tx: crossbeam_channel::Sender<HotkeyEvent>) -> Result<()> {
    // e2e escape hatch on hosts where evdev grabs/uinput are blocked
    if std::env::var_os("STATIC_DICTATION_STDIN_HOTKEYS").is_some() {
        return stdin_fallback(&tx);
    }
    let state = std::sync::Arc::new(std::sync::Mutex::new(KeyState::new()));
    let live: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
    // ONE shared virtual keyboard for re-emitting grabbed events; None on hosts
    // without /dev/uinput access, in which case grabbing is disabled everywhere
    let uinput: std::sync::Arc<std::sync::Mutex<Option<UinputKbd>>> =
        std::sync::Arc::new(std::sync::Mutex::new(UinputKbd::new()));
    let mut armed = 0usize;
    let mut first_scan = true;
    loop {
        let mut found = 0usize;
        let mut new_watchers = 0usize;
        if let Ok(entries) = std::fs::read_dir("/dev/input") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with("event") {
                    continue;
                }
                found += 1;
                let path = format!("/dev/input/{}", name);
                if live.lock().unwrap().contains(&path) {
                    continue; // live watcher already owns this path
                }
                if watch_file(&path, &tx, &state, &live, &uinput).is_some() {
                    armed += 1;
                    new_watchers += 1;
                    eprintln!("hk: watching {path}");
                }
            }
        }
        if new_watchers > 0 {
            eprintln!("hk: {armed} input nodes armed (AltGr+L = dictation; no other hotkey)");
        }
        if first_scan {
            if found == 0 {
                return stdin_fallback(&tx); // nothing readable at all
            }
            first_scan = false;
        }
        // shortcut: 2s poll instead of inotify; fine for a tool that idles at 10ms anyway
        std::thread::sleep(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> KeyState {
        KeyState::new()
    }

    /// linux/input-event-codes.h: 97 = KEY_RIGHTCTRL, 82 = KEY_KP0,
    /// 110 = KEY_INSERT (NumLock-off KP0). None may produce an action.
    #[test]
    fn only_altgr_l_is_bound() {
        let mut s = state();
        for code in [97, 82, 110] {
            assert_eq!(decide(&mut s, code, 1), None, "key {code} must be inert");
            assert_eq!(decide(&mut s, code, 0), None, "key {code} must be inert");
        }
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 1), None);
        assert_eq!(decide(&mut s, KEY_L, 1), Some(HotkeyEvent::Toggle));
    }

    #[test]
    fn d_alone_nothing() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_L, 1), None);
    }

    #[test]
    fn altgr_l_toggles_once_per_hold() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 1), None);
        assert_eq!(decide(&mut s, KEY_L, 1), Some(HotkeyEvent::Toggle));
        assert_eq!(decide(&mut s, KEY_L, 0), None);
        assert_eq!(decide(&mut s, KEY_L, 1), None); // a 2nd tap must NOT toggle or type
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 0), None);
        assert!(!s.modifier_held); // no stuck hold after keyup
    }

    #[test]
    fn altgr_released_l_nothing() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 1), None);
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 0), None);
        assert_eq!(decide(&mut s, KEY_L, 1), None);
    }

    #[test]
    fn stale_meta_cleared_after_idle_gap() {
        // AltGr keydown, then the keyup is lost and the stream goes quiet
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 1), None);
        s.last_event = Instant::now() - Duration::from_secs(10); // simulate the gap
        assert_eq!(decide(&mut s, KEY_L, 1), None); // stale hold must NOT fire a toggle
        assert!(!s.modifier_held);
    }

    #[test]
    fn stdin_debounce_collapses_bursts() {
        let mut s = state();
        assert!(stdin_allowed(&mut s));
        assert!(!stdin_allowed(&mut s)); // sub-window repeat swallowed
        std::thread::sleep(DEBOUNCE + Duration::from_millis(30));
        assert!(stdin_allowed(&mut s)); // a distinct line after the window passes
    }

    #[test]
    fn chord_marks_altgr_release_for_swallowing() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 1), None);
        assert_eq!(decide(&mut s, KEY_L, 1), Some(HotkeyEvent::Toggle));
        assert!(s.chord_consumed); // the real AltGr keyup must not reach apps
        assert!(s.modifier_held); // held until keyup so no L can leak
    }

    /// The real watch_file rule: while AltGr is down, EVERY L event is hidden;
    /// a plain L (and an L after the chord) passes through untouched.
    #[test]
    fn l_events_hidden_only_during_chord() {
        let mut s = state();
        for v in [1, 2, 0] {
            assert!(!l_event_hidden(v, s.modifier_held));
        }
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 1), None);
        for v in [1, 2, 0, 1] {
            // includes a second tap before AltGr keyup
            assert!(
                l_event_hidden(v, s.modifier_held),
                "L value {v} must be hidden"
            );
        }
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 0), None);
        for v in [1, 2, 0] {
            assert!(!l_event_hidden(v, s.modifier_held));
        }
    }
}
