# Round 9 — final seven-critic roundup

**Average: 7.5 / 10** across seven independent DeepSeek V4.1 Flash (routing group)
reviewers. Progression: round 1 = 3.8, round 2 = 7.0, round 4 = 5.6,
round 7 = 6.6, round 8 = 7.3, round 9 = 7.5.

| Lens | Score | File |
|---|---|---|
| Color | 7 | `critique/round9-final-color.md` |
| Geometry | 7 | `critique/round9-final-geometry.md` |
| Bloom | 8 | `critique/round9-final-bloom.md` |
| Motion | 8 | `critique/round9-final-motion.md` |
| Taste | 7.5 | `critique/round9-final-taste.md` |
| Pixel audit | 7 | `critique/round9-final-pixel-audit.md` |
| Constraints | 8 | `critique/round9-final-constraints.md` |

## What the panel agrees is fixed

- One unmistakable apex, a 5-step gradual left ramp and a 3-step steep right
  falloff — the symmetric 5-bar "logo" of rounds 3–8 is gone.
- 0 white-plateau pixels; hot core `(170,235,245)` with R:G 0.723 (reference
  bar tips never bleach white, max G ≈242).
- Waveform span 78.91 % of the capsule (reference ≈79 %), end dots B−G +40
  on both tails, teal halo isolated R/G 0.658 (target 0.57–0.65).
- Idle breathing 0.027 px/step, profile correlation 0.9988, ±8.8 % excursion,
  2.80 s period, worst per-column step 2.0 px against a ≤3 px gate.
- All hard constraints: 3.58 MiB all-Rust binary (14× under 50 MB), real Groq
  `whisper-large-v3-turbo` transcript "Hello this is a dictation test 123"
  driven by a real injected AltGr+L chord, 95 passing tests, clippy clean.

## Real remaining gaps (consensus, worth another round if asked)

1. **Apex too tall relative to the capsule** — 68 % of capsule height vs the
   reference's 44 %; peak:flank ≈22:1 vs ≈7:1. Lowering `inner_h`
   (`src/pill/canvas.rs:389`) and slightly deepening the capsule would help.
2. **Capsule proportions** — live ≈4.7:1 with bloom vs the reference 3.9:1;
   the dotted tails stop a little short of the ends.
3. **Capsule material** — reviewers measure the reference body lighter and more
   neutral than our deliberate dark-on-navy `(16,20,30)`. This one is a design
   decision, not a bug: the desktop host is navy and the pill needs separation.

## Measurement caveats (do NOT read these as regressions)

- Probe halo R/G 0.778 aggregates dark BODY pixels; isolated halo pixels
  measure 0.658 (inside target). Gap contrast / haze / midgap ROIs include the
  drop shadow and are provisional; the probe's ≥3.4× gap gate is stricter than
  the reference's own ≈2.4× lit mound.
- The `~26 % W` mound target is the ≥8 px *zone*; the reference's half-peak
  mound is ≈17 % W, so round 9's 14.97 % is a pass. Several critics used the
  wrong definition in earlier rounds.
- Color/bloom requests to warm the peak toward pearl-white and lighten the body
  toward `(24,27,30)` were **not** actioned: their reference ROIs include
  sky/cloud pixels and use a different crop scale than
  `critique/ref-deep-measure.md`, which established that the reference never
  goes white and has a DARK top crease, not a bright keyline.
- The bloom critic's "reference core fuses into a 132 px mass" uses a 3.67×
  upscaled reference against our 2× render — not a like-for-like width.
- The voiced GIF's frame 0→1 peak jump was a QA cold-start artifact, fixed by
  warming the spring from the idle handoff in `src/dump.rs` (commit `9a2823d`);
  the remaining 34→44 px rise is speech attack.

## Human-only checks still open

Real microphone + foreground typing into a focused Wayland app, and
non-100 AltGr keycodes on other keyboards. Headless end-to-end dictation is
already proven in `critique/altgr-l-live-proof.md`.
