# Round-4 motion critique

**MOTION: 6/10 — FAIL** against the round-3 MOTION gate. Pose and nominal ~2.6 s period pass; profile correlation (0.9951 mean, 0.9780 min < 0.998), ±11% center-cluster range vs ±8–10%, and no-pop requirement are not demonstrated as passing. The GIF shows a breathing idle silhouette, not a room-tone full-height slam or obvious twinkling rim column. The .9780 low-correlation outlier is plausibly threshold/AA quantization at 6 LSB, not visible whole-profile flicker; however GIF thresholding cannot prove no single-column pixel pop. Active 15-frame sample looks stable, though too short to judge voiced fluidity.

Top fixes:
1. `src/pill/vecmentor.rs:173-174, 366` — deterministic per-bar wobble phases/rates plus 5% wobble still risk incoherent rim movement; lower or remove wobble for idle to protect the no-pop gate.
2. `src/pill/vecmentor.rs:372-374` — `max(pose_body, wave_body)` creates a moving crossover in low-mask bars; smooth the blend so the profile remains coherent rather than switching contributors.
3. `src/pill/vecmentor.rs:174` — tighten `idle_breathe` from 0.09 to about 0.07: measured cluster spans ±11%, outside the ±8–10% target.

Verdict: idle reads as slow, broadly fluid breathing, but it misses numeric smoothness gates and the active clip is too brief to establish fluid speech motion.