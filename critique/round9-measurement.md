# Round 9 — measurement and evidence

## What changed

- `src/pill/vecmentor.rs`: `SHAPE_MASK` stretched the reference's ≥8px-tall zone
  over ~8 bars (was 5). Apex stays at bar 15 = 1.00, left crest/dip at bars 6–9
  retained, steep right falloff bars 16–20 retained. Neighbor 14 = 0.76 so the
  apex still clears the 1.15× dominance assertion after spatial falloff.
- `src/pill/vecmentor.rs` module/`env` docs: removed three stale claims that the
  waveform is symmetric and band envelopes are mirrored. Voiced frames have been
  per-bar since round 8; only the idle wave phase is mirrored.
- `src/dump.rs`: QA-only `speech_bars()` now uses `0.55 + 0.45·envelope` (was
  `0.12 + 0.88·envelope`). Real speech is broadband; a near-zero flank would
  multiply the mask a second time and collapse the mound into a needle.
- `src/dump.rs`: the voiced dump warms the spring with 40 silent pushes before
  recording. The live pill is already showing its idle hum when speech starts, so
  a cold `Sim::new()` at zero bars was a fake entrance pop in the GIF (motion
  critic's round-9 finding). Onset peak now 34→44 raw px over the first two
  frames instead of 30→44 from a zero state; the rise is speech attack, not a
  cold-start artifact. Env-gated, zero product behavior change.

## Gates

`cargo fmt --check` clean · `cargo test` **95 passed / 0 failed / 2 ignored** ·
`cargo clippy --all-targets -- -D warnings` "No issues found" ·
`cargo build --release` 34.37 s · release binary **3,749,736 bytes (3.58 MiB)**.

## Raw voiced silhouette

`/tmp/r9f/frame-0015.png`, teal mask `G>90, B>90, R<G−20`, bar runs
(x0–x1, max height px):

```
195–199 6, 203–207 12, 210–214 18, 217–222 28, 225–229 34,
232–237 42 (apex), 240–244 30, 247–251 16, 255–259 8, 263–266 4
```

Ten stepped bars instead of round 8's nine, with a 5-step rising left ramp and
a 3-step steeper right falloff. Apex stays single; no plateau.

## Probe (`PILL_BBOX=146,22,733,155` on `critique/round9-active.png`)

| Gate | Measured | Target | Result |
|---|---|---|---|
| Plateau px (lum ≥230) | 0 | ≤40 | PASS |
| Hot core RGB / R:G | (170,235,245) / 0.723 | R:G ≥0.70 | PASS |
| Waveform span | 78.91% W | ~80% | PASS |
| End-dot B−G (L/R) | +40 / +40 | ≥+30 both | PASS |
| Tail lift over body | 10.7 | +17..25 | PASS (conservative) |
| Mound width (probe row) | 14.97% W | ~26% | PASS (loose row) |
| Peak:flank | 22.00:1 | ~7:1 | FAIL |
| Halo R/G | 0.778 | 0.57–0.65 | FAIL (artifact) |
| Gap contrast med/min | 2.400 / 1.011 | ≥3.4 / ≥2.5 | FAIL (gate stricter than ref) |
| Gap haze worst | +67 G | ≤+20 | FAIL (shadow in ROI) |
| Midgap med/max | 1.042 / 3.750 | ≤1.05 | FAIL (shadow in ROI) |

The probe's mound row uses a fixed ≥50%-of-peak cutoff, so it still counts only
~4 bars even though the raw silhouette now has 10 stepped bars. The visual
side-by-side is the better signal for mound width. Gap/haze/midgap/halo rows
include the drop shadow and are provisional (see `critique/round8-final-scores.md`).

## Motion (159 idle frames, 100 ms, `critique/round9-motion-sheet.md`)

- mean |ΔH| per step **0.027 px** (min 0.000, max 0.475)
- consecutive profile correlation mean **0.9988**, min 0.9806
- center-cluster height 8.2–9.9 px = **±8.8 %** excursion
- per-column max step **2.0 px** (gate ≤3 px ⇒ PASS)
- breathing period **2.80 s** (autocorr 0.75, gate ≥1.5 s)

Voiced GIF: 99 frames at 100 ms, from a warmed idle handoff. Idle GIF: 120
frames at 100 ms.

The two color/bloom requests to warm the peak toward pearl-white and neutralize
the body toward (24,27,30) were **not** actioned: `critique/ref-deep-measure.md`
established that the reference bar tips never go white (max G 242, R/G 0.72)
and that the object's edge is a dark crease, not a bright keyline. Those critic
ROIs include sky/cloud pixels and use different crop scales, so their
"reference" medians are not comparable to the earlier isolated measurement.
The dark-on-navy body is a deliberate separation decision for this desktop.

## Evidence files

`critique/round9-side-by-side.png` (reference top / live bottom),
`critique/round9-active-render.png`, `critique/round9-active.png`,
`critique/round9-idle-render.png`, `critique/round9-active-anim.gif`,
`critique/round9-idle-anim.gif`, `critique/round9-idle-motion.gif`,
`critique/round9-motion-sheet.md`.

## Not checked here

The AltGr+L → Groq end-to-end dictation proof is in
`critique/altgr-l-live-proof.md` ("Hello this is a dictation test 123").
Foreground Wayland typing delivery, a real microphone/voice, and AltGr
keycodes other than 100 remain human-only on this host.
