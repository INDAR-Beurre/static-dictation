# Geometry review — 7/10

| Criterion | Result | Evidence |
|---|---|---|
| Single apex, right of center | PASS | Round-9 side-by-side has one clear peak right of capsule center; raw voiced heights peak at bar 15 (42), with immediate right descent 30, 16, 8, 4. |
| Gradual left ramp | PASS | The voiced profile rises through 6, 12, 18, 28, 34, 42 before the apex; visually the left shoulder is smoother than the right. |
| Mound width | PASS | R9 broadens the >=8 px zone to roughly eight bars; the visible mound is now broader and closer to the reference's ~26% capsule width. |
| Dot-line span / capsule proportions | FAIL | Across the side-by-side, the lower capsule is noticeably shorter horizontally and taller/rounder than the reference capsule. The line stops well inside both capsule ends rather than approaching the long reference span. R9 widens the mound, but does not solve the whole-pill scale/span mismatch. |
| Fluid shape | PASS | Motion sheet reports 0.027 px mean height delta/step, 0.9988 profile correlation and 2.80 s period; no random jitter or silhouette breaking. |

## Top remaining fixes
1. Extend dots toward both capsule ends while keeping their faint outer intensity; line endpoints remain visibly too inset. `src/pill/canvas.rs:95` (verify current line number).
2. Match the reference capsule's wider, less rounded proportions/occupied size in the live composition; the lower sample remains visibly squat-width-inverted relative to the source. Check `src/pill/canvas.rs` capsule geometry and `src/pill/pill.rs` sizing.

Right: R9 fixes the narrow-needle mound; it now reads as a single asymmetric waveform with a gradual left ramp and steep right falloff.
