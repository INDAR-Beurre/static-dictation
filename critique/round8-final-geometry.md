# Geometry review — 7/10

| Reference criterion | Result | Evidence |
|---|---|---|
| Single dominant apex, slightly right of center | PASS | Raw active render has a single 84 px maximum at x≈469; neighboring bars are 60, 28, 12 px. The asymmetry and steep right falloff are visible in the side-by-side. |
| Gradual left ramp | PASS | From the supplied raw-frame measurements: 4, 4, 10, 18, 30, 42 px up to the apex; clear staged ramp, not the former symmetric cluster. |
| Dot-line span | FAIL | Active waveform extent is x=208–655 (448 px) in an 880 px render: 50.9% of image width. The reference line occupies about 80% of its pill width, and the measurement sheet reports the rendered span at 78.91% W; the image-based measure therefore needs a consistent pill-relative ROI, not full-image width. Visually, the dots in the live pill are markedly shorter than the reference's nearly end-to-end line. |
| Mound width / bar density | FAIL | The 84 px peak spans a compact central cluster; the measured mound is 15.31% W versus reference ~26%. Bars step 15 px between centers at the peak, leaving a narrow, logo-like cluster rather than the broader reference mound. |
| Capsule proportions | FAIL | Side-by-side shows live capsule approximately 590×140 px (4.2:1), while the reference capsule is roughly 685×180 px (3.8:1): the live pill is too narrow and shallow relative to the source. |

## Top fixes
1. Widen the active mound while preserving its single apex and asymmetric mask; target the reference's ~26% pill-width mound. `src/pill/vecmentor.rs:197-200`.
2. Extend dot-line span to the pill-relative reference target (~80%); inspect `BAR_SPAN_FRAC` and its rendering geometry in `src/pill/canvas.rs:95`.
3. Correct capsule width/height ratio and occupied scale against the reference crop before changing bar heights; live is visibly slimmer. Check pill sizing/placement in `src/pill/canvas.rs`.

Already right: the R8 asymmetric silhouette has fixed the prior mirrored, multi-apex appearance, and the left-ramp/right-drop read matches the reference.
