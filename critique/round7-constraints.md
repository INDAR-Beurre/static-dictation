# Round 7 — constraints lens

**Score: 9/10** — hard footprint, implementation-language, and tested-key-path constraints have strong evidence; “100%” and fluidity are not fully established across real-user conditions.

| Constraint | Result | Concrete evidence |
|---|---|---|
| Release binary ≤50 MB | PASS | `stat -c %s target/release/static-dictation` = **3,749,864 bytes** (~3.58 MiB), about 13× below the cap. |
| All Rust / no shipped image or font assets | PASS* | `round6-constraints.md` reports Rust sources, procedural rendering, no embedded image/font assets or `include_bytes!` in `src/`. |
| AltGr+L dictation | PASS (host test) | `altgr-l-live-proof.md` logs RightAlt(100)+L(38), toggle recognized, Groq transcript “Hello this is a dictation test 123”, exit 0. |
| Fluid, deterministic animation | PASS, minor caveat | Deterministic (no RNG); 79-frame motion sheet reports 0.158 px mean profile change/step, max 0.608 px, 2.60 s breathing. Newer round-6 idle metrics: 0.047 px mean |ΔH|, 2.70 s period, mean correlation 0.9982 but minimum 0.9856. |
| Fast iteration / render budget | PASS on measured machine | `footprint-fluidity.md`: scale-2 render 6.8–7.6 ms vs 30 ms audio frame on i7-1185G7. Low-end 15–22 ms is an estimate, not a measurement. |

## Top remaining fixes / evidence gaps (ranked)
1. **Prove desktop delivery end-to-end:** current AltGr+L run was headless; typing explicitly unavailable. Run on Wayland and confirm transcript appears in the focused application.
2. **Close the real-input gap:** test real microphone + human speech through AltGr+L, and a keyboard/layout where AltGr does not emit keycode 100.
3. **Validate fluidity on target low-end hardware:** measure actual render cadence/frame drops; current low-end headroom is estimated, and the minimum profile correlation is below the stated 0.998 per-step gate.

**Already right:** the release artifact is tiny, and the automated chord-to-transcription path has concrete successful evidence.

*Language/assets finding relies on the documented audit; this review did not independently audit every shipped dependency/artifact.