# Round-6 numeric audit — `round5-final-active.png`

Measured from `critique/round5-final-active.png` with PIL/numpy, inclusive capsule bbox **x146..733, y22..155** (W=588, H=134). Waveform profile uses the probe’s central 40% band, y61..114. Threshold-based metrics below follow `critique/round3-probe.py`; this is a single active still, not a frame-dump.

| Gate | Measurement | Verdict | Trust / caveat |
|---|---:|---|---|
| Plateau (lum ≥230) | 0 px; 0 rows | PASS (≤40 px, ≤5 rows) | Trustworthy within specified bbox. |
| Hot core | max RGB (172,235,239), G=235, R/G=.732 | PASS (G≥242? **FAIL**; R/G≥.70 PASS) | Exact G gate is inconsistent: round-3 probe only gates R/G; design-spec says G≥242. Under design-spec numeric threshold this is FAIL. |
| Halo hue | “above peak” sample (0,0,0), R/G undefined; adjacent-cluster gap proxy (23,30,40), R/G=.767, B/G=1.33 | FAIL / indeterminate | Above-peak band is empty because probe seeks G≥230 but peak max is 235 in the 40%-band profile while selected central column/band does not provide that window; black sentinel must not be interpreted as a real hue. Gap proxy is dim, mixed with capsule body, and not a clean halo sample. |
| Gap contrast | median 2.06×, minimum 1.18× (22 clean gaps) | FAIL (<3.4× / <2.5×) | Trustworthy only for probe-defined clean gaps; contrast uses adjacent column core over gap max and is sensitive to coarse dot sampling. |
| Gap haze | +23 G over body | FAIL (>20) | Trustworthy as worst gap max relative to estimated body G=22; can include halo and is not pure haze. |
| Midgap / single-bar | median 1.15×, max 1.88× | FAIL (>1.05×) | Weak: Q1 baseline derived from clean gaps, while gaps near the fused central mound are contaminated by its bloom. |
| Full dot-line span | 79.3% W (x206..671 at profile G>45); margins 10.2% / 10.5% | span PASS (~80% ±5); right margin FAIL (<13%) | Trustworthy as emission span under explicit G>45 threshold; threshold controls dim tail endpoint. |
| ≥8px-tall bar zone (separate from full span) | 90 columns with G≥120 in ≥8 rows: x376..515 = 23.8% W; at G≥100: 92 columns, 24.5% W | PASS against ~26% ±6% | Better operational mound measure than the fused ON-run; still threshold-dependent, and this screen-space bar zone is not the same as the reference’s fitted waveform envelope. |
| Peak:flank | 26.5:1 (106px vs 4px median) | FAIL (~7:1 ±2.5) | Not fully trustworthy: 4px flanks approach the 2× raster/sample floor; ratio exaggerates near-vanishing tails. |
| End-dot blue | left B−G=+5 (87,124,129); right +5 (87,124,129) | FAIL (≥+30) | Trustworthy for the brightest pixel in each endpoint run; endpoint run itself depends on G>45. |
| Tail lift | +71..+102 G over body | FAIL (target +17..25, probe tolerance ±3) | Not trustworthy as “tail glow”: endpoint-run max picks the luminous dot core, not halo lift; compares it to median body and thus conflates dot brightness with tail halo. |

## Resolve the 57.4% claim

**57.4% is not the full dot-line span and is not the ≥8px bar zone.** The full line is about 79% W (probe: 79.3%; direct profile threshold variants G>45..80 yield 41.8%..28.2%, demonstrating threshold sensitivity). The old 57.4% was the width of a loosely defined bright/mound region from the previous render/probe, and the round-5 pixel-audit mislabeled it as mound width. On this final image, the probe’s fused G>45 ON-run is 464/588 = **78.9% W** because halo joins the line and central bars into one run; it is therefore unsuitable as a mound measure. Separately measuring columns with at least 8 rows G≥120 gives **23.8% W** (90 columns), close to the ~26% target. Thus geometry’s “57.4% mound” cannot establish the mound is too narrow, nor can it be used as the 80% full span; it conflated quantities. The reference target is full dot-line span ≈80% and central tall mound ≈26%—distinct measures.

## Overall

Against the probe’s numeric thresholds, clear failures are gap contrast, haze, midgap, end-dot blue, peak:flank, and tail lift; halo hue’s above-peak statistic is invalid here. Core G fails the design-spec ≥242 criterion but passes the probe’s R/G criterion. Geometry span is near target, while the ≥8px bar zone is near target; the earlier 57.4% statement is not a valid measurement of either. This audit does not cover motion, idle, live H3 proof, or remaining appearance criteria.
