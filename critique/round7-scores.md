# Round 7 scores — 7 independent DeepSeek V4.1 Flash (routing group) critics

Evidence: commit `65d4c12`, `critique/round6-final-side-by-side.png`,
`critique/round6-final-active-render.png`, `critique/round6-final-active.png`,
`critique/round6-final-idle-render.png`, `critique/round6-final-active-anim.gif`,
`critique/round6-final-measurement.md`. All critics were read-only, ran in
parallel, and used `thinking=low` for fast iteration.

| Lens | Score | File | One-line verdict |
|---|---:|---|---|
| Color | 7/10 | `critique/round7-color.md` | Hot core exact; halo R/G .783 FAIL (but see measurement note below). |
| Geometry | 5/10 | `critique/round7-geometry.md` | Mound 17.7% W vs 26–28%, symmetric vs ref's left ramp / right falloff. |
| Bloom | 6/10 | `critique/round7-bloom.md` | Central bars fuse; halo too neutral; long low-level skirt. |
| Motion | 7/10 | `critique/round7-motion.md` | Idle metrics PASS, but the 15-frame/200 ms active GIF is too short to prove fluidity. |
| Taste | 6/10 | `critique/round7-taste.md` | Same kind of object, not the same object: narrow equalizer vs soft integrated prop; weak shadow. |
| Pixel audit | 6/10 | `critique/round7-pixel-audit.md` | Gates that matter PASS (0 plateau, teal core, 79% span, +40/+40 ends); the FAIL gates are contaminated proxies. |
| Constraints | 9/10 | `critique/round7-constraints.md` | 3,749,864 B binary, all Rust, real Groq transcript via AltGr+L; desktop delivery + real mic still human-only. |

**Average: 6.6/10** — not 10/10, so another round is required.

## Consensus fixes (ranked by how many critics named them)

1. **Broaden and reshape the mound** (geometry, taste, pixel-audit, bloom):
   17.7% → ~26% W, single apex slightly right of centre, gradual left ramp,
   steep right falloff, keep full dot-line span ≈79% W.
2. **Halo / gap isolation** (color, bloom, taste, pixel-audit): reduce central
   bar fusion and make the fringe teal rather than neutral.
3. **Visible soft drop shadow** (taste, pixel-audit): separate the capsule from
   the navy host, left-biased, ~18 px below.
4. **Longer animation evidence** (motion): ≥3 full breathing periods at the
   real 100 ms cadence, settled first frame, plus a per-column pop metric.
5. **Desktop typing delivery** (constraints): headless proof logged "typing
   unavailable"; a real focused-window Wayland run remains a human-only check.

## Measurement correction discovered while aggregating (important)

The `halo R/G = 0.783` FAIL is **an artifact of the probe**, not the renderer.
`critique/round7-pixel-audit.md` itself says `lum<180` aggregates dark body
pixels. Measuring only isolated halo pixels in the raw 440×128 frame
`/tmp/r7f/frame-0015.png` (G 50..170, B > 0.6G, R < G, around the apex) gives:

```
apex surround  n=611  R/G 0.658  mean RGB (51.4, 78.2, 87.7)
wide           n=693  R/G 0.679  mean RGB (59.0, 86.9, 96.1)
core           n=734  R/G 0.730  mean RGB (166.1, 227.5, 229.1)
```

So the halo is already ≈0.66–0.68, essentially inside the 0.57–0.65 target.
Similarly the ≥3.4× gap-contrast gate contradicts the reference's own ≈2.4×
lit mound. These two FAILs should be fixed by using an isolated-halo ROI, not
by bleaching the halo. Documented so the next panel does not re-report them.

## Verified body RGB (color critic left it UNVERIFIED)

Interior-only patches of `critique/round6-final-active-render.png`, excluding
shadow, edge AA and glow: right interior median **(16, 20, 30)** = exact
target; left interior median (22, 26, 36) carries a subtle +6 specular lift.
Body PASS. Shadow rows y160–190 measure (14,27,52)…(16,30,61) against host
navy (17,33,66) — a real but low-contrast darkening.

Read-only plan for the mound fix: `critique/round7-asymmetry-plan.md`.
