Score: 7/10

- PASS — Idle center cluster is in the requested pose band: 14.4–18.0 px, approximately 40% of the 36 px cap height (35–45% target); reference-shaped center mound and dim dot line remain visible.
- PASS — Breathing cycle is 2.60 s (≥1.5 s); measured motion is gradual (mean |ΔH| 0.158 px/step, max 0.608 px); no full-height room-tone bars visible in idle render.
- PASS — Audio-to-Groq transcript is proven end-to-end (`critique/altgr-l-live-proof.md`); animation retains responsive active bars.
- FAIL — Profile correlation mean 0.9951 / minimum 0.9780, below ≥0.998 gate; residual per-bar motion still reshapes the profile.
- FAIL — Center cluster varies ±11.0% (14.4–18.0 px), just outside the ±8–10% breathing target. One-rim-column twinkle was a prior-frame pop (0→26 px); no evidence in supplied measurements establishes that it is eliminated.

Top 3 fixes:
1. `src/pill/vecmentor.rs:168-176` — reduce residual per-bar wobble / spring-induced profile variation until correlation reaches ≥0.998 without losing the slow breath.
2. `src/pill/vecmentor.rs:172-176` — trim `idle_breathe` from 0.09 enough to bring measured center excursion within ±10% (currently ±11%).
3. `src/pill/vecmentor.rs:363-366` — gate or smooth rim-column wobble to guarantee no one-frame threshold pop; verify with 100 ms frame dump.

