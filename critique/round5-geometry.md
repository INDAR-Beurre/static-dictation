Score: 4/10

- PASS — Capsule silhouette is near the required 3.9:1, full-round stadium; clean boundaries and no clipping in the supplied active render.
- FAIL — Wave span measures only **57.4% W**, not ~80%; leaders stop well inside the capsule, leaving conspicuous empty margins. Target 25–30 dots per side; current visible line has roughly **17–19 per side**. `src/pill/canvas.rs:93` limits bar span to 0.68 half-width; leader endpoint is `src/pill/canvas.rs:505`.
- FAIL — Cluster is too narrow: mound spans **57.4% W** by the supplied measurement, while target’s ≥8px zone is **28% W**; visible bars occupy only a small central patch. `src/pill/canvas.rs:397-412` shapes envelope/peak height.
- FAIL — Instead of a single 21px apex with a broad 28%-W ≥8px zone, render has a visibly flat **three-bar top**. Target peak:flank is ~7:1; round-4 image makes flank heights look much smaller, but exact ratio is not provided by probes. `src/pill/canvas.rs:402-412` controls peak emphasis.
- PARTIAL — Pitch and element widths appear close to target (measured pitch **4.08px**, dot FWHM **2.2**, bar FWHM **2.7**); line has visible interruptions near both ends, so leader is not convincingly unbroken.

Top 3 fixes:
1. `src/pill/canvas.rs:93` — extend leader waveform span to ~80% W; retain measured 4.08px pitch.
2. `src/pill/canvas.rs:397-412` — broaden the mound to a 28%-W ≥8px zone and make one slot the sole apex (21px); remove three-bar plateau.
3. `src/pill/canvas.rs:503-509` — emit a continuous, hard-stop leader line at 25–30 dots per side, with no missing visible slots.
