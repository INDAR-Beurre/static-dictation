# Round-1 Quantitative Targets — measured from `critique/reference.png`

Method: PIL + numpy on the full-res reference (1024×572). Reference pill capsule measured at
**x 421–602, y 15–52 → 182×38 px, ratio 4.8:1, corner radius ≈ 19 (full capsule height)**.
Live render measured from `critique/live-pill-zoom.png` (native screenshot scale, capsule ≈
213×53 px at x 6–218, y 0–53) and cross-checked on `critique/live-active-1.png` (2× scale, same values).
Scratch crops in `critique/scratch/` (`ref-pill-context.png`, `ref-peak-10x.png`, `ref-dots-10x.png`).

All colors are RGB 0–255, lum = mean(R,G,B). "Core" = brightest pixel of a bar/dot.
Reference pixel pitch is 4.0 px/bar; if the renderer uses a different pill width, **scale by
pitch (ratios below are scale-invariant)**.

## Table 1 — Targets (from reference)

| # | Metric | Target value | How measured |
|---|--------|--------------|--------------|
| a | Capsule body RGB | **(43, 44, 46)**, lum 44.3, peak lum anywhere on body ≤ 55 | median of clean body patch (y20–24, x435–470) |
| a | Body character | near-neutral charcoal (R≈G≈B, Δ≤3), NOT green-gray, NOT navy-tinted | channel deltas |
| b | Waveform core peak RGB | **(171, 239, 238)** — lum 216, **peak G 242**, R/G = 0.72 (cyan-teal, R pulled down) | top-10 pixels in waveform band |
| b | Hue ramp L→R | teal (R/G≈0.72, B≈G) at center → steel-blue (B>G, e.g. (106,152,211) at 70% out) → desaturated slate at ends | per-bar max-lum pixel |
| c | Bar pitch | **4.0 px** native (26 bars across 100 px of span); scale-invariant | run-length at y=38 |
| c | Bar width : gap | width 2–3 px, gap 1–2 px → **fill ratio ≈ 0.50 of pitch** (FWHM 2 px both) | FWHM at bar center row |
| d | Leader-dot diameter | **2 px** (FWHM), same 4 px pitch, 1–2 px tall | run-length, dots-only zone |
| d | Dot alpha falloff to edges | nearest-to-bars dot G ≈ **110** (45% of peak) → edge dot G ≈ **61–68** (25%) → outermost fades into body | per-dot peak G, L and R ends |
| e | Center-peak height | **21 px tall = 7× flank bar (3 px), ≈ 20× edge dot (1 px)**; peak centered, symmetric Gaussian-ish falloff | column extent > body+40 |
| f | Bloom halo (× core lum) | **1 px out: 0.5–0.6× · 5 px out: 0.34–0.41× · 15 px out: ≤0.35× and only beside tall bars — halo dies within 2–3 px of waveform ends and 5–8 px of the peak bar** | gap-envelope + vertical profiles |
| f | Halo extent (hard rule) | glow returns to body level (±5 lum) within **≤ 8 px** of any core; dot-level glow dies within 2–3 px | horizontal scan out of bar cluster |
| g | **Gap contrast (core G vs between-bar G)** | **≥ 3.4:1 typical, 3.7:1 at peak** (core 216 / worst adjacent gap 58). Far from center, gap ≈ body (48 vs 44) | local minima between bars at y=38 |
| — | Idle/edge dots vs body | dimmest visible dot G ≈ 61 = body + 17 (barely-there whisper line) | outermost dots |

## Table 2 — Current live render (same metrics)

| # | Metric | Live value | Verdict |
|---|--------|------------|---------|
| a | Body RGB | (37, 41, 40) lum 39 — greenish-gray, ΔR/B=3 | close on color, wrong on lightness role: body ≈ host navy → invisible |
| b | Core peak RGB | (86, 132, 134) — lum 117, **max G 132** | **−110 G** vs target |
| c | Pitch | ≈ 4.7 px (bars nominally present) | pitch OK |
| c | Bar width:gap | **unmeasurable — dots/bars fused**; whole waveform is one haze, FWHM **135 px** × 22 px | fatal |
| d | Dot diameter | ~2–3 px visible but zero separation | fused |
| d | Dot alpha falloff | edge G 61–71, center 132 — envelope exists but rides on 90–110 haze floor | floor too high |
| e | Center-peak ratio | max envelope 22 px vs dot-zone 11–16 px → **≈ 1.4–2 : 1** | target 7:1 — no dominant peak |
| f | Bloom | haze spans full capsule (135 px), mid-gap lum 90–110 = **2.3–2.8× body lift, never returns to body** | 20–40× too wide |
| g | **Gap contrast** | **1.01–1.06** (median per-dot core/adjacent-gap) | **target 3.7 — the single biggest failure** |

## Delta lines (what must move)

1. **Gap contrast 1.01 → ≥3.4** (this kills the "jewel is fog" failure; everything else is secondary).
2. **Peak G 132 → ≥242** (×1.83) with R/G ≈ 0.72 — peaks must reach the top of the palette.
3. **Haze FWHM 135 px → core 2–3 px + halo ≤ 8 px** (halo lift ≤ 0.4× core, 0 at 8 px).
4. **Peak:flank height 2:1 → 7:1** (one dominant center element; flank bars ≈ 3 px at ref scale).
5. **Peak-vs-body Δlum 78 → ≥170** (peak 216+ vs body ≤ 55); body toward (43,44,46) neutral charcoal.
6. **Dot alpha falloff**: restore graded whisper-line — edge dot G ≈ body+17 (25% of peak), not +22 flat.
7. Capsule aspect 4.0 → **4.8 : 1** (shorter/longer; pill height ≈ pitch × 9.5).

## Acceptance-test recipe (for round-2 self-check)

Run the same probes on a native-scale screenshot crop of the live pill:

- `max G` inside capsule must be ≥ 242; body median lum 40–50; capsule bbox ratio ≈ 4.8.
- Row through waveform center: run-length ≥ 20 runs, pitch σ ≤ 0.5 px, fill ratio 0.4–0.6.
- Median (local-max / nearest local-min) ≥ 3.4; min over ≥20 dots ≥ 2.5.
- Vertical/horizontal halo: lum ≤ 0.4× core at 5 px, ≤ body+8 at 8 px, no column with FWHM > 6 px.
- Envelope: center-peak height ≥ 5× flank median height; edge-dot G ≤ 40% of peak G.
