# Round 8 — final panel scores

Seven independent DeepSeek V4.1 Flash (routing group) critics, all read-only, all
scoring the committed `4ef7811` evidence (`critique/round8-side-by-side.png`,
`critique/round8-active-render.png`, `critique/round8-idle-anim.gif`).

| Lens | Score | Report | Headline |
|---|---|---|---|
| Color | 7/10 | `critique/round8-final-color.md` | teal core + halo + blue tails pass; wants a lighter, neutral body and no pale rim |
| Geometry | 7/10 | `critique/round8-final-geometry.md` | single apex + left ramp pass; mound too narrow, peak:flank too high |
| Bloom | 7/10 | `critique/round8-final-bloom.md` | teal, restrained, clean tails; central halos still merge slightly |
| Motion | 8/10 | `critique/round8-final-motion.md` | 0.032 px/step, corr 0.9983, 2.70 s, 2 px pop gate pass |
| Taste | 7/10 | `critique/round8-final-taste.md` | asymmetric apex now reads like the reference; tails too short/prominent, capsule too flat |
| Pixel audit | 7/10 | `critique/round8-final-pixel-audit.md` | plateau 0, core (170,235,245), span 78.91%, end dots +40/+40 pass; mound 15.31% W, peak:flank 22:1 fail |
| Constraints | 8/10 | `critique/round8-final-constraints.md` | 3.58 MiB, all Rust, 95 tests, real Groq transcript; typing delivery + real mic still human-only |

**Average: 7.3/10** (round 7 was 6.6/10, round 1 was 3.8/10).

## Consensus (4 of 7 named it the top fix)

The voiced mound is a needle: ~15% of pill width vs the reference's ~26–28%, and
peak:flank 22:1 vs ~7:1. Root cause: `SHAPE_MASK` mapped the reference's
18-slot envelope onto only 5 tall bars, and the QA voiced feed's near-zero
flank energy multiplied that mask a second time.

Also recurring: dotted tails look too short/prominent (span measures 78.91% of
pill width = PASS, so this is visual weight, not length), capsule reads flatter
and more outlined than the reference, and the desktop typing path is unproven.

## Measurement caveats carried into round 9

- Halo R/G 0.784 is a probe artifact: it aggregates dark body pixels. Isolated
  halo pixels measure 0.658 (target 0.57–0.65).
- Gap contrast / haze / midgap ROIs include the drop shadow, and the probe's
  ≥3.4× gate is stricter than the reference's own ~2.4× intra-cluster contrast.
  These rows are provisional, not failures.
- The color critic's request to lighten the body toward (40,45,47) conflicts
  with the established dark-on-navy separation decision: the reference sits on
  a bright sky, our host top bar is navy (17,33,66). Not actioned.
- Geometry's "50.9% span" measures full-image width, not pill-relative width.

## Round-9 action (main context, single focused pass)

`src/pill/vecmentor.rs` — widened `SHAPE_MASK` so ~8 bars exceed half the apex
(apex bar 15 at 1.00, left crest/dip retained, steep right falloff), cleaned
three stale "symmetric/mirrored" comments. `src/dump.rs` — QA voiced feed gained
a broadband pedestal (0.55 + 0.45·envelope) so shoulders are not double-dimmed.
No test tolerance was loosened; `SHAPE_MASK[14]` was set to 0.76 after measuring
that the apex/shoulder ratio fell to 1.148 against the existing 1.15 assertion.
