# Round 2 Pixel Audit + Scoreboard

Auditor: round-2 visual scorer. All numbers measured with PIL+numpy on `critique/round2-active.png`
(880×190 = 2× upscale of a 440×95 physical crop @ display scale 1.5; capsule x4–456, y0–99 @2x).
Probe method: waveform band rows 30–70 @2x; per-column max G; runs = columns with G>45 (body G=22);
midgap G taken at the taller adjacent bar's emitting rows. Native px = @2x ÷ 2. Released-frame and
8-frame `live-anim-30.gif` checks noted where relevant. No captures were re-run.

## 1. Pixel audit — round-2 acceptance criteria (round1-synthesis.md §5)

| # | Criterion | Current (measured) | Target | Verdict |
|---|-----------|--------------------|--------|---------|
| 1a | Peak G | **250** @ x234–236, RGB (217,250,250) — white-hot core; flanks teal R/G 0.66–0.67 | ≥200 (design-spec ≥242) | **PASS** |
| 1b | Peak-vs-body G delta | **228** (250 − body G 22) | ≥100 (design-spec ≥170) | **PASS** |
| 2a | Mid-gap G vs single-bar G | **median 1.09×, max 1.78×** (n=31 gaps; 9/31 ≤1.05×; worst at peak-cluster flanks: 42 vs clean sidegap 24) | ≤1.05× | **FAIL (marginal)** |
| 2b | Gap-vs-body haze | midgap G 23–44 (median 34) vs body G 22 → **+1…+22**; dot-gaps +18/+20 | within 20 | **FAIL (marginal)** — worst +22 at cluster edge |
| 3a | Emission discipline (tail-dot G vs body G) | outermost dots G **78–86** = body+56–64 (31–34% of peak; ref whisper-line = body+17, 25–28% of peak); dot-vs-gap delta now +38–44 (round 1: 4 LSB) | graded taper, dimmest dot ≈ body+17 | **FAIL** — dots over-emit, taper compressed |
| 3b | Halo width (targets.md hard rule) | horizontal: dies to body±5 within ~1–2 native px of dots, ~2 px of cluster; vertical 1px-out 0.66× core (ref 0.5–0.6×), 5px-out 0.23× (ref ≤0.35×); max channel 250, no saturation | ≤8 px return to body; no FWHM>6px; no >255 | **PASS** (vertical 1px-out slightly hot) |
| 4a | Body RGB | **(18,22,32)** median (patches (20,24,34) L / (17,21,31) R), lum 24 | (16,20,30)±6 | **PASS** |
| 4b | Rim + visibility vs navy host | 1–2px rim +12–16 lum over body; top specular (33,37,47); host navy (17,33,66) lum 38.7, B−G 33 vs body 14 → hue+lum separation, edge unambiguous | edge unambiguous in screenshot | **PASS** |
| 5a | Geometry (bonus) | peak:flank **8.8:1** (peak vext 88 vs flank 10 @2x); fill ratio 0.50 (ref 0.50); aspect 4.64 (ref 4.8) | peak ≥5×flank (7:1 ref); fill 0.4–0.6 | **PASS** |
| 5b | Motion honesty (bonus, gif) | frames 1–6: peak stable x234–235, G 238–255; dot-zone median vext 6px @2x (3 native — no full-height room tone); per-frame meanΔ 0.21–0.44 (slow breathing, no wobble). Frames 0/7 are a scene-cut seam (pill re-composited on dark backdrop), not pill motion | idle posed breathing, dominant peak, no wobble | **PASS** (stills/gif; ζ not directly measurable) |

Residual weakness concentrated in one place: **the peak-cluster base** — interior cluster gaps sit at
G 52–91 under bloom bleed (cluster reads 52px @2x fused at the >45 threshold; peak-adjacent gap
contrast ≈2.7:1 vs ref 3.7:1).

## 2. Scoreboard (scores from critic reports — placeholder)

| Lens | Score | 
|------|-------|
| COLOR | _/10 (from critic reports — do not invent scores) |
| GEOMETRY | _/10 (from critic reports — do not invent scores) |
| BLOOM | _/10 (from critic reports — do not invent scores) |
| MOTION | _/10 (from critic reports — do not invent scores) |
| COMPOSITION | _/10 (from critic reports — do not invent scores) |
| TASTE | _/10 (from critic reports — do not invent scores) |

**Evidence pack** (5 most decision-relevant measurements):

- **Peak G 250** (was 132; target ≥200, design-spec 242) — core RGB (217,250,250) white-hot, flanks teal R/G 0.66–0.67: bright-curve + white-hot lerp landed.
- **Gap contrast median 2.4–2.7:1** (was 1.01–1.06; target ≥3.4 median, min ≥2.5; measured min 1.60) — gaps legible for the first time, but still under target, worst inside the peak cluster (≈2.7:1 vs ref 3.7:1).
- **Midgap-vs-single-bar glow 1.09× median, 1.78× max** (target ≤1.05×) — bloom still bleeds across cluster-flank gaps; haze floor 90–110 (round 1) → midgap G 23–44.
- **Tail dots G 78–86 = body+56–64** (ref whisper-line body+17; 31–34% of peak vs ref 25–28%) — DOT_A 100 raised all dots; envelope taper compressed at the tails.
- **Body (18,22,32) vs navy host (17,33,66)** with +12–16 rim and top specular — dark-on-dark failure fixed; capsule edge unambiguous (round 1: body ≈ host, invisible). Bonus: peak:flank 8.8:1 (was 2:1, target 7:1); no channel saturation (max 250).

## 3. Delta — round 1 → round 2 per acceptance criterion

| Criterion | Round 1 | Round 2 | Target | 
|-----------|---------|---------|--------|
| Peak G | 132 | **250** | ≥200 (spec 242) ✅ |
| Peak-vs-body Δ | 78 (lum 117−39) | **228** (G 250−22) | ≥100 (spec 170) ✅ |
| Gap contrast (core/adjacent-gap) | 1.01–1.06 | **2.41 median excl-cluster / 2.67 all; min 1.60, max 3.89** | ≥3.4 median, min 2.5 ❌ improved, short |
| Midgap vs single-bar glow | ≈1.3× | **1.09× median / 1.78× max** | ≤1.05× ❌ marginal |
| Gap haze over body | +51…+71 (midgap lum 90–110 on body 39) | **+1…+22** (midgap G 23–44 on body 22) | within 20 ❌ marginal |
| Dot emission | dot≈gap (4 LSB, dots emit like gaps) | **dots +38–44 over their gaps; +56–64 over body** | whisper-line +17 ❌ |
| Halo extent | 135 px fused haze, never returns to body | **dies ≤1–2 native px horizontal, 0.23× at 5px vertical** | ≤8 px, 0.4× @5px ✅ |
| Saturation | 2.5–3× over | **max channel 250** | none >255 ✅ |
| Peak:flank | 2:1 | **8.8:1** | ≥5:1 (7:1 ref) ✅ |
| Body / visibility | (37,41,40) ≈ host navy, invisible | **(18,22,32), rim +12–16, specular; hue Δ B−G 14 vs 33** | edge unambiguous ✅ |
| Motion | per-frame max-norm full-height noise, 2–4 Hz wobble | **per-frame Δ 0.21–0.44, peak stable, no full-height noise** | posed idle, no wobble ✅ |

**Net:** 5 acceptance criteria clear PASS, 3 marginal/soft FAILs — all three are one residual: bloom
bleed + over-bright dots at the peak-cluster base and tail taper.
