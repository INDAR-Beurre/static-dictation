Score: 4/10

| PIXEL-AUDIT row | Result | Measured (explicit crop x146..733, y22..155) |
|---|---|---|
| Plateau | PASS | 0 px with luminance ≥230; 0 rows contain such pixels |
| Hot core | PASS | peak RGB (173,236,235); G=236; R/G=0.731 (target ≥0.70) |
| Halo hue | FAIL | halo-mask mean R/G=0.713, B/G=1.112 (target R/G 0.57–0.65); too red/white |
| Gap contrast | FAIL | median 1.72×, minimum 1.17× (target ≥3.4× / ≥2.5×) |
| Gap haze | FAIL | worst gap +22 G over body (limit +20) |
| Midgap | FAIL | median 1.27×, max 1.73× vs neighboring bar (limit 1.05×) |
| Span | PASS | 57.4% mound span (x≈392–487; target mound ≈26% W); full dot line 78.9% W |
| Cluster/mound | FAIL | mound 57.4% W, over 2× target ≈26%; broad/fused rather than reference envelope |
| Peak:flank | FAIL | peak G 236; flank median G≈89 → 2.65:1 (target ≈7:1; flanks 7–11% cap H) |
| End-dot blue | FAIL | left end RGB (21,25,35), B−G=+10; right RGB (17,21,32), B−G=+11 (target ≥+30) |
| Tail lift | FAIL | +73…+109 luminance over body (target +17…+25) |

Top 3 fixes:
1. Reduce bloom kernel/gain and remeasure halo: `src/pill/canvas.rs:132-149,612-638`.
2. Narrow the over-wide cluster; current 57.4% mound vs 26% reference: `src/pill/canvas.rs:93,398-409`.
3. Increase left/right end-dot blue bias to B−G≥30: `src/pill/canvas.rs:105-106,640-678`.

Gap/midgap/haze/lift use the corrected-mask round-4 measurements; halo ratio uses crop pixels with G=55..179 (proxy halo mask).