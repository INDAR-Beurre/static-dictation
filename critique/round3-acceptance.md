# Round-3 acceptance matrix — pill fidelity

Doc-only companion to `round3-brief.md`. Columns: criterion · ref target (`targets.md` / round-2
verdicts) · round-2 measured (`round2-scores.md` + live anchors) · round-3 threshold (brief C/M
items) · owner. Lens scores at round-2 close: taste 7, motion 8, color 7, bloom 7, geometry 6
(avg 7.0) — no separate verdict files existed under `critique/`; `round2-scores.md` §2 is the
verdict record (placeholders by design). Owners: canvas = `src/pill/canvas.rs`, motion =
`src/pill/vecmentor.rs` + `src/pill/tsar.rs`, harness = `scripts/*` + `src/main.rs` +
`src/pill/preview.rs`, n.a. = no code change (verify only).

## COLOR

| # | Criterion | Ref target | Round-2 measured | Round-3 threshold (brief) | Owner |
|---|-----------|------------|------------------|---------------------------|-------|
| C-1 | White-hot plateau / core color | hottest px (171,239,238), R/G 0.72; 13px ≥ lum 210; zero plateau | peak G 250 PASS but core (217,250,250) R/G 0.87; plateau 280px ≥ lum 230 (~20× ref clipped area) | px ≥ lum 230 → ≤ ~40; no flat plateau > 5 rows; core R/G ≥ 0.70; field ≥1.0 → solid teal ≈(171,239,238), white_mix ≈ 0.35·smoothstep(0.90,0.99,field) (C1) | canvas |
| C-2 | Halo tint | halo R/G 0.57 (teal) | 0.75–0.82 (glow samples post-lerp white) | R/G ≈ 0.57–0.65, B/G ≈ 1.05–1.17; sample pre-lerp teal ramp or ×(0.82,1.0,1.02) (C2) | canvas |
| C-3 | Tail hue | end dot (60,85,131), B−G +28…+58; mid-height bars steel-blue | B−G +6…+20 | end-dot bias floor B−G ≥ +30; tail cores ≈ (90,125,175); mid-height bars B>G ≈ +8, not mint (C8) | canvas |
| C-4 | Body + rim + specular (hold) | (16,20,30)±6, rim +12–16 lum, top specular; edge unambiguous vs host navy (17,33,66) | (18,22,32) PASS | hold; edge stays unambiguous; no channel > 255 (C13) | canvas |
| C-5 | Emit gate (hold) | only v ≥ 0.35 emits | working — do not regress | KEEP `smoothstep((v−0.35)/0.65)`; do NOT raise to 0.40 (kills C11 hum) (C10) | canvas |

## GEOMETRY

| # | Criterion | Ref target | Round-2 measured | Round-3 threshold (brief) | Owner |
|---|-----------|------------|------------------|---------------------------|-------|
| G-1 | Waveform span | 78.8% W (margins L 8.2% / R 13%) | 96.3% W | ~80% W; leader end `hw_c − 2.5·s` → `−15.0·s` (C3) | canvas |
| G-2 | Cluster width | 26.1% W, ~12 tall columns, mound 54/74/84/74/54 | 11.4% W (thin spike) | ~26% W; σ² ≈ 19, gain 1.5→1.0; broad mound not spike (C4) | canvas |
| G-3 | Peak:flank + flank floor | 7.3:1; flanks ≈ 7–11% of cap H | 17:1 (flanks collapsed) | ≈ 7:1; env floor 0.65→0.75; flanks 7–11% cap H (C5) | canvas |
| G-4 | Dropouts | unbroken dot line, every slot rendered | holes at crop x30–38, x70–87, x383–401 (interior max G < 30, neighbors 64–85) | zero dropouts; every slot renders ≥ DOT_R_MIN element; dot line unbroken across span (C6) | canvas |
| G-5 | Dot pitch | 4.0 native | 4.84 logical | 3.7 logical (DOT_STEP 4.5→3.7) (C7) | canvas |
| G-6 | Dot grain | 25–30 dots/side ("cheap execution" tell at ref scale) | ~9/side | 25–30/side (follows C7) | canvas |
| G-7 | Top cap | grown-cap top below y=0; no flat cut | clipped by margin −6 (y=0 row spans 87% of max width) | margin `(0,0,0,0)`; top lands ~3 logical px below y=0; bottom-edge + entrance tests still pass (M3) | motion |

## BLOOM

| # | Criterion | Ref target | Round-2 measured | Round-3 threshold (brief) | Owner |
|---|-----------|------------|------------------|---------------------------|-------|
| B-1 | Midgap glow vs single bar | gap ≈ bar height off-center (halo dies 2–3 px) | 1.09× med / 1.78× max (9/31 ≤1.05×) | ≤1.05× med and max (unchanged; improves via C1/C4/C9) | canvas |
| B-2 | Gap haze over body | far gap ≈ body (48 vs 44) | worst +22 at cluster edge | ≤ +20 (unchanged) | canvas |
| B-3 | Gap contrast | ≥3.4 typical, 3.7 at peak; min ≥2.5 over ≥20 dots | 2.41 med excl-cluster / 2.67 all; min 1.60 | ≥3.4 med, min ≥2.5 (unchanged; improves via C1/C4/C9) | canvas |
| B-4 | Tail dot glow lift | whisper: body +17 (25–28% of peak) | +56–64 (31–34% of peak) | +17–25, keep ≥ +10 visibility floor (C9); also scythes cluster base | canvas |
| B-5 | Cluster-base fusion | discrete columns, gaps read at peak | cluster reads 52px @2x fused at >45 threshold; peak-adjacent gap ≈2.7:1 | cluster unfused; discrete columns at matched scale (via C9) | canvas |
| B-6 | Halo extent + saturation (hold) | ≤8 px return to body±5; ≤0.4× @5px; no FWHM>6px; no channel >255 | dies 1–2 px horiz; 0.23× @5px; max 250 PASS | hold (C13 field cap 1.0) | canvas |

## MOTION

| # | Criterion | Ref target | Round-2 measured | Round-3 threshold (brief) | Owner |
|---|-----------|------------|------------------|---------------------------|-------|
| M-1 | Breathe amplitude | spec ±8–10% of posed height | ±1.5px (on full-height pose) | ±8–10% of posed height (M1) | motion |
| M-2 | Breathe period | spec ≥ 1.5 s cycle | 1.2 s (idle_rate 0.175) | ≈ 2.8 s (idle_rate → 0.075) (M1) | motion |
| M-3 | Idle pose height + shape | spec 35–45% of cap; silhouette shape kept (dim dots, ramp, dip, center cluster) | full-height pose | 35–45% of cap; scale height, NOT shape (M1) | motion |
| M-4 | Twinkle / wobble | no single-frame pops | one rim column popped 0→26px for 1 frame (6 LSB AA crossing) | wobble_amp 0.1→0.05; no column pop (M2) | motion |
| M-5 | Springs + profile (hold) | ζ ≈ 0.45, settle without jelly | settled, PASS | hold ζ 0.45 / ceil 1.12 / kick 4.6; profile corr ≥ 0.998 (M4) | motion |
| M-6 | Silence gate (hold) | byte-exact silence | byte-exact PASS | hold byte-exact (M4) | motion |

## TASTE

| # | Criterion | Ref target | Round-2 measured | Round-3 threshold (brief) | Owner |
|---|-----------|------------|------------------|---------------------------|-------|
| T-1 | Idle halo hum (new requirement) | n/a — new charm requirement (idle was dead) | absent | dim teal halo on posed peak, breathing ±20% in phase, ≈20–30% of active peak halo alpha; hums without shouting (C11) | canvas |
| T-2 | Glass-edge objecthood (new requirement) | taste spec: capsule must not read as a void darker than navy host | absent — rim only | 1px top keyline ≈ rgba(140,160,175,0.85); darker bottom edge line; elliptical specular streak upper half (stronger than 6px roll-off); soft blurred drop shadow; fits inside surface (bottom ≈48.5 phys of 95); use `ref-deep-measure.md` numbers if present at integration (C12) | canvas |
| T-3 | Jewel-first reading | cores read before glow; cluster is a mound, not a spike | cluster fused (see B-5) | cluster unfused + broad mound (via C4/C9); cores-first everywhere (C13 contrast direction kept) | canvas |

## PIXEL-AUDIT (cross-cutting — probe recipe per `targets.md`)

| # | Criterion | Ref target | Round-2 measured | Round-3 threshold (brief) | Owner |
|---|-----------|------------|------------------|---------------------------|-------|
| P-1 | Peak G | ≥242 (design-spec) | 250 PASS | hold ≥ 242-class on the new solid-teal core (C1) | n.a. |
| P-2 | Peak-vs-body ΔG | ≥170 (spec) | 228 PASS | hold ≥ 170 | n.a. |
| P-3 | Frame dump cadence | — | gif at ~0.3 s/shot aliases the 1.2–2.8 s cycles | 100 ms PNG dump, env-gated `STATIC_DICTATION_DUMP_FRAMES=<dir>`, zero behavior change (H1) | harness |
| P-4 | Motion sheet | — | not computable from 0.3 s gif | script → 100 ms gif + mean\|ΔH\|/step, profile corr, breathing-period estimate (H2) | harness |
| P-5 | Live proof | — | round-2 stills + 8-frame gif | post-integration: hold + released captures + 100 ms dump → side-by-side + live-anim gif (H3) | harness |
| P-6 | Released frame | edge + silhouette hold | PASS | re-check after C/M changes | n.a. |

## Round-3 pass bar

Minimum measured set that justifies 10/10 per lens (all from the H1 100ms dump unless noted):

- **COLOR**: plateau gate (≤~40 px ≥ lum 230, no flat >5 rows, core R/G ≥ 0.70) + halo R/G in 0.57–0.65 + tail B−G ≥ +30 + body/rim/saturation holds.
- **GEOMETRY**: span ≈80% W + cluster ≈26% W mound + peak:flank ≈7:1 with flanks 7–11% cap H + zero dropouts + pitch 3.7 logical / 25–30 dots per side + unclipped top cap.
- **BLOOM**: midgap ≤1.05× (med and max) + haze ≤ +20 + gap contrast ≥3.4/2.5 + tail lift +17–25 + halo/saturation holds.
- **MOTION**: period ≈2.8 s + amplitude ±8–10% + pose 35–45% (shape kept) + no column pop + M4 holds — period/amplitude read off the H2 motion sheet, not the old 0.3 s gif.
- **TASTE**: hum present (20–30% alpha, ±20% in phase) + glass edge (keyline, bottom edge, specular streak, drop shadow) + cluster reads as discrete mound.
- **PIXEL-AUDIT**: full probe suite green on the dump AND on the H3 live proof (side-by-side + released frame).

Verification-only (no implementation — harness dump / scorer reads them): P-1, P-2, P-6, B-1, B-2, B-3, B-6, C-4, C-5, M-5, M-6, and every H2 metric (mean|ΔH|/step, profile corr, period estimate). Implementation-bearing rows: C-1…C-3 (canvas), G-1…G-6 (canvas), G-7 (motion), B-4, B-5 (canvas), M-1…M-4 (motion), T-1, T-2 (canvas), P-3, P-4, P-5 (harness).
