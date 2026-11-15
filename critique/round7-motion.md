# Round 7 — motion lens

**Score: 7/10.** The idle breathing is documented as calm and deterministic, but the requested active GIF is too short and too coarsely sampled to establish a fluid, alive cycle; its transitions visibly/quantitatively include a large initial jump. A 10/10 would require the reference-like slow breathing without perceptible frame pops, verified over a full cycle.

| Check | Result | Verdict |
|---|---:|---|
| Idle mean ΔH / step | 0.047 px (100 ms, 119 frames) | PASS: gradual breathing |
| Idle profile correlation | mean 0.9982; min 0.9856 | PASS mean; isolated first-frame transient, not perfectly smooth |
| Idle excursion / period | ±9.1%; 2.70 s | PASS: visible, slow cycle |
| Determinism | golden-angle sine jitter; no RNG (documented) | PASS |
| Active GIF cadence / coverage | 15 frames × 200 ms = 3.0 s, only ~1.1 cycles | FAIL: insufficient to judge repeatability; 200 ms sampling can hide pops |
| Active GIF sampled silhouette | fixed ROI, G>50: mean |ΔH| 0.523 px/step; max 84 px | FAIL: large onset/threshold transition; likely capture/pose change, not evidence of a subtle animation defect |

**Top fixes, by impact**
1. Capture a fresh 100 ms active/idle frame dump over ≥3 full periods; the current 15-frame, 200 ms GIF is inadequate for detecting single-frame rim-column pops or judging cycle continuity.
2. Investigate the GIF's first-frame pose/ROI transition (measured max 84 px against a 0.523 px mean); ensure animation begins from a settled state or preserve the actual transition in a longer capture.
3. Keep the measured idle sine amplitudes and period; add a direct per-column maximum-step/pop assertion across the full dump so no localized rim spike can hide in aggregate profile correlation.

**Already right:** idle motion has a measured ±9.1% excursion, 2.70 s period, high profile correlation, and deterministic (non-random) motion.
