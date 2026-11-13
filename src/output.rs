//! output: deliver transcribed text (stdout + direct typing) and spoken feedback.

use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
enum Choice {
    WlType,
    Xdotool,
    None,
}

/// Pure decision for tests: which direct-typing path fits this session.
fn typing_target(wayland: bool, x11: bool) -> Choice {
    if wayland {
        Choice::WlType
    } else if x11 {
        Choice::Xdotool
    } else {
        Choice::None
    }
}

/// type text into the focused window + stdout + spoken "OK done"
pub fn deliver(text: &str, no_tts: bool) {
    println!("{text}");

    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    let ok = match typing_target(wayland, x11) {
        Choice::WlType => type_text("wtype", &[], text) || type_text("xdotool", &["type", "--", "-delay", "0"], text),
        Choice::Xdotool => type_text("xdotool", &["type", "--", "-delay", "0"], text),
        Choice::None => false,
    };
    if !ok {
        eprintln!("static-dictation: typing unavailable (no wtype/xdotool or no display)");
    }

    if !no_tts {
        speak("OK done");
    }
}

/// eprintln the error + speak an error phrase
pub fn error(msg: &str, no_tts: bool) {
    eprintln!("static-dictation: {msg}");
    if !no_tts {
        speak("transcription failed");
    }
}

/// speak a phrase synchronously (espeak-ng preferred, espeak fallback). Best-effort; never panics.
pub fn speak(text: &str) {
    for bin in ["espeak-ng", "espeak"] {
        if let Ok(mut child) = Command::new(bin)
            .arg(text)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            // Best-effort: ignore status, but wait so speech is synchronous.
            let _ = child.wait();
            return;
        }
    }
}

/// Type text as if from a keyboard via a typing tool. wtype types raw; xdotool needs `type --`.
/// shortcut: no per-char pacing on Wayland; some apps reject instant text, revisit if reported
fn type_text(bin: &str, args: &[&str], text: &str) -> bool {
    match Command::new(bin)
        .args(args)
        .arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(s) => s.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_prefers_wtype() {
        assert_eq!(typing_target(true, true), Choice::WlType);
    }

    #[test]
    fn wayland_only() {
        assert_eq!(typing_target(true, false), Choice::WlType);
    }

    #[test]
    fn x11_falls_back_to_xdotool() {
        assert_eq!(typing_target(false, true), Choice::Xdotool);
    }

    #[test]
    fn no_display_is_none() {
        assert_eq!(typing_target(false, false), Choice::None);
    }
}
