# Round 8 final pixel audit

**7/10**

PILL_BBOX = `(146,22,733,155)` on `critique/round8-active.png` (880×190), measured independently with PIL/NumPy.

| Reference feature / gate | Result |
|---|---|
| Bright plateau (lum ≥230) | **PASS** — 0 px; no blown-out plateau. Hottest pixel `(170,235,245)` (R:G 0.723). |
| Capsule body | **PASS** — representative dark-body median RGB `(19,23,33)`, appropriately near-black. |
| Waveform horizontal span | **PASS** — 78.91% of pill width (463 px), close to ~80% reference. |
| End dots, B−G | **PASS** — left +39, right +40; both exceed +30. |
| Tall-bar mound width | **FAIL** — probe reports 15.31% W (~90 px) vs ~26% reference (~153 px): central tall cluster is too narrow. |
| Peak:flank | **FAIL** — 22:1 vs ~7:1 reference; the main peak over-dominates its flanks. |
| Halo / gap & haze probes | **PROVISIONAL** — halo R/G 0.784 is contaminated by dark body pixels (isolated halo is 0.658); gap contrast’s ≥3.4× gate is stricter than reference’s ~2.4×; gap/haze/midgap ROIs include shadow. Do not count these as failures. |

**Top fixes**
1. Broaden the tall-bar cluster toward the reference’s ~26% pill width, retaining the single apex. `src/pill/vecmentor.rs:192` (`SHAPE_MASK`).
2. Reduce peak-to-flank contrast from 22:1 toward ~7:1 by lifting neighboring bar heights without restoring a multi-bar plateau. `src/pill/vecmentor.rs:192`.
3. Keep the revised voiced asymmetry, but tune the mound’s shoulders and compare against the reference crop at the same scale before further palette changes. `src/pill/vecmentor.rs:22`.

**Already right:** the dark body, non-clipped teal core, broad overall waveform span, and cool end dots are all close to the reference; the voiced profile now has one apex rather than the old forced mirror.
