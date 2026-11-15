# Round 8 — final taste

**7/10**

| Reference comparison | Result |
|---|---|
| Single, off-center voiced peak rather than a symmetric logo | **PASS** — the live render now has a single cyan apex and asymmetrical falloff; the old mirrored badge read is gone. |
| Fine, long, low-contrast dotted tails | **FAIL** — live tails are visibly short and stop abruptly; reference's faint line runs almost end-to-end. |
| Pill silhouette and finish | **FAIL** — live capsule reads flatter, more outlined, and more uniformly lit; reference is softer, fuller, and more integrated with the sky. |
| Animation | **PASS** — measured idle breathing is slow and smooth: 2.70 s period, 0.032 px mean height change/frame, correlation 0.9983. |

## Top 3 remaining perceptual fixes

1. **Restore the long, delicate leader line.** Reference spans roughly 80% of its capsule width with tiny, nearly hairline dots. The live row is sparse/short, with prominent separated dots and visible gaps before the central bars. Extend and reduce their visual weight. Likely tune `src/pill/canvas.rs:488-500` and `:634-669` (dot span, pitch, radius/alpha); the side-by-side makes the length discrepancy obvious.
2. **Soften the capsule boundary and increase its volume.** Live lower capsule has a stronger, continuous dark outline and a flatter, near-uniform fill; reference's boundary is blurrier with a more dimensional, soft shadow. Reduce edge contrast and refine the body/shadow treatment in `src/pill/canvas.rs:509-552` (especially the rim/sheeen and shadow), guided by side-by-side rather than the provisional ROI gates.
3. **Make the voiced mound less needle-like and more reference-scaled.** The asymmetry is a major improvement, but the live apex dominates with a steep, narrow central cluster; reference has a broader luminous mound and gentler shoulders, plus longer low bars on the left. Revisit the mask and its rendered bar-height mapping at `src/pill/vecmentor.rs:197` and `src/pill/canvas.rs:402-438`; do not restore forced mirroring.

**Already right:** the dark teal-blue capsule palette, rounded cyan bars, one unmistakable off-center apex, and restrained fluid breathing now read as the reference's dictation pill—not a generic symmetric logo.
