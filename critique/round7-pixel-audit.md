# Round 7 — pixel-audit

**Score: 6/10.**

Measurements below use the requested inclusive `PILL_BBOX=146,22,733,155` on `critique/round6-final-active.png`. The file is exactly a 2× nearest-neighbor enlargement of `/tmp/r7f/frame-0015.png` (verified byte-for-byte after decimation); so duplicated pixels are not new sampling evidence. Its ROI measures 588×134 as expected. The snapshot is trustworthy as the renderer’s raster output, but not as a continuous-shape/color measurement: bbox-derived masks and threshold gates include scale-dependent edge/bloom behavior.

| Gate | Independent result | Verdict / trust |
|---|---:|---|
| Plateau, lum≥230 | 0 px | PASS; exact for this raster, no clipped white plateau. |
| Hottest RGB / R:G | (172,235,239) / .732 | PASS; trustworthy renderer pixel, but RGB is quantized/duplicated by nearest scaling. |
| Halo R/G | .783 | FAIL vs .57–.65; weak as a “halo-only” measure: `lum<180` includes dark body and mixed bloom, not an isolated halo sample. |
| Gap contrast median / min | 2.303× / 1.071× | FAIL by nominal 3.4/2.5 gates; poor acceptance measure. Ref’s measured intra-cluster gap is ≈2.4×, so 3.4× is incompatible; minimum is vulnerable to adjacent-core/halo geometry. |
| Gap haze worst +G | +63 | FAIL nominally; not pure haze—the worst gap can be mound bloom. Numeric audit says this proxy is contaminated. |
| Midgap median / max | 1.089× / 3.602× | FAIL nominally; max especially untrustworthy (mound bloom contaminates baseline/gap). |
| Full line span | 78.91% W | PASS; useful for this stated G>90 operational definition, matches ref ≈79%. |
| Mound width | 17.69% W | Reported PASS by script, but visually/target comparison FAIL: target ≈26–28%. Script measures widths of qualifying bars at half peak, unlike ref’s ≥8px bar zone; number is definition-dependent. |
| Peak:flank | 19.50:1 | FAIL nominally; unreliable: 4px flanks are at raster/sample floor and this ratio exaggerates almost-vanished tails. |
| End-dot B−G | +40 / +40 | PASS; trustworthy for selected brightest endpoint pixels, not the complete tail hue ramp. |
| Tail lift | +11 lum | PASS by script’s unconditional `True`; numeric result misses +17..25 target, but metric is invalid for halo lift because it selects a bright dot core vs body. |

The prior `round6-numeric-audit.md` covered **round5-final-active.png**, not this final image. Its key warnings remain valid: provisional gap/haze/midgap and peak:flank probes are contaminated; do not treat the black above-peak sentinel as a halo sample. On the final image I independently obtain script-table values matching `round6-final-measurement.md` (halo .783, gap 2.303/1.071, haze +63, midgap 1.089/3.602, span 78.91%, mound 17.69%, peak:flank 19.50, end dots +40, lift +11). Compared with round5’s audit, endpoint color clearly improved (+5/+5 → +40/+40), while core, plateau and span are unchanged; gap contrast rises modestly (2.065→2.303), haze worsens slightly (+67→+63), and the metric-definition problems persist. Reference audit sets gap contrast around 2.4×, mound zone around 28%, span 79%, and brightest core G≈242; independently measuring the final file’s native 294×67 ROI yields max G=236 and zero pixels at lum≥230, so the core is a little dimmer but avoids white clipping.

## Top fixes by visual impact
1. Widen and reshape the central mound toward the reference’s asymmetric 18-slot envelope (currently reads as a narrow symmetric logo); keep full line span near 79%.
2. Reduce halo spread/neutrality around the bars; retain the no-white teal core and tune against isolated halo pixels, not `lum<180` aggregate.
3. Implement the reference’s left-rising/right-steeper envelope and soft left-biased drop shadow; do not optimize the invalid 3.4× gap gate.

**Already right:** no clipped plateau, teal hot core, ~79% line span, and dim blue endpoint dots now closely track the reference’s basic treatment.
