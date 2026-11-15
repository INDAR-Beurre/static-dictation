# Round 7 — bloom lens

**Score: 6/10**

| Check | Reference (top) | Current (bottom) | Verdict |
|---|---:|---:|---|
| Halo reach above peak | Visible teal fringe about 2–3 px before body; local body G≈56, fringe G≈75 | G falls 236 core → 64 at first pixel above thresholded core, then 56/50/45 and ~41 baseline across the next several pixels | FAIL — reach exists, but mostly a dim, neutral-looking skirt; the bright fringe is abrupt and narrow |
| Gap isolation | Lit gaps are allowed: reference intra-mound core/gap contrast ≈2.4× | Measured 2.303× median / 1.071× minimum | MIXED — median is reference-like; minimum is near fusion, visibly blurring bars together |
| Gap haze / midgap | Ref gaps remain lit, but retain distinct bar edges | +63 G worst gap haze; midgap 1.089× median / 3.602× max | FAIL — local hot spots spill substantially; numeric max gate is stricter than reference, but visible center fusion remains |
| Halo hue | Target R/G 0.57–0.65 | Measured halo R/G 0.783 | FAIL — glow is too neutral/white relative to the teal reference |

Top fixes, ranked by visual impact:
1. Reduce bloom in the near-field gaps (especially around the central bars) until neighboring bars separate; preserve the roughly 2.4× reference-like median contrast rather than chasing the probe's ≥3.4× gate.
2. Tint the halo teal, not just the emitting cores: bring halo R/G down from 0.783 toward 0.57–0.65 without bleaching the hot core.
3. Shape the falloff: retain a soft 2–3 px visible fringe like the reference, but attenuate the longer low-level skirt and local gap peaks; tune reach/decay from pixels, not the nominal 18px reach alone.

Already right: the halo is present, visibly teal-adjacent, and the core has no white plateau (max RGB 173,236,239; zero pixels at lum ≥230).
