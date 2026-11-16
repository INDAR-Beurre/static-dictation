//! static-dictation v1: lightweight toggle dictation → Groq whisper-large-v3-turbo.

mod bands;
mod capture;
mod config;
mod endpoint;
mod groq;
mod hk;
mod output;
// Kept only for the offscreen QA frame dump; the Wayland overlay is no longer spawned.
#[allow(dead_code)]
mod pill;
mod vad;

// H1 frame dump (see maybe_spawn call in main): dead weight unless the env
// flag is set — when it is not, the module's one function returns immediately.
mod dump;

use crossbeam_channel::unbounded;
use endpoint::Decision;
use hk::HotkeyEvent;

/// ms of silence after speech before we finalize an utterance.
const SILENCE_MS: u32 = 700;
/// 30 ms frames @ 16 kHz.
const FRAME: usize = 480;
/// Keep ~300 ms of pre-voice audio so utterance starts aren't clipped.
const PRE_ROLL: usize = 4800;
/// Whisper invents "Thank you."/"Amen." on room noise. Require ~300 ms of voiced
/// frames in the utterance before spending a Groq request on it.
const MIN_VOICE_FRAMES: u32 = 10;

fn worth_transcribing(voice_frames: u32, samples: usize) -> bool {
    voice_frames >= MIN_VOICE_FRAMES && samples > 0
}

fn main() {
    // A closed stdout reader (e.g. piped to `head`) must kill us silently, not panic in println.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cfg = config::load();
    // The separate Wayland overlay sat on top of the desktop island and looked wrong.
    // Dictation runs headless; the island keeps its clean interface.
    let pill: Option<pill::PillHandle> = None;

    let (hk_tx, hk_rx) = unbounded::<HotkeyEvent>();
    std::thread::spawn(move || {
        match hk::listen(hk_tx) {
            Err(e) => {
                eprintln!("hotkey listener failed: {e}");
                std::process::exit(1);
            }
            // stdin fallback hit EOF/"quit": hotkey source gone → shut down the whole process.
            Ok(()) => {
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let _ = std::io::stderr().flush();
                std::process::exit(0);
            }
        }
    });

    println!("static-dictation v1 — AltGr+L toggles dictation (the only shortcut)");

    let audio = capture::start().expect("no microphone available");
    let mut vad = vad::Vad::new();
    let mut ep = endpoint::EndPointer::new(SILENCE_MS);
    let mut band_analyzer = bands::Analyzer::new();
    let mut recording = false; // AltGr+L toggles this
    let mut voice_frames: u32 = 0;
    let mut utterance: Vec<f32> = Vec::new();
    let mut buf: Vec<f32> = Vec::new();

    // H1 harness hook: STATIC_DICTATION_DUMP_FRAMES=<dir> spawns a headless
    // frame dumper rendering the pill's real Sim+canvas pipeline offscreen,
    // writing a PNG every fixed 100 ms wall interval — beats grim's ~0.3
    // s/shot aliasing. Env unset → nothing starts; zero behavior change.
    dump::maybe_spawn();

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
                    voice_frames += 1;
                }
                if voice_frames == 0 && utterance.len() > PRE_ROLL {
                    let excess = utterance.len() - PRE_ROLL;
                    utterance.drain(..excess);
                }
                utterance.extend_from_slice(&frame);
                if let Some(p) = &pill {
                    let rms =
                        (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
                    let bands = band_analyzer.energies(&frame);
                    p.update(rms, &bands, voice);
                }
                if ep.push(voice) == Decision::End {
                    finalize(&cfg, &pill, &mut utterance, &mut voice_frames, &mut ep);
                    audio.drop_backlog();
                    // dictation keeps going until AltGr+L toggles it off
                }
            }
        }

        // Hotkey events.
        while let Ok(ev) = hk_rx.try_recv() {
            match ev {
                HotkeyEvent::Toggle => {
                    eprintln!(
                        "static-dictation: toggle event received (recording={} )",
                        recording
                    );
                    if recording {
                        recording = false;
                        if let Some(p) = &pill {
                            p.hide();
                        }
                        finalize(&cfg, &pill, &mut utterance, &mut voice_frames, &mut ep);
                        audio.drop_backlog();
                    } else {
                        ep = endpoint::EndPointer::new(SILENCE_MS);
                        voice_frames = 0;
                        if let Some(p) = &pill {
                            p.show();
                        }
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
    pill: &Option<pill::PillHandle>,
    utterance: &mut Vec<f32>,
    voice_frames: &mut u32,
    ep: &mut endpoint::EndPointer,
) {
    *ep = endpoint::EndPointer::new(SILENCE_MS);
    // Hide before the blocking network call so the pill vanishes the instant dictation stops.
    if let Some(p) = pill {
        p.hide();
    }
    if !worth_transcribing(*voice_frames, utterance.len()) {
        utterance.clear();
        *voice_frames = 0;
        return;
    }
    match groq::transcribe(&cfg.groq_api_key, &cfg.endpoint, utterance) {
        Ok(text) if !text.is_empty() => output::deliver(&text),
        Ok(_) => output::error("empty transcription"),
        Err(e) => output::error(&e.to_string()),
    }
    utterance.clear();
    *voice_frames = 0;
}

#[cfg(test)]
mod tests {
    use super::worth_transcribing;

    #[test]
    fn room_noise_blips_are_not_transcribed() {
        // Measured on this host: 12 s of ambient mic yields at most 8 voiced
        // frames in a row, 11 total — well under a real utterance.
        assert!(!worth_transcribing(3, 4800));
        assert!(!worth_transcribing(9, 4800));
        assert!(!worth_transcribing(10, 0));
    }

    #[test]
    fn sustained_speech_is_transcribed() {
        // 3 s of spoken "hello this is a dictation test one two three" = 36 voiced frames.
        assert!(worth_transcribing(10, 4800));
        assert!(worth_transcribing(36, 48000));
    }
}
