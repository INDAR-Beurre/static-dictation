# Round-3 motion sheet (from in-process 100 ms frame dump)

- source: `live-proof/dump-test8` — 79 frames used of 79 (frame-0000.png … frame-0078.png), 100 ms/frame, G-threshold 40
- mean |ΔH| per step: 0.127 px (min 0.007, max 0.745) — slow breathing target ≲1 px/step
- consecutive profile correlation: mean 0.9987, min 0.9844 (critic gate ≥ 0.998)
- center-cluster height series: 20.2–23.7 px (±7.8% of mean)
- breathing period: 2.70 s (autocorr peak 0.55, gate ≥ 1.5 s)

## per-step |ΔH| (px)
0.74 0.17 0.10 0.04 0.02 0.02 0.01 0.04 0.07 0.05 0.11 0.18 0.12 0.18 0.18 0.14 0.17 0.25 0.14 0.12 0.11 0.16 0.11 0.15 0.17 0.16 0.18 0.15 0.10 0.13 0.12 0.04 0.02 0.02 0.01 0.04 0.07 0.10 0.15 0.14 0.13 0.22 0.14 0.16 0.23 0.17 0.14 0.10 0.11 0.16 0.11 0.16 0.16 0.14 0.20 0.12 0.15 0.11 0.07 0.03 0.03 0.01 0.01 0.06 0.09 0.08 0.18 0.12 0.17 0.18 0.12 0.15 0.23 0.15 0.17 0.09 0.14 0.13
