Score: 5/10

- **PASS — core hue:** hottest G=236, RGB(173,236,235), R/G=0.733 (target ~0.72); plateau pixels at lum≥230 = 0 (target zero white pixels).
- **FAIL — halo hue:** measured halo R/G 0.72–0.74, above target 0.57–0.65; it reads pale cyan rather than teal.
- **FAIL — tail blue:** endpoint B−G=13..33; requirement ≥+30 at both ends is not met consistently.
- **FAIL — rim:** +23 lum, exceeds +10..+20 target.
- **PASS — body:** (18,23,33), within (16,20,30) ±6.
- **FAIL — banding:** not measured; cannot establish the no-banding criterion from provided metrics.

Top 3 fixes:
1. `src/pill/canvas.rs:622–631` — adjust halo tint toward R/G 0.57–0.65; current 0.72–0.74 is too pale.
2. `src/pill/canvas.rs:657–671` — ensure both endpoint tails meet B−G ≥+30, especially the failing low end.
3. `src/pill/canvas.rs:161–163, 559–563` — reduce rim lift from +23 to +10..+20 and verify banding on the full render.