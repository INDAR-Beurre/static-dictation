Score: 7/10

- PASS — silhouette reads as a dark capsule; body RGB(18,23,33), rim +23, soft underside shadow. Close to reference obsidian against navy.
- PASS — idle is alive but quiet: ~40% pose, slow 2.60s breath; idle render shows a restrained centered mound, not a dead line.
- PASS — animation is smooth (mean |dH| 0.158px/step; corr 0.9951/0.9780); no evidence of random wobble in these stills. Active render is not a frozen logo by design, but stills alone cannot establish temporal liveliness.
- FAIL — active mound spans 57.4% capsule width vs reference ~26–28%; too broad and regular, more equalizer badge than jewel.
- FAIL — brightest core G=236 RGB(173,236,235), but reference is narrower, more cyan-hot jewel; current five parallel bars read as a logo. Plateau 0px (good), yet apex is flat-topped across three bars.
- FAIL — body lacks reference’s distinctly dark top crease and convincing cast shadow; edge appears like a faint outline, shadow is broad/weak against navy. End-dot B−G 13–33 only partly reaches reference +28…+58.

Top 3 fixes:
1. Narrow active central emphasis; restore ~26–28% mound width and single breathing apex: `src/pill/canvas.rs:157-160`.
2. Refine peak to one dominant apex instead of three equal vertical bars: `src/pill/vecmentor.rs:192-195`.
3. Strengthen directional shadow / top crease selectively against navy without bright rim: `src/pill/canvas.rs:164-175`.

Ranked remaining tells: (1) over-wide active mound; (2) three-bar flat apex; (3) weakly legible shadow on navy; (4) outline-like rim/crease rather than jewel depth; (5) teal center reads mint rather than cyan. No copy tell visible.