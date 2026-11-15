# Round 8 — final constraints review

**8/10**

| Requirement | Result | Evidence / qualification |
|---|---|---|
| All-Rust application | PASS | Product implementation is `src/*.rs`; `Cargo.toml` declares Rust dependencies and no embedded assets. QA/injection scripts are Python, not app runtime. |
| ≤50 MB | PASS | Release executable `target/release/static-dictation` is 3.6M (3.58 MiB), well below limit. |
| Fluid, non-random animation | PASS | `critique/round8-motion-sheet.md`: 0.032 px mean height change/step, 0.9983 mean consecutive profile correlation, 2.70 s breathing period; settled per-column max step 2 px. |
| AltGr+L dictation end-to-end | PARTIAL PASS | `critique/altgr-l-live-proof.md` records injected chord → real Groq transcription (“Hello this is a dictation test 123”). It deliberately ran headless: transcript printed, **typing into a foreground Wayland app was not proven**. |
| Tests / clippy | PASS | Re-run: 95 passed, 2 ignored; `cargo clippy --all-targets -- -D warnings` clean. |
| Reference fidelity | PARTIAL | This constraints-only review does not score pixels; round-8 measurements document the shape/motion fix. |

## Top fixes / evidence gaps

1. **Prove actual typing delivery on the normal desktop.** `critique/altgr-l-live-proof.md` explicitly says “typing unavailable” because the proof unsets `WAYLAND_DISPLAY`/`DISPLAY`. Capture a foreground-app test showing the recognized transcript reaches the focused window, with cleanup and no unintended keystrokes. This is the largest unmet part of “AltGr+L works 100% for dictation.”
2. **Exercise real microphone + speaker path.** The proof uses a null sink and espeak-generated speech; real microphone/voice remains human-only (the document lists H1–H8). Run that plan on the target host and record device, transcript, and delivery result.
3. **Verify portability of the hotkey path.** `src/hk.rs:20` hardcodes RightAlt kernel code 100, and the proof notes keyboards emitting another code are untested. Test the intended target keyboards/layouts; if support is required, make the modifier configurable or detect the actual key event without breaking ordinary AltGr input.

**Already right:** the implementation is compact, the verified synthetic E2E reaches real transcription, and the measured animation is smooth rather than jittery; release size and automated quality gates have ample margin.

Human-only remaining: real-mic/real-voice test, foreground typing on the actual Wayland desktop, and alternate AltGr keycode coverage. The “100%” claim should remain host-conditional until those are covered.