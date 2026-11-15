# Round 9 — final constraints review

**8/10**

| Constraint | Result | Evidence |
|---|---|---|
| All-Rust runtime | PASS | App implementation and dependencies are Rust (`src/*.rs`, `Cargo.toml`). Python files are QA/injection helpers, not runtime. |
| ≤50 MB | PASS | `target/release/static-dictation`: 3,749,736 bytes (3.6 MiB), stripped x86-64 ELF. |
| Fluid, deterministic animation | PASS | `critique/round9-motion-sheet.md`: 0.027 px mean change/step, 0.9988 consecutive-profile correlation, 2.80 s period, 2 px max column step. Renderer animation uses fixed-time/sine-based signals; GIFs show smooth motion. |
| AltGr+L E2E dictation | PARTIAL | `critique/altgr-l-live-proof.md` proves RightAlt(100)+L toggles capture and synthetic speech reaches real Groq transcription. It is headless: foreground Wayland typing was unavailable; real mic/speaker remains human-only. |
| Automated quality gates | PASS | Re-ran `cargo test --quiet`: 95 passed, 2 ignored; `cargo clippy --all-targets -- -D warnings` clean. |
| Reference match | PARTIAL | Round-9 shape/feed changes address broad active-voice shoulders and asymmetric single apex. Measurement still calls halo isolation and gap/haze/midgap ROIs provisional; their current ROI includes shadow, so this is not a clean quantitative fidelity pass. |

## Top remaining fixes

1. **Prove typed delivery into a focused Wayland app** — `critique/altgr-l-live-proof.md:19` records “typing unavailable”; its headless setup intentionally cannot establish the end-to-end user-visible delivery requirement. This is the main blocker to an unconditional “works 100%” claim.
2. **Close the remaining visual measurement gap** — the brief cites `critique/round9-measurement.md`, but that file does not exist; only `critique/round9-motion-sheet.md` (stale title "Round-3 motion sheet") exists. Write the measurement file, isolate halo from body, and use shadow-excluding gap/haze/midgap ROIs compared against equivalent reference regions.

**Right:** the binary and test/clippy gates have clear margin, while the motion data supports deterministic, gradual animation; the AltGr+L proof reaches actual transcription on this host.

Skipped: no new real-mic or foreground-typing test (these require a human desktop/audio path); no alternate AltGr keycode validation. Risk: end-to-end typing and non-100 AltGr hardware remain unproven.