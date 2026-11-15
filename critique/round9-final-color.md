# Round 9 — final color critic

**7/10**

Measured with PIL/NumPy on `critique/round9-active.png` (880×190, PILL_BBOX 146,22,733,155) and `critique/reference.png` (pill crop 419,15–606,63). Round 9 was geometry/motion; the color verdict is unchanged inside a better silhouette.

| LENS criterion | Result | Evidence (reference → renderer) |
|---|---|---|
| Core teal / hot hue | **FAIL** | Bright-mask median **(197,218,232)** R:G **0.936** → **(169,233,240)** R:G **0.729**. Hot pixel (233,241,252) → (170,235,245): the peak is missing ~60 levels of red, so the core reads mint-cyan where the reference reads pearl ice-cyan. B:G already matches (1.04 vs 1.05) — red is the whole error. |
| Halo tint | **PASS** | Isolated halo R/G **0.656** → **0.67**; halo stays teal, no white bloom. Relative glow area is *smaller* than the reference (5.7% vs 18.9%), so the bloom is not oversized. |
| End-dot blue bias | **PASS** | B−G **+40** both tails; left dot (87,124,164) is steel-blue. Slightly bright/early on the left ramp (G 124 at the 3rd dot vs the reference's 46 at the same slot), but the hue is right. |
| Rim / crease | **PASS (weak)** | Top edge (31,36,46) is only +13 over body (18,22,32) — a soft rim, not the round-8 bright keyline. Still bluish; no crease highlight. |
| Body dark-on-dark | **FAIL** | Body median **(40,45,48)** neutral charcoal → **(18,22,32)**, ~22 levels darker and B−G +10 navy-cast. Partly correct glass-over-navy physics, but the reference's material depth is lost and the tint is too cool. |

## Top 2 fixes

1. **Warm the peak.** `src/pill/canvas.rs:426-428` — the peak tinge `(171,234,233)` caps red; move it to ≈`(232,238,246)` and lift `GRAD_C` (`src/pill/canvas.rs:186`) from `(140,220,222)` to ≈`(178,222,230)`. Target bright-mask R:G ≈0.90–0.94.
2. **Neutralize the body.** `src/pill/canvas.rs:183` `BG=(16,20,30)` → ≈`(24,27,30)` (drives body median toward (26,30,33), B−G ≈+3). Re-check rim contrast after, since RIM rides the same base.

**Already right:** the icicle silhouette, single teal apex right of centre, coherent teal→cyan→steel-blue tails, +40 blue end dots, and a non-white halo all match the reference closely; only the peak's red deficiency and the body's navy cast remain.
