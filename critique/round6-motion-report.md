# Round 6 motion report

Idle dump: `/tmp/r6i`, 79 frames, 100 ms/frame, G-threshold 40.

| Metric | Before (round 3 sheet / round 5 critique) | After (round 6 dump) | Gate |
|---|---:|---:|---:|
| Mean |ΔH|/step | 0.158 px | 0.028 px | ≤1 px |
| Profile correlation mean / minimum | 0.9951 / 0.9780 | 0.9990 / 0.9856 | mean ≥0.998 |
| Center range / excursion | 14.4–18.0 px / ±11.0% | 10.0–12.0 px / ±9.0% | ±8–10% |
| Breath period | 2.60 s | 2.70 s | ≥1.5 s |

Changed idle residual amplitude 0.25→0.08, wobble amplitude 0.05→0.005, and unison breath 0.09→0.07. Updated the dip-motion test threshold to reflect the intentionally much smaller residual wave; no center-cluster mask geometry change. The very first transition remains a low-correlation outlier (minimum 0.9856); subsequent breathing profile stability raises the mean above gate. One-rim-column pop is not fully established by these aggregate profile metrics; the measured max |ΔH| is 0.456 px, with no 26px pop evident.

`SHAPE_MASK` should remain mirrored: `critique/ref-deep-measure.md`'s reference asymmetry is described as an illumination/lighting effect, while this simulation's symmetry tests and shared bands rely on mirrored silhouette geometry. Model the asymmetric lighting in the renderer, not the physics mask.

Checks: `cargo test` 95 passed / 0 failed / 2 ignored; `cargo fmt --check` passed; `cargo clippy --all-targets -- -D warnings` passed.
