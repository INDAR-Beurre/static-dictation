# Round 7 — asymmetry plan (proposed, NOT applied)

This is a read-only design note. The round-7 critic panel is scoring commit
`65d4c12`; do not edit `src/pill/canvas.rs` until those scores land, or the
panel will see a mixed tree.

## The remaining #1 visual gap

`critique/round6-final-side-by-side.png`: our bottom pill has a **symmetric**
mound with the apex at the exact capsule centre; the reference top has a
gradual left ramp, an apex slightly RIGHT of centre, and a steep right falloff.
Reference 18-slot heights (`critique/ref-deep-measure.md`):

```
2,3,3,5,5,3,3,3,3,5,8,14,21,17,12,7,5,3
│──── low left plateau ────│   ╱peak╲  steep right
```

Normalised to the 21 px apex: `.095 .143 .143 .238 .238 .143 .143 .143 .143
.238 .381 .667 1.000 .810 .571 .333 .238 .143`. Peak is slot 11/17 ≈ u 0.65,
not 0.50. Left of the peak spans 11 slots of low then rising energy; right of
the peak collapses in 6 slots.

## Current code (where to change)

`src/pill/canvas.rs:396-407`:

```rust
let env = 0.75 + 0.25 * (PI * ((i as f32 + 0.5) / n)).sin();
let dc = (i as f32 + 0.5) - n * 0.5;
let pk = 1.0 + (-(dc * dc) / PEAK_SIGMA2).exp(); // PEAK_SIGMA2 = 19.0 at :160
let raw_field = env * pk;
let field = raw_field.min(1.0);
let half_h = ((v * inner_h * 0.5).max(DOT_R_MIN * s) * field).min(inner_h * 0.5);
```

`field` also feeds `wmix` (hot teal snap, `:421-425`), `dim` (`:441`), and
`gw` (`:455-458`). `src/dump.rs:93-99` `speech_bars()` is a symmetric QA feed
`exp(-9 d²)`; the renderer envelope dominates the shape, so changing the
renderer is enough — but the QA frame will still be symmetric under it, so A/B
the renderer with a fixed flat `bars = [1.0; 27]` before trusting the dump.

## Two candidate fixes (pick by measured A/B, not by taste)

**A — asymmetric peak gaussian (geometry).** Move the apex right and split σ²:

```rust
let dc = (i as f32 + 0.5) - n * 0.58;
let sig2 = if dc < 0.0 { 26.0 } else { 9.0 };
let pk = 1.0 + (-(dc * dc) / sig2).exp();
```

Risk: a broader left σ² pushes more slots to `raw_field ≥ 1`, and the
`.min(1.0)` clamp at `:407` recreates the 3-bar plateau we just killed in
`54eb47c`. If measured heights flatten, lower `pk`'s gain (e.g. `0.5 +
0.5*exp(...)`) instead of removing the clamp.

**B — lateral illumination multiplier (lighting).** Keep the peak gaussian
exactly as-is and apply the asymmetry after the clamp:

```rust
let u = i as f32 / (n - 1.0).max(1.0);
let illum = if u < 0.58 {
    0.55 + 0.45 * (u / 0.58).powf(0.7)
} else {
    1.0 - 0.85 * ((u - 0.58) / 0.42).powf(1.4)
};
let half_h = (... * field * illum).min(inner_h * 0.5);
```

Then multiply `dim` by the same `illum` so short right-flank bars are also
dimmer (the reference's lit mound falls off in brightness, not only height).
Risk: `wmix` uses `field` (unchanged), so a short right bar could still snap to
hot teal — cap `wmix` by `illum` if that reads wrong.

Option B is smaller and cannot resurrect the plateau; prefer it unless the A/B
shows the reference's apex position matters more than the ramp shape.

## Also still open (lower priority)

- Mound width 17.69% W vs reference ~26–28% W. Widen by lowering `PEAK_SIGMA2`
  is WRONG (narrower); instead raise the `VIS_FLOOR`/whisper ramp so the
  8 px-tall zone extends, or increase `BAR_SPAN_FRAC` `0.68 → 0.76` and
  re-measure span (currently 78.91% W ≈ 80% PASS — do not exceed 84%).
- Halo R/G 0.783 vs 0.57–0.65: the pre-hot `glow_r *= 0.85` at
  `src/pill/canvas.rs:~616` already helps; a further `0.78` would deepen the
  teal but must not push the gap haze above +20 G.
- Weak drop shadow: `SHADOW2_A 110` (~43% α) is already above the ref's ~35%;
  the perceived weakness is the navy-on-navy host, not the alpha. Do NOT
  brighten the rim past +20 lum (currently +23, already slightly hot).

## Verification checklist before committing any of the above

1. `cargo test` must stay 95 passed / 0 failed / 2 ignored **without
   loosening any assertion**; re-anchor with measured numbers only.
2. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
3. Regenerate `/tmp/r8f` voiced + `/tmp/r8i` idle dumps with the final release
   binary; rebuild `critique/round7-final-side-by-side.png` with **2× NEAREST**
   (LANCZOS ringing fabricates a 732-px white plateau — see
   `critique/round6-final-measurement.md`).
4. Re-run `scripts/round6-measure.py` with `PILL_BBOX=146,22,733,155` and the
   motion sheet; corr must stay ≥0.998 mean, excursion ±8–10%, period ≥1.5 s.
5. One more ≥6-agent routing-group critic round on the new render.
