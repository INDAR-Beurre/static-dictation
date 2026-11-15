# Round 8 — Motion critique

**Score: 8/10**

| Reference comparison | Result | Evidence |
|---|---|---|
| Idle motion is fluid, not random | PASS | 119-frame sheet: mean ΔH 0.032 px/step, profile correlation 0.9983, ±9.3% breathing excursion, 2.70 s period. Tiny, coherent changes read as slow breathing rather than jitter; no freeze. |
| Idle columns avoid pops | PASS | Maximum per-column step 2.0 px, below the 3 px gate. GIF frame comparison confirms 99/99 adjacent steps change (no duplicate/frozen steps). |
| Voiced motion avoids abrupt pops | PASS, with caveat | 58/58 adjacent voiced GIF steps change. The first entrance jump is intentionally excluded by the specified gate; assess settled speech separately. |
| Voiced waveform matches reference | PARTIAL | Asymmetric single apex is fixed, but side-by-side shows the live cluster narrower and more sharply peaked: measured peak:flank is 22:1 versus reference ~7:1. |
| Pill framing / proportions | FAIL | In the side-by-side, live pill is visibly much shorter relative to its render width than reference; current capsule half-extents are documented as 74/18 in `src/pill/canvas.rs:364`, while reference silhouette is broader/taller. Verify comparison crop/device scale before altering geometry. |

### Top fixes
1. Broaden and soften the illuminated mound: peak:flank 22:1 vs reference ~7:1 (`critique/round8-measurement.md`). Increase shoulder energy/whisper while retaining the single off-center apex; do not revert to a mirrored waveform.
2. Recheck capsule scale against an aligned crop before changing sizing. Current displayed comparison has notably different pill proportions; ensure renderer and reference are normalized to the same pixel scale, then tune capsule height/width if mismatch persists (`src/pill/canvas.rs:364`).
3. Remove stale symmetric-shape comments in `src/pill/vecmentor.rs:3-5, 47-52, 182-190`; they contradict the R8 asymmetric mask and per-band voiced behavior, making future motion tuning error-prone.

Already right: idle breathing is exceptionally stable and perceptible without flicker, and the voiced bars now have the reference's single, right-of-center apex rather than a mirrored cluster.