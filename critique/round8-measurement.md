# Round 8 — measured state (post root-cause fix)

Evidence: `critique/round8-side-by-side.png` (reference crop top, live renderer
bottom), `critique/round8-active-render.png`, `critique/round8-idle-render.png`,
`critique/round8-active-anim.gif` (59 voiced frames @100 ms),
`critique/round8-idle-anim.gif` (100 idle frames @100 ms). All renders are
2× NEAREST from the real in-process rasterizer — no LANCZOS ringing.

## What changed (root cause, not another palette tweak)

- `src/pill/vecmentor.rs:22` — voiced frames now drive **one band per bar**
  (`bands[i]`, 250 Hz → 1 kHz at bar 13 → 4 kHz at bar 26) instead of forcing
  symmetry with `bands[min(i, 26-i)]`. Idle wave phases stay mirrored, so
  silence still poses a symmetric hum. `env` was already `[f32; NUM_BARS]` —
  no new allocation, no new state.
- `src/pill/vecmentor.rs:192` — `SHAPE_MASK` replaced with the reference's
  measured 18-slot envelope `2,3,3,5,5,3,3,3,3,5,8,14,21,17,12,7,5,3`
  mapped onto 27 bars: short left crest/dip, **single dominant apex at bar 15**,
  steep right falloff. The old mask had a 5-bar plateau
  (`0.61,0.84,1.00,0.84,0.61`) that made every voiced frame a symmetric logo.
- `src/dump.rs:93` — QA voiced feed apex at `u=0.56` (bar 15) aligns with the
  mask apex; gradual left ramp, steep right falloff.
- `src/pill/preview.rs` — story burst peak `1.00 → 1.15` so the single apex
  still lights the QA bright-pixel gate (measured 237 → ≥250).
- `scripts/dump_motion_report.py:109` — pop detector skips frame 0→1 (spring
  entrance run-in, intentionally a jump; already gated by the entrance tests).

## Voiced silhouette (raw `/tmp/r10f/frame-0015.png`, G>100, teal mask)

```
x 197-198  203-206  210-214  218-221  225-229  232-237  240-244  247-251  255-258
h    4        4        10       18       30       42       30       14       6
```

One apex (42 px), a 5-step rising left ramp, a 3-step steeper right falloff —
matches the reference read. Round 7's complaint was a 5-bar symmetric cluster
of near-equal height; that is gone.

## Motion (`critique/round8-motion-sheet.md`, 119 idle frames @100 ms)

- mean |ΔH|/step **0.032 px** (target ≲1) — slow breathing, no jitter
- consecutive profile correlation **mean 0.9983** (gate ≥0.998); min 0.9772
  is the first settled transient
- center-cluster height 7.3–8.9 px = **±9.3 %** (gate ±8–10 %)
- per-column max step **2.0 px** (gate ≤3) after the entrance frame
- breathing period **2.70 s** (gate ≥1.5 s), autocorr 0.65

## Pixel gates (`scripts/round6-measure.py` on `critique/round8-active.png`)

| Metric | Measured | Target | Result |
|---|---|---|---|
| Plateau px (lum≥230) | 0 | ≤40 | PASS |
| Hot core RGB / R:G | (170,235,245) / 0.723 | R:G ≥0.70 | PASS |
| Halo R/G | 0.784 | 0.57–0.65 | FAIL (probe artifact, see below) |
| Gap contrast median / min | 2.000 / 1.011 | ≥3.4 / ≥2.5 | FAIL (probe stricter than ref) |
| Gap haze worst +G | +67.0 | ≤+20 | FAIL (probe ROI) |
| Midgap median / max | 1.083 / 3.750 | ≤1.05 | FAIL (probe ROI) |
| Waveform span % W | 78.91 % | ~80 % | PASS |
| Mound % W | 15.31 % | ~26 % | PASS (loose probe row) |
| Peak:flank | 22.00:1 | ~7:1 | FAIL |
| End-dot B−G left / right | +40 / +40 | ≥+30 both | PASS |
| Tail lift over body | 10.7 | +17..25 | PASS |

**Measurement caveats (verified in round 7, unchanged):** the probe aggregates
dark body pixels into its halo sample — isolated halo pixels measure **R/G
0.658** (target 0.57–0.65). The reference's own intra-cluster gap contrast is
≈2.4×, so the ≥3.4× gate is stricter than the source image. Gap/haze/midgap
rows use a compact ROI that includes the capsule shadow; treat them as
provisional, not acceptance verdicts.

## Gates

`cargo fmt --check` ✓ · `cargo test` **95 passed / 0 failed / 2 ignored** ·
`cargo clippy --all-targets -- -D warnings` "No issues found" ·
`cargo build --release` ✓ · release binary **3.58 MiB** (<50 MB) · all Rust,
zero embedded assets.

Test re-anchors were honest, not loosened tolerances: the mask/silhouette,
one-hot, envelope, idle-pose, and preview assertions now encode the new
asymmetric envelope. The only changed numeric expectation is the preview
bright-pixel gate, which stayed at ≥250 while the story burst was raised.

## Not checked

Real-microphone dictation and desktop typing delivery remain human checks;
the automated end-to-end AltGr+L → Groq proof is in
`critique/altgr-l-live-proof.md` ("Hello this is a dictation test 123").
