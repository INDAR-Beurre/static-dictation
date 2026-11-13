//! output: deliver transcribed text (stdout + clipboard) and spoken feedback.

use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
enum Choice {
    WlCopy,
    Xclip,
    None,
}

/// Pure decision for tests: which clipboard path fits this session.
fn clipboard_target(wayland: bool, x11: bool) -> Choice {
    if wayland {
        Choice::WlCopy
    } else if x11 {
        Choice::Xclip
    } else {
        Choice::None
    }
}

/// clipboard + stdout + spoken "OK done"
pub fn deliver(text: &str, no_tts: bool) {
    println!("{text}");

    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    let ok = match clipboard_target(wayland, x11) {
        Choice::WlCopy => copy_to_clipboard("wl-copy", &[], text) || copy_to_clipboard("xclip", &["-selection", "clipboard"], text),
        Choice::Xclip => copy_to_clipboard("xclip", &["-selection", "clipboard"], text),
        Choice::None => false,
    };
    if !ok {
        eprintln!("static-dictation: clipboard unavailable (no wl-copy/xclip or no display)");
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

/// Send text to a clipboard tool's stdin WITHOUT a trailing newline so paste is clean.
fn copy_to_clipboard(bin: &str, args: &[&str], text: &str) -> bool {
    match Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => match child.stdin.as_mut() {
            Some(stdin) => {
                if stdin.write_all(text.as_bytes()).is_err() {
                    let _ = child.kill();
                    return false;
                }
                // dropping stdin closes it
                child.stdin.take();
                child.wait().map(|s| s.success()).unwrap_or(false)
            }
            None => false,
        },
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_prefers_wl_copy() {
        assert_eq!(clipboard_target(true, true), Choice::WlCopy);
    }

    #[test]
    fn wayland_only() {
        assert_eq!(clipboard_target(true, false), Choice::WlCopy);
    }

    #[test]
    fn x11_falls_back_to_xclip() {
        assert_eq!(clipboard_target(false, true), Choice::Xclip);
    }

    #[test]
    fn no_display_is_none() {
        assert_eq!(clipboard_target(false, false), Choice::None);
    }
}
