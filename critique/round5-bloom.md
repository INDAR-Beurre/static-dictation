Score: 3/10

- **PASS:** Core cap 0 pixels at lum ≥230; max G 236 (RGB 173,236,235), so the bloom does not saturate the core. Midgap/halo field uses a max-source rather than sum (`src/pill/canvas.rs:612-617`), avoiding unbounded stacking.
- **FAIL:** Supplied round-4 audit lacks valid numerical probe results (the prior probe report says masks/autodetection fail); therefore exact halo falloff, gap contrast, haze, midgap ratio, tail lift, inversion, and saturation gates are **unverified**, not passes. Do not reuse stale round-4 critique numbers.
- **FAIL (visual):** In the new active render, bloom visibly fills the mound and brightens tails; idle render also retains conspicuous halo across the posed cluster. This conflicts with discrete gap separation and restrained tail glow. Known clean cap and teal hue do not compensate for bloom isolation.
- **FAIL (implementation risk):** 18px reach (`src/pill/canvas.rs:137`) permits long tails; `glow.min(1.0)` (`src/pill/canvas.rs:636`) does not reduce a single strongest source, so it is not evidence of midgap control. `BLOOM=0.28` (`src/pill/canvas.rs:139`) and `GLOW_IDLE=0.30` (`src/pill/canvas.rs:149`) need a valid probe before numerical claims.

Top 3 fixes:
1. Repair probe regions/masks and report four halo offsets + gap median/min + haze and midgap ratios: `critique/round4-probe-report.md:1-7`.
2. Tune decay/reach to measured 0.46/0.34/0.28/0.25 falloff, then verify no inversion: `src/pill/canvas.rs:132-139`.
3. Retune tail and idle emission against +17..+25 lift and haze ≤+20 G, preserving the teal core: `src/pill/canvas.rs:452-457`, `src/pill/canvas.rs:631-640`.
