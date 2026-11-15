# Round 4 — BLOOM critic

**Score: 3/10.** The core is clean, but this is not reference-faithful bloom: active render visibly halos/fuses the mound; idle render is not a quiet dot line. Probe regions are explicitly untrustworthy, so treat the supplied measurements as directional, not validated pixel truth. Even so, every reported bloom gate misses badly: gap R/G .72–.74 (target .57–.65); gap contrast 1.72 median / 1.17 minimum (≥3.4 / ≥2.5); haze +22 G (≤20); midgap 1.27× median / 1.73× max (≤1.05×); tail lift +73–109 lum (target +17–25). Plateau 0 and hottest R/G .73 pass; do not trade those away.

**PASS:** no ≥230 plateau; hot core hue. **FAIL:** halo hue, gap separation, haze, midgap isolation, tail restraint, and halo falloff (reported tail glow lift far above target). Visuals corroborate over-bright idle dots and a conspicuous active halo.

**Top fixes (in order):**
1. Stop idle dots emitting: `GLOW_IDLE` path at `src/pill/canvas.rs:450-455` applies hum to every peak-weighted bar even at silence. Gate hum to the intended posed center/quiet amount; preserve the hum requirement, not a glowing full dot train.
2. Remove or sharply reduce `glow.min(1.0)` at `src/pill/canvas.rs:629-638`: with one-source MAX it is not a useful stack limiter; it masks over-strong source weight and leaves excessive bloom. Retune the underlying weight/gain instead.
3. Sample bloom before the hot/white lerp and tighten kernel/gain: glow currently samples `wave_color` at `src/pill/canvas.rs:616-623` but still uses post-field `bar.dim`; `GLOW_K=2.5`, reach 18, `BLOOM=.28` (`src/pill/canvas.rs:132-149`) spread excess into gaps/tails. Reduce kernel/radius and bloom gain against measured 1–4px falloff; keep core hot teal.

Do not claim numeric passes from the current unreliable regions; rerun the probe with corrected masks.