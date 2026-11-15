# Round 8 — Bloom critic

**Score: 7/10**

| Reference criterion | Result | Evidence |
|---|---|---|
| Teal rather than white-gray | PASS | Active render's selected halo pixels average RGB (18.1, 30.1, 54.4), R/G **0.603**; isolated-halo measurement in the round-8 report is **0.658**. Core is bright cyan, but not a broad white slab. |
| Falloff | PARTIAL | Glow is restrained around the bars, but the bright central cluster still spills into a single soft patch; the reference keeps more distinct luminous bars. |
| Clean gaps / no fusion | PARTIAL | Bars remain visibly discrete in the side-by-side, including the central five; the halo partially bridges the narrow gaps between those bars. Outer dotted tails stay clean. |
| Tail lift | PASS | Measured tail lift is **+10.7** over body, and the faint teal/blue dots remain legible without a broad luminous tail. |
| No stray glow / balanced bloom | PASS | Active-render teal threshold spans x=208–531, y=42–125; visible bloom is localized to the waveform rather than washing the pill. |

## Top fixes

1. **Reduce the central bars' halo radius or intensity slightly** so adjacent halos do not merge into a continuous patch. Keep the bright core; target cleaner separation rather than dimming the whole waveform. Inspect the glow/raster path in `src/pill/` (the side-by-side shows the join most clearly at the center cluster).
2. **Tune the halo toward the reference's more saturated teal.** The isolated halo R/G is **0.658**, just above the stated 0.57–0.65 reference range; lower red relative to green modestly, without shifting the cyan core toward green.
3. **Keep tail bloom conservative.** Tail lift is **+10.7** (below the reference's +17–25), but increasing it globally risks haze in gaps; if raised, confine the extra lift to the faint end dots.

**Already right:** the per-bar silhouette stays crisp, the apex glow reads cyan-teal, and the glow does not overwhelm the dark capsule.
