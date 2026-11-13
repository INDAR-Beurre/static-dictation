//! Global hotkey listener: evdev (RightCtrl PTT, KP0 toggle, Super+D toggle) with stdin fallback.

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

/// key codes
const KEY_D: u16 = 32;
const KEY_KP0_OLD: u16 = 97; // old kernels
const KEY_KP0: u16 = 98;
const KEY_RIGHTCTRL: u16 = 304;
const KEY_LEFTMETA: u16 = 125;
const KEY_RIGHTMETA: u16 = 126;

const EV_KEY: u16 = 0x01;
const DEBOUNCE: Duration = Duration::from_millis(175);

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

/// Keyboard state tracked per input device, shared per-thread via decide().
pub struct KeyState {
    meta_held: bool,
    last_ptt_down: Option<Instant>,
    last_ptt_up: Option<Instant>,
    last_toggle: Option<Instant>,
}

impl KeyState {
    fn new() -> Self {
        KeyState { meta_held: false, last_ptt_down: None, last_ptt_up: None, last_toggle: None }
    }

    fn past(&self, last: &Option<Instant>) -> bool {
        match last {
            Some(t) => t.elapsed() >= DEBOUNCE,
            None => true,
        }
    }
}

/// Map one EV_KEY event to an action; updates meta-hold tracking and debounce.
fn decide(state: &mut KeyState, code: u16, value: u8) -> Option<HotkeyEvent> {
    match value {
        0 => {
            // release
            if code == KEY_RIGHTCTRL && state.past(&state.last_ptt_up) {
                state.last_ptt_up = Some(Instant::now());
                return Some(HotkeyEvent::PttUp);
            }
            if code == KEY_LEFTMETA || code == KEY_RIGHTMETA {
                state.meta_held = false;
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
                KEY_KP0 | KEY_KP0_OLD if state.past(&state.last_toggle) => {
                    state.last_toggle = Some(Instant::now());
                    Some(HotkeyEvent::Toggle)
                }
                KEY_D if state.meta_held && state.past(&state.last_toggle) => {
                    state.meta_held = false; // consume Super+D, ignore stray D until next tap
                    state.last_toggle = Some(Instant::now());
                    Some(HotkeyEvent::Toggle)
                }
                KEY_LEFTMETA | KEY_RIGHTMETA => {
                    state.meta_held = true;
                    None
                }
                _ => None,
            }
        }
        _ => None, // 2 = repeat, ignore
    }
}

fn watch_file(path: &str, tx: &crossbeam_channel::Sender<HotkeyEvent>) -> Option<std::thread::JoinHandle<()>> {
    let mut file = File::open(path).ok()?;
    let tx = tx.clone();
    Some(std::thread::spawn(move || {
        let mut state = KeyState::new();
        let mut buf = [0u8; EVENT_SIZE];
        loop {
            match file.read_exact(&mut buf) {
                Ok(()) => {
                    // shortcut: kernel input_event is native-endian; no unsafe
                    let u16at = |o: usize| u16::from_ne_bytes([buf[o], buf[o + 1]]);
                    let (type_, code, value) = (u16at(16), u16at(18), u16at(20));
                    if type_ == EV_KEY {
                        // value: 0=release 1=press 2=autorepeat; decide() ignores repeat
                        if let Some(h) = decide(&mut state, code, value as u8) {
                            if tx.send(h).is_err() {
                                return; // receiver gone
                            }
                        }
                    }
                }
                Err(_) => return,
            }
        }
    }))
}

fn stdin_fallback(tx: &crossbeam_channel::Sender<HotkeyEvent>) -> Result<()> {
    eprintln!("hk: no readable /dev/input/event* — stdin fallback: empty line or \"start\" = start, \"stop\" = stop, \"toggle\" = toggle, \"quit\" = exit");
    let mut line = String::new();
    loop {
        line.clear();
        let n = std::io::stdin().read_line(&mut line)?;
        if n == 0 {
            return Ok(()); // EOF
        }
        let t = line.trim_end_matches(['\n', '\r']).trim();
        match t {
            "" | "start" => tx.send(HotkeyEvent::PttDown)?,
            "stop" => tx.send(HotkeyEvent::PttUp)?,
            "toggle" => tx.send(HotkeyEvent::Toggle)?,
            "quit" => return Ok(()),
            _ => {}
        }
    }
}

/// Blocks the calling thread forever, sending events. evdev if readable, else stdin fallback.
pub fn listen(tx: crossbeam_channel::Sender<HotkeyEvent>) -> Result<()> {
    let mut opened = 0;
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("event") {
                let path = format!("/dev/input/{}", name);
                if let Some(_h) = watch_file(&path, &tx) {
                    opened += 1;
                }
            }
        }
    }
    if opened == 0 {
        stdin_fallback(&tx)
    } else {
        loop {
            std::thread::park();
        }
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
        assert_eq!(decide(&mut s, KEY_KP0_OLD, 1), None); // debounced
    }

    #[test]
    fn d_alone_nothing() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_D, 1), None);
    }

    #[test]
    fn meta_d_toggles() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_LEFTMETA, 1), None);
        assert_eq!(decide(&mut s, KEY_D, 1), Some(HotkeyEvent::Toggle));
    }

    #[test]
    fn meta_released_d_nothing() {
        let mut s = state();
        assert_eq!(decide(&mut s, KEY_RIGHTMETA, 1), None);
        assert_eq!(decide(&mut s, KEY_RIGHTMETA, 0), None);
        assert_eq!(decide(&mut s, KEY_D, 1), None);
    }
}
