# static-dictation v1 — spec + module contracts

Normal dictation tool (no pill bar — next step). Rust, threads + channels, NO async/tokio.
Cloud-only: audio → Groq API `whisper-large-v3-turbo` → text → clipboard + stdout.

## Pipeline

```
main: config load (GROQ_API_KEY env)
  hotkey listen (evdev /dev/input/event*, stdin fallback)
    RightCtrl hold  = push-to-talk (down=record, up=finalize)   [default mode]
    NumPad0 press   = toggle  (once=start, again=stop)
    Super+D         = toggle (track L/R Super held + D pressed)
  capture (cpal → resample to 16k mono f32 → channel)
  VAD (30ms frames, energy+ZCR, EMA noise floor) → endpointer (700ms silence→end, 60s cap)
  on end-of-utterance → groq::transcribe (blocking) → output::deliver
  at most 1 transcription in flight (drop-old-keep-new queue of depth 1)
```

## Hotkeys

| Key | evdev code | Action |
|---|---|---|
| RightCtrl | KEY_RIGHTCTRL (304) | PTT: down=start, up=stop+finalize |
| NumPad0 (KP0) | KEY_KP0 (98) | toggle |
| D with L/R-Super held | KEY_D (32) + KEY_LEFTMETA(125)/KEY_RIGHTMETA(126) | toggle |

envdev notes: 175ms debounce per key; ignore synthetic KEY_N re-emit garbage (value 2 = repeat, skip); swallow matched hotkey events is not needed at v1 (can't easily inject, don't try). /dev/input unreadable → warn + stdin fallback (Enter=start, empty Enter=stop+finalize).

## Outputs

- clipboard: `wl-copy` (Wayland), fallback `xclip -selection clipboard`; stdout print always.
- espeak-ng (fallback `espeak`) speaks "OK done" on success; error phrase on failure. NO_TTS=1 disables speech.

## Errors

- GROQ_API_KEY missing → eprintln + exit(1)
- no mic / cpal stream error → eprintln + exit(1)
- groq 4xx/5xx → speak error, log, keep running
- logging: eprintln! only (no log/tracing crates)

## Cargo deps (pinned, nothing else)

cpal 0.15, ringbuf 0.4 (optional—prefer channels), crossbeam-channel 0.5,
reqwest 0.12 {blocking, multipart, rustls-tls, default-features=false},
serde 1 {derive}, serde_json 1, anyhow 1.

## Module contracts (EXACT — all agents code against these signatures)

```rust
// src/config.rs
pub struct Config { pub groq_api_key: String, pub no_tts: bool }
pub fn load() -> Config; // env GROQ_API_KEY (exit 1 if missing), NO_TTS

// src/capture.rs
/// Start default input device; returns rx of f32 chunks @ 16k mono.
pub fn start() -> anyhow::Result<crossbeam_channel::Receiver<Vec<f32>>>;
// resamples native rate (44.1k/48k) to 16k by linear interpolation.
#[cfg(test)] fn resample(input: &[f32], from: u32) -> Vec<f32>; // unit-testable pure fn

// src/vad.rs
pub struct Vad { /* EMA noise floor, thresholds */ }
impl Vad {
    pub fn new() -> Self;
    /// frame = 480 samples @16k (30ms). true if voice.
    pub fn is_voice(&mut self, frame: &[f32]) -> bool;
}

// src/endpoint.rs
pub enum Decision { Ongoing, End }
pub struct EndPointer { /* silence budget */ }
impl EndPointer {
    pub fn new(silence_ms: u32) -> Self; // 700
    /// Feed 30ms frame verdict; End when silence exceeded or 60s cap hit.
    pub fn push(&mut self, is_voice: bool) -> Decision;
}

// src/groq.rs
/// samples @16k mono f32 → Groq → text. Blocking. Bumps volume NOT normalized; converts to 16-bit WAV in-memory (no extra deps).
pub fn transcribe(api_key: &str, samples: &[f32]) -> anyhow::Result<String>;
#[cfg(test)] fn to_wav(samples: &[f32]) -> Vec<u8>; // unit-testable pure fn

// src/hk.rs
pub enum HotkeyEvent { PttDown, PttUp, Toggle }
/// Blocks current thread. evdev if readable, else stdin fallback.
pub fn listen(tx: crossbeam_channel::Sender<HotkeyEvent>) -> anyhow::Result<()>;

// src/output.rs
pub fn deliver(text: &str, no_tts: bool);          // clipboard + stdout + "OK done"
pub fn error(msg: &str, no_tts: bool);             // eprintln + spoken error

// src/main.rs — pipeline glue only (uses the above)
```

## Style

- edition 2021, no unsafe, no clones of big buffers
- one shortcut comment allowed where noted
- unit tests only for pure logic (resample, vad params, endpointer timing, wav header)
- `cargo test` must pass with no network
