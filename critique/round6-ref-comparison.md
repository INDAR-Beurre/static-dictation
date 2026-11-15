# Round 6 — reference shape comparison

Compared the 18-slot census in `critique/ref-deep-measure.md` with `critique/round5-final-side-by-side.png` (reference above, current below) and the explicit crop `critique/round5-final-active.png` (bbox x146..733, y22..155). Current profile below is measured from the crop by counting consecutive rows with luminance ≥100 at each slot center; approximate slot centers follow the visible 4.08px-pitch-equivalent sequence across the current mound. These are rendered-crop pixels (not reference-scale pixels), so compare the profile shape and relative heights, not raw pixel magnitude.

| Slot | Reference height (px) | Current height (crop px) |
|---:|---:|---:|
| 1–18 | 2, 3, 3, 5, 5, 3, 3, 3, 3, 5, 8, 14, 21, 17, 12, 7, 5, 3 | 4, 6, 8, 8, 12, 16, 52, 76, 96, 76, 52, 12, 16, 12, 8, 8, 4, 4 |

## Top remaining shape gaps

1. **Mound width is much too broad.** Reference ≥8px zone is 5 slots (slots 11–15), 28% of capsule width; current ≥8px zone spans about 13 of these 18 slots (slots 4–17), roughly 2.6× as many. Existing pixel audit independently measures current mound span at 57.4% W vs reference ≈26–28% W.
2. **Wrong peak structure: current is a three-bar plateau, not one apex.** Reference has one 21px maximum at slot 13, with adjacent slots 12/14 at 14/17px. Current crop shows three equal tallest bars (slots 8–10 by profile, each 76–96px); the image visually reads as a broad flat summit. The single-apex fix must preserve `raw_field.min(1.0)`; do not add a soft knee above 1.0.
3. **Envelope is too symmetric and its shoulders are too tall.** Reference rises from 2→3→3→5→5 before the central rise, then falls 21→17→12→7→5→3: a gradual left ramp and a steeper right falloff (with small measured jitter). Current broad cluster rises abruptly into its three-bar summit and remains high across both shoulders. Reference flank-to-apex is about 3:21 (7:1); current pixel audit reports peak:flank only 2.65:1.

The target is a single central apex with that left-ramp/right-falloff asymmetry—not a 3-bar plateau. Keep the reference’s small slot-to-slot irregularities; its 18-slot values are measured, not a perfectly mirrored curve.
