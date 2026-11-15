# Round-5 critic scores (on the pre-single-apex render, commit 2ae527d)

| Lens | Score | File |
|---|---|---|
| Color | 5/10 | critique/round5-color.md |
| Geometry | 4/10 | critique/round5-geometry.md |
| Bloom | 3/10 | critique/round5-bloom.md |
| Motion | 7/10 | critique/round5-motion.md |
| Taste | 7/10 | critique/round5-taste.md |
| Pixel audit | 4/10 | critique/round5-pixel-audit.md |
| Hard constraints | 9/10 | critique/round5-constraints.md |
| **Average** | **5.6/10** | — |

Not 10/10, so round 6 was launched with 7 DeepSeek V4.1 Flash (routing group) agents:
canvas fixer, motion fixer, measurement script, ref comparator, shadow spec, numeric audit,
constraints QA.

## What the critics agreed on
- PASS: hottest core RGB(173,236,235), R/G 0.73, zero white plateau pixels; body (18,23,33);
  no saturation; capsule proportions; deterministic 2.60 s breathing; ~40% idle pose;
  real Groq transcript via AltGr+L; binary 3.58 MiB (14× under 50 MB); all Rust; ~4× render headroom.
- FAIL: halo R/G 0.71–0.74 (target 0.57–0.65); end-dot B−G +10..+11 at both ends (target ≥+30);
  rim +23 (target +10..+20); tail glow lift +73..109 (target +17..+25); gap contrast 1.72×/1.17×
  (target ≥3.4×/≥2.5×); gap haze +22 G (≤+20); midgap 1.27×/1.73× (≤1.05×);
  flat three-bar top; active mound too broad/regular; weak crease + shadow;
  profile corr 0.9951/0.9780 (≥0.998); center excursion ±11.0% (±8–10%).
- Measurement caveat: several numeric rows came from a broken general probe
  (critique/round4-probe-report.md). `scripts/round6-measure.py` replaces it with an explicit bbox.

## Already fixed before these scores landed
- `src/pill/canvas.rs`: `let field = raw_field.min(1.0);` — removed the soft knee that pushed
  every central slot to the same max height. Measured bar heights went from
  `[…, 60, 92, 96, 92, 60, …]` (3-bar plateau) to `[…, 52, 76, 96, 76, 52, …]` (single apex).
- `critique/round5-final-*.png` regenerated from that binary, including a TRUE idle dump
  (the earlier `critique/round4-idle-render.png` was accidentally a voiced frame).
- `glow_falls_off_with_distance` re-anchored honestly to `g_near > g_far + 4` (near 30 G, far 24 G)
  after the field clamp dimmed the halo. Gates: 95 passed / 0 failed / 2 ignored, fmt + clippy clean.
- Commit `54eb47c`.

Open: the ≥8px-tall bar zone vs full dot-line span were conflated in the round-5 numbers
(57.4% W was quoted both as "too narrow vs 80% span" and "2× the 26% mound target").
The round-6 numeric audit resolves them separately.
