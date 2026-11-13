//! static-dictation v1: lightweight push-to-talk / toggle dictation → Groq whisper-large-v3-turbo.

mod capture;
mod config;
mod endpoint;
mod groq;
mod hk;
mod output;
mod vad;

use crossbeam_channel::unbounded;
use endpoint::Decision;
use hk::HotkeyEvent;

/// ms of silence after speech before we finalize an utterance.
const SILENCE_MS: u32 = 700;
/// 30 ms frames @ 16 kHz.
const FRAME: usize = 480;
/// Keep ~300 ms of pre-voice audio so utterance starts aren't clipped.
const PRE_ROLL: usize = 4800 * 3 / 10;

fn main() {
    let cfg = config::load();

    let (hk_tx, hk_rx) = unbounded::<HotkeyEvent>();
    std::thread::spawn(move || {
        if let Err(e) = hk::listen(hk_tx) {
            eprintln!("hotkey listener failed: {e}");
            std::process::exit(1);
        }
    });

    println!("static-dictation v1 — hold RightCtrl to speak (release to transcribe); NumPad0 or Super+D toggles continuous dictation");

    let audio = capture::start().expect("no microphone available");
    let mut vad = vad::Vad::new();
    let mut ep = endpoint::EndPointer::new(SILENCE_MS);
    let mut recording = false; // PTT held or toggle-mode ON
    let mut seen_voice = false;
    let mut utterance: Vec<f32> = Vec::new();
    let mut buf: Vec<f32> = Vec::new();

    loop {
        // Move any new audio into the local buffer (non-blocking).
        while let Ok(chunk) = audio.try_recv() {
            buf.extend_from_slice(&chunk);
        }

        // Process whole 30 ms frames.
        while buf.len() >= FRAME {
            let frame: Vec<f32> = buf.drain(..FRAME).collect();
            if recording {
                let voice = vad.is_voice(&frame);
                if voice {
                    seen_voice = true;
                }
                if !seen_voice && utterance.len() > PRE_ROLL {
                    let excess = utterance.len() - PRE_ROLL;
                    utterance.drain(..excess);
                }
                utterance.extend_from_slice(&frame);
                if ep.push(voice) == Decision::End {
                    finalize(&cfg, &mut utterance, &mut seen_voice, &mut ep);
                    // toggle mode keeps going; PTT mode ends when key is released
                }
            }
        }

        // Hotkey events.
        while let Ok(ev) = hk_rx.try_recv() {
            match ev {
                HotkeyEvent::PttDown => {
                    ep = endpoint::EndPointer::new(SILENCE_MS);
                    recording = true;
                }
                HotkeyEvent::PttUp => {
                    recording = false;
                    finalize(&cfg, &mut utterance, &mut seen_voice, &mut ep);
                }
                HotkeyEvent::Toggle => {
                    if recording {
                        recording = false;
                        finalize(&cfg, &mut utterance, &mut seen_voice, &mut ep);
                    } else {
                        ep = endpoint::EndPointer::new(SILENCE_MS);
                        recording = true;
                    }
                }
            }
        }

        // Nothing to do: brief sleep instead of a busy loop.
        if buf.len() < FRAME && hk_rx.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

/// Send utterance → groq → output. Blocking; at most one utterance in flight by construction.
fn finalize(
    cfg: &config::Config,
    utterance: &mut Vec<f32>,
    seen_voice: &mut bool,
    ep: &mut endpoint::EndPointer,
) {
    *ep = endpoint::EndPointer::new(SILENCE_MS);
    if !*seen_voice || utterance.is_empty() {
        utterance.clear();
        *seen_voice = false;
        return;
    }
    match groq::transcribe(&cfg.groq_api_key, &cfg.endpoint, utterance) {
        Ok(text) if !text.is_empty() => output::deliver(&text, cfg.no_tts),
        Ok(_) => output::error("empty transcription", cfg.no_tts),
        Err(e) => output::error(&e.to_string(), cfg.no_tts),
    }
    utterance.clear();
    *seen_voice = false;
}
