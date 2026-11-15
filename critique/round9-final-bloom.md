# Round 9 — final critic, BLOOM lens

**Score: 8/10.** Shape reads right; the light does not yet spread like the reference.

| Check | Result |
|---|---|
| Halo falloff reach (10% of peak) | **FAIL** — live 11 px vs ref 22 px at comparable pill widths (≈0.6× reach; live glow dies by 11 px, ref still carries soft light at 22) |
| Bar separation vs fusion | **FAIL** — ref core fuses into one 132 px mass (runs 302–434) and 58 px (197–254); live's longest run is 16 px. Gaps of 3–11 px hold between every bar, incl. the apex cluster |
| Teal, not white-gray | **PASS** — no plateau px; hot core (170,235,245) R:G 0.723; live halo R/G 0.667→0.83 vs ref 0.729→0.816 |
| Tail lift | **WEAK PASS** — end dots +40/+40 B−G; 10.7 lift vs ref +17..25 |
| Soft-shadow interplay | **PASS (visual)** — probe rows contaminated by drop shadow, so ignore the FAIL rows in `round9-measurement.md` |

## Top 2 fixes

1. `src/pill/canvas.rs:134-140` — halo stops too soon. Keep `GLOW_K 2.5` for the hot core, but add a third, wide low-alpha lobe (alpha ≈0.05–0.07, `GLOW_REACH` 34, teal-preserving) so halo G-excess decays gently to ~22 px. Do not just raise `BLOOM 0.28` globally: it buys reach but multiplies the mid-gap haze the probe already flags (+67 G).
2. `src/pill/canvas.rs:154` (`ELEM_R_MAX 1.2`) + `GLOW_A 197` — apex cluster should fuse. Scale element radius to ~1.45 and halo gain +15% for v ≥ 0.85 only, so bars 13–17 merge into one luminous mound while the dotted tails keep 3–4 px dark gaps.

## What is right

`SHAPE_MASK` (`src/pill/vecmentor.rs:188-192`) now poses a single apex at bar 15 with a 5-step left ramp and a 3-step steep right falloff, and the broadband pedestal in `src/dump.rs:100` stops the shoulders collapsing — the silhouette matches at a glance.

Skipped: real compositor capture and Wayland typing (human-only per brief). Risk: wider halo may reintroduce the mid-gap haze if it is not gated by height.
