# Round-3 motion sheet (from in-process 100 ms frame dump)

- source: `../../../../../tmp/r10i` — 119 frames used of 119 (frame-0000.png … frame-0118.png), 100 ms/frame, G-threshold 40
- mean |ΔH| per step: 0.032 px (min 0.000, max 0.510) — slow breathing target ≲1 px/step
- consecutive profile correlation: mean 0.9983, min 0.9772 (critic gate ≥ 0.998)
- center-cluster height series: 7.3–8.9 px (±9.3% of mean)
- per-column max step: 2.0 px at column 112 (gate ≤ 3 px ⇒ PASS: no single-column pop)
- breathing period: 2.70 s (autocorr peak 0.65, gate ≥ 1.5 s)

## per-step |ΔH| (px)
0.51 0.09 0.04 0.03 0.01 0.01 0.00 0.01 0.01 0.01 0.01 0.00 0.05 0.05 0.04 0.04 0.02 0.03 0.03 0.03 0.02 0.02 0.03 0.06 0.06 0.04 0.05 0.07 0.05 0.00 0.03 0.03 0.00 0.01 0.00 0.00 0.02 0.01 0.00 0.01 0.05 0.06 0.01 0.05 0.03 0.03 0.02 0.02 0.02 0.02 0.05 0.06 0.05 0.04 0.05 0.07 0.04 0.01 0.03 0.01 0.02 0.01 0.01 0.00 0.01 0.01 0.00 0.05 0.05 0.04 0.03 0.02 0.03 0.03 0.03 0.00 0.03 0.02 0.07 0.05 0.06 0.03 0.08 0.04 0.02 0.03 0.02 0.01 0.01 0.00 0.01 0.01 0.02 0.00 0.00 0.05 0.05 0.04 0.04 0.02 0.04 0.02 0.02 0.02 0.02 0.05 0.06 0.05 0.04 0.04 0.07 0.05 0.00 0.03 0.02 0.01 0.01 0.01
