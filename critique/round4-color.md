# Round-4 color critic — 4/10

Color is still visibly off-reference: the active core is close to teal, but the halo turns nearly white, the tail is not consistently blue, and the glow swamps the body. The pill body itself is close.

- **FAIL — core:** hottest (172,235,239), R/G 0.73 is near target, but reference requires zero white pixels; verify the whole render, not just the hottest sample.
- **FAIL — halo:** gap R/G 0.72–0.74 vs target 0.57–0.65; too white.
- **FAIL — tail blue:** left endpoint B−G +4 vs required ≥+30; right endpoint passes at +91, but both ends must pass.
- **FAIL — bloom lift:** tail +73–109 luminance vs +17–25 target; overwhelms the quiet dotted line.
- **PASS — body:** (18,23,33), within ±6 of (16,20,30).
- **FAIL — rim:** +23 lum vs target +10–20; too bright.
- **FAIL — banding:** not established by supplied measurements; do not claim a pass.

Top fixes (`src/pill/canvas.rs`):
1. Lines 622–631: reduce bloom contribution substantially and tune its color mix so the halo gap returns to teal R/G 0.57–0.65; current lift is far over target.
2. Lines 657–671: fix left-dot blue bias too (not only the `u > 0.5` right flank); clamp/tune both endpoint hues to B−G ≥+30 without pushing the right tail excessively blue.
3. Lines 161–163, 559–563: reduce rim/specular contribution until measured rim lift is +10–20 lum; keep peak core teal and confirm zero white pixels across the full render.
