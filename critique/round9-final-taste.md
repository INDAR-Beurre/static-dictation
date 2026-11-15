# Round 9 — final taste (TASTE lens)

**7.5/10**

Closer than round 8, but the bottom still reads as a *logo of a waveform* —
compact, near-centred, flat-topped bars — while the reference reads as a soft
glow *inside* a lit charcoal capsule. Left and right shoulders are almost
mirrored, and the capsule is a different material.

| Check | Result |
|---|---|
| Single apex, no plateau | **PASS** — one clean tall bar |
| Apex right of centre, long left ramp | **FAIL** — apex at ~0.53–0.56 of span; `SHAPE_MASK` is mirror-symmetric (right 0.80/0.56/0.38/0.25 vs left 0.76/0.58/0.42/0.30), so it reads as a pyramid |
| Mound breadth (~8–10 bars) | **PASS** — 10 stepped bars (6→42→4 px) |
| Long fine dotted tails | **PASS** — tails reach near both capsule ends |
| Capsule material/light | **FAIL** — live body (23,28,39) vs reference (43,54,55): ~20 darker, ~5 bluer; lit charcoal vs flat navy |
| Glow inside the body | **PASS** — plateau 0, core R:G 0.723 |
| Fluid, non-random motion | **PASS** — 0.027 px/step, corr 0.9988, 2.80 s, max column step 2.0 px |

## Top 2 remaining fixes

1. **Turn the mirrored mask into a real ramp** — `src/pill/vecmentor.rs:189-191`.
   Move the apex one bar right (15 → 16), extend the left shoulder (bars 9–14 →
   ~0.12/0.18/0.26/0.36/0.48/0.64) and steepen the right (bars 16–19 →
   0.72/0.44/0.24/0.13). Retains the 1.15× dominance assertion.
2. **Lift the capsule off navy onto lit charcoal** — `src/pill/canvas.rs:183`
   `BG` (16,20,30) → ~(32,40,41), with `RIM`/`SHEEN` (`:166-167`) one step softer,
   so the body measures near the reference's (43,54,55) and carries a top-left
   brightness gradient. Reference body is teal-neutral, not blue.

**What is right:** the ten-step mound with long dotted tails and one
unmistakable apex now reads as dictation, not a generic symmetric badge — the
core silhouette problem of rounds 3–8 is solved.

Not checked: provisional gap/haze/halo probe rows (their ROI includes the
shadow); foreground typing, real mic and non-100 AltGr keycodes remain human-only.
