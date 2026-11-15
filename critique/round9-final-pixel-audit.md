# Round 9 — final pixel audit

**7/10**

PIL/NumPy read of `critique/round9-active.png`, `PILL_BBOX=146,22,733,155`; capsule 586×124.

| Reference feature | Measured | Verdict |
|---|---|---|
| Plateau, lum ≥230 | 0 px | PASS |
| Hot core / R:G | (170,235,245) / 0.723; max G 245 | PASS |
| Apex position | x 466 → 54.4 % W | PASS (ref 54 %) |
| Asymmetry | left ramp 5 steps, right falloff 4 | PASS |
| Waveform span | 78.91 % W | PASS (ref 79 %) |
| End dots B−G | +40 / +40 | PASS |
| Tall zone (>17 % H) | 4 bars, 54 px = 9.2 % W | PASS (ref ≈11 %) |
| **Apex height** | 84 px = **68 % H** | **FAIL** (ref 44 %) |
| **Peak:flank** | 22:1 | **FAIL** (ref 7:1) |
| Capsule aspect | 4.73:1 | **FAIL** (ref 3.88:1) |
| Halo / gap / haze / midgap | 0.778 / 2.400 / +67 / 1.042 | UNTRUSTWORTHY |

**Trust:** plateau, core, span, end dots, apex position are exact. Halo 0.778 aggregates dark body — isolated 0.658 meets the reference edge. Gap/haze ROIs include shadow; the ≥3.4× gate exceeds the reference's ≈2.4×. Peak:flank is directionally right but raster-floor-inflated. The script mound target (~26 %) is the wrong definition: 28 % is the ≥8 px *zone*, while the reference's half-peak mound is ~17 % W — so round 9's 14.97 % is a pass and the broadening worked.

**Top 2 fixes**
1. Lower the apex, don't lift flanks: `src/pill/vecmentor.rs:192` (`SHAPE_MASK`) has 1.00 at bar 15 vs 0.20–0.30 shoulders; `src/pill/canvas.rs:389` maps that to 84 px on a 124 px capsule. Target ~54 px (44 % H) → peak:flank ≈11:1.
2. Slim capsule toward 3.9:1: `src/pill/canvas.rs:367-369` (`hw_c` 148, `hh_c` 36) gives 4.1:1 logical, 4.7:1 with bloom. Cut width ~15 %.

**Right:** single apex at 54 % W, gradual left ramp, steep right falloff, unclipped teal core, 79 % span, +40 end dots.

*Skipped: motion-gate re-derivation and live Wayland/audio paths.*
