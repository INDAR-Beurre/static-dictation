# Geometry critic — round 4

**Score: 4/10.** Capsule silhouette is convincingly full-round, but the waveform still reads like a narrow, symmetric logo mark rather than the measured broad, asymmetric mound.

## Ranked
1. **FAIL — mound width/apex:** reference has an 18-slot envelope with one 21px apex and a ≥8px zone spanning 28% of capsule width. Current render visibly contracts to a few central bars; three neighboring bars look nearly co-dominant. The synthetic input’s symmetric Gaussian further encourages this (`src/dump.rs:93-99`); canvas field/hot-snap shaping is at `src/pill/canvas.rs:397-410`.
2. **FAIL — flank balance:** visible flanks fall too abruptly against the center, matching the reported excessive peak:flank ratio. Height floor and color scaling are at `src/pill/canvas.rs:397-412` and `src/pill/canvas.rs:430-437`; adjust there to land near 7:1, not by widening the capsule.
3. **PARTIAL — line span/endpoints:** target is ~80% width with asymmetric 8%/13% margins and hard-stop end dots. Current line termination is explicitly `src/pill/canvas.rs:501-507`; compare endpoint placement there against the target before changing pitch.
4. **PASS visually — capsule:** proportions and rounded ends are close. **Unverified — shadow:** the current preview background hides a useful reference-scale comparison.

## Top three fixes
- Reshape `src/dump.rs:93-99` to a single, slightly asymmetric apex; avoid a broad symmetric synthetic input.
- Tune mound width and central rank at `src/pill/canvas.rs:397-410` so the ≥8px zone reaches ~28% W and only one slot owns the apex.
- Raise flank contribution at `src/pill/canvas.rs:397-412` / `430-437` until peak:flank is ~7:1 while retaining the outer envelope.

**Probe artifacts:** the “6 interior dropouts” are likely crop/probe artifacts, not established geometry failures; the requested evidence does not show six clear holes. “~12 dots/side” and “14px crop pitch” are crop-scale readings, not native pitch—`DOT_STEP=2.05` at 2× yields ~4.1px (`src/pill/canvas.rs:95`). The 3-bar flat-top report is not fully a probe artifact: the render does show a visually over-dominant central group, though exact count/height needs consistent-scale sampling.
