# Round 8 — Color critic

**7/10**

| Reference criterion | Result | Evidence (reference → renderer) |
|---|---|---|
| Core teal / hot hue | PASS, with mismatch | Bright cyan mask median RGB is **(153,183,206)** → **(138,202,223)**. Renderer is more saturated cyan/green and less blue; peak is capped at (170,235,245), below the reference bright maximum (247,246,255), so it reads a little flatter/less luminous. |
| Halo tint | PASS | Renderer glow visibly stays teal rather than turning white. The isolated halo R/G is **0.658**, just above the stated 0.57–0.65 target; aggregated 0.784 is body-contaminated and not a valid failure. |
| End-dot blue bias | PASS | Right tail reads steel-blue, not neon; measured B−G is **+40**, exceeding the +30 target. |
| Rim / crease | FAIL | Capsule outline is conspicuously cool/teal against the dark body. Reference edge is a soft dark crease, explicitly not a bright keyline; current `RIM=(20,24,27)` adds a pale outline contribution. |
| Body dark-on-dark readability | FAIL | Reference body median dark-neutral RGB **(40,45,47)**; renderer body median **(19,23,33)**. Renderer is roughly 20–22 levels darker and bluer, losing the reference’s charcoal/glass separation from the navy backdrop. |

## Top 3 concrete fixes

1. **Lift and neutralize the body.** In `src/pill/canvas.rs:~160` (`BG`) / body shading, move the visible capsule from median (19,23,33) toward the reference’s ~ (40,45,47); preserve the navy exterior. The larger dark body/backdrop delta is the clearest color mismatch.
2. **Remove the light edge, retain a dark crease.** In `src/pill/canvas.rs:~139` (`RIM`) and `:~144` (`CREASE`), reduce the positive rim contribution (currently (20,24,27)) substantially; let the soft subtractive crease define the edge, as in the reference.
3. **Tune hot cyan toward the reference.** In `src/pill/canvas.rs:~207` (`GRAD_C`) and peak stop, shift the center ramp slightly less green and brighter/whiter; current bright-mask median (138,202,223) is greener and dimmer than reference (153,183,206). Keep the blue-biased tails intact.

**Already right:** the teal-to-cyan-to-steel-blue progression is coherent, and the isolated halo remains teal without a white bloom.
