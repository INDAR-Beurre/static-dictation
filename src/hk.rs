//! Global hotkey listener: evdev (RightCtrl PTT, KP0 toggle, AltGr+L toggle) with stdin fallback.

use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HotkeyEvent {
    PttDown,
    PttUp,
    Toggle,
}

/// key codes (values from linux/input-event-codes.h, NOT X keycodes: X = kernel + 8)
const KEY_L: u16 = 38;
const KEY_KP0: u16 = 82; // numpad 0 with NumLock on; NumLock off sends KEY_INSERT
const KEY_RIGHTCTRL: u16 = 97;
const KEY_RIGHTALT: u16 = 100; // AltGr on most layouts

const EV_KEY: u16 = 0x01;
const DEBOUNCE: Duration = Duration::from_millis(175);
/// an idle gap this long means a held meta key missed its keyup — stop trusting it
const MODIFIER_IDLE_RESET: Duration = Duration::from_secs(2);
/// bounded reopen attempts for transient device-read errors before the watcher exits
const REOPEN_ATTEMPTS: u32 = 5;
const REOPEN_SLEEP: Duration = Duration::from_millis(500);
/// raw errno values meaning the device node is permanently gone (unplug / removed)
const ENODEV: i32 = 19;
const ENOENT: i32 = 2;

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

/// Keyboard state shared by ALL device watchers: a grab-and-re-emit chain
/// (keyd → handy clones → compositor) can deliver Super on one /dev/input node
/// and D on another; per-device state would split the combo. Shared also makes
/// the cross-node debounce swallow handy's duplicated physical presses, and
/// means a meta keyup seen by ANY watcher clears the hold for everyone.
/// Keyboard state shared by ALL device watchers: a grab-and-re-emit chain
/// (keyd → handy clones → compositor) can deliver Super on one /dev/input node
/// and D on another; per-device state would split the combo. Shared also makes
/// the cross-node debounce swallow handy's duplicated physical presses, and
/// means a modifier keyup seen by ANY watcher clears the hold for everyone.
struct KeyState {
    modifier_held: bool,
    last_ptt_down: Option<Instant>,
    last_ptt_up: Option<Instant>,
    last_toggle: Option<Instant>,
    last_event: Instant, // any tracked stream activity, for the stale-hold safety
}

impl KeyState {
    fn new() -> Self {
        KeyState {
            modifier_held: false,
            last_ptt_down: None,
            last_ptt_up: None,
            last_toggle: None,
            last_event: Instant::now(),
        }
    }
}

/// Tracked keys, for the startup/keypress log line that makes recognition visible.
fn tracked(code: u16) -> bool {
    matches!(code, KEY_L | KEY_KP0 | KEY_RIGHTCTRL | KEY_RIGHTALT)
}

fn key_name(code: u16) -> &'static str {
    match code {
        KEY_L => "L",
        KEY_KP0 => "KP0",
        KEY_RIGHTCTRL => "RIGHTCTRL",
        KEY_RIGHTALT => "RIGHTALT",
        _ => "?",
    }
}

/// True for read errors that mean the device node is permanently gone.
fn device_gone(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(ENODEV) | Some(ENOENT))
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
    // MODIFIER_IDLE_RESET, the hold is stale.
    if state.modifier_held && state.last_event.elapsed() >= MODIFIER_IDLE_RESET {
        state.modifier_held = false;
    }
    state.last_event = Instant::now();
    match value {
        0 => {
            // release: any watcher seeing a meta keyup clears the shared hold
            if code == KEY_RIGHTCTRL && state.past(&state.last_ptt_up) {
                state.last_ptt_up = Some(Instant::now());
                return Some(HotkeyEvent::PttUp);
            }
            if code == KEY_RIGHTALT {
                state.modifier_held = false;
            }
            None
        }
        1 => {
            // press
            match code {
                KEY_RIGHTCTRL if state.past(&state.last_ptt_down) => {
                    state.last_ptt_down = Some(Instant::now());
                    Some(HotkeyEvent::PttDown)
                }
                KEY_KP0 if state.past(&state.last_toggle) => {
                    state.last_toggle = Some(Instant::now());
                    Some(HotkeyEvent::Toggle)
                }
                KEY_L if state.modifier_held && state.past(&state.last_toggle) => {
                    state.modifier_held = false; // consume AltGr+L, ignore stray L until next tap
                    state.last_toggle = Some(Instant::now());
                    Some(HotkeyEvent::Toggle)
                }
                KEY_RIGHTALT => {
                    state.modifier_held = true;
                    None
                }
                _ => None,
            }
        }
        _ => None, // 2 = repeat, ignore
    }
}

/// Per-kind debounce for the stdin path, mirroring the evdev windows: a burst of
/// the same event inside DEBOUNCE collapses to one, while distinct lines spaced
/// ≥ DEBOUNCE apart all pass.
fn stdin_allowed(st: &mut KeyState, ev: &HotkeyEvent) -> bool {
    let slot = match ev {
        HotkeyEvent::PttDown => &mut st.last_ptt_down,
        HotkeyEvent::PttUp => &mut st.last_ptt_up,
        HotkeyEvent::Toggle => &mut st.last_toggle,
    };
    let ok = match slot {
        Some(t) => t.elapsed() >= DEBOUNCE,
        None => true,
    };
    if ok {
        *slot = Some(Instant::now());
    }
    ok
}

fn watch_file(
    path: &str,
    tx: &crossbeam_channel::Sender<HotkeyEvent>,
    state: &std::sync::Arc<std::sync::Mutex<KeyState>>,
    live: &std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
) -> Option<std::thread::JoinHandle<()>> {
    let mut file = File::open(path).ok()?;
    // the registry is authoritative: never two watchers on one device path
    if !live.lock().unwrap().insert(path.to_string()) {
        return None; // a live watcher already owns this path
    }
    let tx = tx.clone();
    let state = state.clone();
    let live = live.clone();
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
                        let hit = {
                            let mut st = state.lock().unwrap();
                            decide(&mut st, code, value as u8)
                        };
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
    eprintln!("hk: no readable /dev/input/event* — stdin fallback: empty line or \"start\" = start, \"stop\" = stop, \"toggle\" = toggle, \"quit\" = exit");
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
            "" | "start" => Some(HotkeyEvent::PttDown),
            "stop" => Some(HotkeyEvent::PttUp),
            "toggle" => Some(HotkeyEvent::Toggle),
            "quit" => return Ok(()),
            _ => None,
        };
        if let Some(ev) = ev {
            if stdin_allowed(&mut st, &ev) {
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
                if watch_file(&path, &tx, &state, &live).is_some() {
                    armed += 1;
                    new_watchers += 1;
                    eprintln!("hk: watching {path}");
                }
            }
        }
        if new_watchers > 0 {
            eprintln!("hk: {armed} input nodes armed (AltGr+L / NumPad0 = toggle dictation, hold RightCtrl = push-to-talk)");
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

    #[test]
    fn rightctrl_ptt() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_RIGHTCTRL, 1), Some(HotkeyEvent::PttDown));
        assert_eq!(decide(&mut s, KEY_RIGHTCTRL, 0), Some(HotkeyEvent::PttUp));
    }

    #[test]
    fn kp0_toggles() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_KP0, 1), Some(HotkeyEvent::Toggle));
        assert_eq!(decide(&mut s, KEY_KP0, 1), None); // second tap inside debounce window ignored
    }

    #[test]
    fn d_alone_nothing() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_L, 1), None);
    }

    #[test]
    fn altgr_l_toggles() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_RIGHTALT, 1), None);
        assert_eq!(decide(&mut s, KEY_L, 1), Some(HotkeyEvent::Toggle));
        assert!(!s.modifier_held); // consumed with the toggle, no leak
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
        assert!(stdin_allowed(&mut s, &HotkeyEvent::Toggle));
        assert!(!stdin_allowed(&mut s, &HotkeyEvent::Toggle)); // sub-window repeat swallowed
        assert!(stdin_allowed(&mut s, &HotkeyEvent::PttDown)); // other kind unaffected
        std::thread::sleep(DEBOUNCE + Duration::from_millis(30));
        assert!(stdin_allowed(&mut s, &HotkeyEvent::Toggle)); // a distinct line after the window passes
    }
}
