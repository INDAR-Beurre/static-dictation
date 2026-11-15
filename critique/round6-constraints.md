# Round-6 constraints check

**Score: 9/10.** All specified hard constraints are supported by measured or documented evidence; remaining proof gaps require a human with target hardware/input.

| Constraint | Evidence / verdict |
|---|---|
| Release binary ≤50 MB | `stat -c %s target/release/static-dictation` = **3,750,056 bytes** (~3.58 MiB), well under 50 MB. `critique/footprint-fluidity.md` independently measured 3,757,344 bytes; the small difference is build variation. |
| All Rust; no shipped image/font assets | App sources are Rust. No `include_bytes!` use in `src/` (there is a source-only `include_str!("tsar.rs")` self-reference). No embedded fonts/images; procedural rendering. Images in `critique/` and `live-proof/` are QA/evidence only, not shipped assets. |
| ~33 Hz audio and 16 ms render cadence with headroom | `critique/footprint-fluidity.md`: audio frame 480 samples at 16 kHz = **30 ms / 33.3 Hz**; live render/fade tick **16 ms / 62.5 Hz**. Full scale-2 render measured **6.8–7.6 ms** on i7-1185G7, ~4.3× headroom against 30 ms; 2–3× slower low-end estimate is 15–22 ms, still within budget. |
| AltGr+L end-to-end proof | `critique/altgr-l-live-proof.md` documents actual evdev RightAlt(100)+L(38) injection, virtual mic with synthetic speech, and a real Groq `whisper-large-v3-turbo` transcript: “Hello this is a dictation test 123”. Exit 0. Headless test intentionally did not type into desktop. |
| Fluid deterministic animation | `critique/round3-motion-sheet.md`: 79 frames; mean profile change **0.158 px/step**, max **0.608 px**, breathing period **2.60 s**. Correlation is 0.9951 mean / 0.9780 min, below the stated ≥0.998 critic gate, so “fluid” is evidenced but that stricter consistency gate is not met. `src/pill/vecmentor.rs:225` uses deterministic golden-angle sine jitter, no RNG; identical across runs/tests. |

## Still human-only

- Real microphone and real human voice through AltGr+L (the existing proof uses a virtual source and synthetic `espeak` audio).
- A keyboard/layout where AltGr emits a keycode other than 100.
- Performance on actual low-end CPUs; the 15–22 ms figure is an estimate, not a measurement.
- Normal desktop typing delivery was not exercised by this headless proof; it explicitly reports typing unavailable without a display.
