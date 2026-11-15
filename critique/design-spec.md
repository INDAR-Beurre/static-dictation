# Round-1 Scoreboard — Pill Fidelity vs `critique/reference.png`

Six critic lenses, avg **3.8/10**. Root cause is one causal chain: unnormalized glow sum erases
gaps → dots emit (no threshold) → per-frame max-norm lifts silence to full height → midpoint
bright curve never reaches top palette → dark green-gray body on navy host disappears.
Quantified targets for every number below live in `critique/targets.md` (this file's sibling).

## Scoreboard

| Lens | Score | Top finding | Accepted fix (owner file) |
|------|-------|-------------|---------------------------|
| COLOR | 3 | Peak G≈54-class mids: `bright = 0.5+0.5·v` keeps everything at half palette; live peak G 132 vs ref 242 | Bright curve `0.35+0.65·v^0.6`, env floor 0.65, white-hot lerp at v², teal bias (R/G≈0.72), DOT_A 60→100 (`src/pill/canvas.rs`) |
| GEOMETRY | 3 | All elements within 1.5× parity — no dominant center peak (live 2:1, ref **7:1**); glow `+= w` sums neighbors so gaps can't exist | Peak-emphasis gaussian on half_h; ELEM_R_MAX 1.35→1.0, DOT_R 0.75→1.0, DOT_STEP 3.5→4.5, DOT_R_MIN 0.80→0.55 (`src/pill/canvas.rs`) |
| BLOOM | 4 | Glow fog: dot-vs-gap delta 4 LSB, halo 135 px wide vs ref ≤ 8 px, glow brightest between cores (inverted) | Max-field bloom + min(1.0) cap; emit threshold `smoothstep((v−0.35)/0.65)`; GLOW_K 4.5→2.5–3 two-scale; BLOOM 0.85→0.5 (`src/pill/canvas.rs`) |
| MOTION | 3 | Per-frame max-norm renders room tone full-height; 2–4 Hz wobble incoherent | AGC slow_peak decay 0.995 + rms gate 0.01 (`src/pill/vecmentor.rs`); wobble 0.4→0.1, idle_phase 1.1→0.4, idle_pose 0.8 SHAPE_MASK (`src/pill/vecmentor.rs`); ζ 0.28→0.45, overshoot 1.25→1.12 (`src/pill/tsar.rs`) |
| COMPOSITION | 5 | Dark-on-dark: body (35,38,37) ≈ host navy — capsule invisible in screenshot; hangs below clock center | BG → (16,20,30), 1px rim 8–10%, top specular (`src/pill/canvas.rs`); clock-center alignment (`src/pill/pill.rs`) |
| TASTE | 5 | Jewel-is-fog (cores must read first); idle is dead — the most-lived state has no charm | Covered by bloom threshold + white-hot peak; idle breathing pass below |

## Round-2 acceptance criteria (all six lenses sign off against these)

Measured (probe recipe in `targets.md`):

1. **Gap contrast ≥ 3.4:1** median per-dot core/adjacent-gap, min ≥ 2.5 over ≥ 20 dots — *the* number; fails if the waveform reads as one haze.
2. **Peak G ≥ 242**, peak-vs-body Δlum ≥ 170, core R/G ≈ 0.72 (cyan-teal), white-hot at v≥0.9.
3. **Bloom discipline**: only v ≥ 0.35 emits; halo ≤ 0.4× core at 5 px, returns to body+8 by 8 px; no bar/dot column FWHM > 6 px; no channel saturation > 255.
4. **Geometry**: pitch fill ratio 0.4–0.6; center peak ≥ 5× flank median height (target 7:1); capsule aspect ≈ 4.8:1; body ≈ (43,44,46) ± 6.
5. **Visibility on navy desktop**: capsule edge unambiguous in a screenshot against the navy host (rim + specular + (16,20,30) body), pill orbits clock center.
6. **Motion honesty**: idle = slow posed breathing (idle_phase ≤ 0.4, no full-height noise, ≥ 1.5 s cycle feel), dominant center peak present at idle AND active; springs settle (ζ≈0.45) without visible jelly wobble.

Deferred to round 3 (not blocking): SURF_W 220→170, landing flash, idle target rate-limiting, gamma-correct glow.
