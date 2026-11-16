//! output: deliver transcribed text (stdout + direct typing). No audio output.

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

/// Max chars typed per command: Linux per-arg limit is ~128 KiB (E2BIG); stay well under.
const MAX_TYPED_CHARS: usize = 100_000;

/// type text into the focused window + stdout
pub fn deliver(text: &str) {
    println!("{text}");

    let typed = clamp_for_typing(flatten_newlines(text));

    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    let ok = match typing_target(wayland, x11) {
        Choice::WlType => {
            type_text("wtype", &[], &typed)
                || type_text("xdotool", &["type", "--", "-delay", "0"], &typed)
        }
        Choice::Xdotool => type_text("xdotool", &["type", "--", "-delay", "0"], &typed),
        Choice::None => false,
    };
    if !ok {
        eprintln!("static-dictation: typing unavailable (no wtype/xdotool or no display)");
    }
}

/// eprintln the error
pub fn error(msg: &str) {
    eprintln!("static-dictation: {msg}");
}

/// Newlines would be typed as Enter keypresses (submitting chat forms mid-dictation); flatten to spaces.
fn flatten_newlines(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// Guard against E2BIG: keep the typed arg under the OS per-arg limit.
fn clamp_for_typing(text: String) -> String {
    if text.chars().count() <= MAX_TYPED_CHARS {
        text
    } else {
        eprintln!("static-dictation: transcription exceeds {MAX_TYPED_CHARS} chars, truncating typed text");
        text.chars().take(MAX_TYPED_CHARS).collect()
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
        .output()
    {
        Ok(o) => o.status.success(),
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

    #[test]
    fn newlines_become_spaces() {
        assert_eq!(flatten_newlines("hello\nworld"), "hello world");
        assert_eq!(flatten_newlines("line1\r\nline2"), "line1  line2");
        assert_eq!(flatten_newlines("a\rb\nc"), "a b c");
        assert_eq!(flatten_newlines("no newlines"), "no newlines");
        assert_eq!(flatten_newlines(""), "");
    }
}
