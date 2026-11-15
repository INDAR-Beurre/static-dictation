# Round-3 reference deep-measure — `critique/reference.png` (1024×572)

Method: PIL+numpy, lum = mean(RGB), G = green channel. Probes: column/row threshold scans
(lum<100 vs bright host), per-column peak in band y26..54, per-slot 4px-cell argmax, deficit
maps vs lateral control columns, FWHM run-lengths. Crops saved: `critique/scratch/ref-r3-context-3x.png`,
`ref-r3-capsule-5x.png`, `ref-r3-shadow-4x.png`, `ref-r3-blcorner-6x.png`, `ref-r3-brcorner-6x.png`.

## ⚠ GEOMETRY CORRECTION (supersedes round-1)

Capsule bbox measured **x 419.0–604.7, y 14.8–62.4 → W≈186, H≈48, ratio 3.9:1, corner r≈24 (full-round)**.
Round-1's "182×38, 4.8:1" used a wrong bottom edge (y52): body is solidly present (lum 30–46) down to y62.
Edge evidence (x=470): y62 lum 42 (body), y63 lum 116 (host AA), y64+ = host/shadow.
**All canvas scaling that assumed 4.8:1 (C3 span, C5 flank %H, M1 pose 35–45% of cap H) must re-anchor to H=48, r=24.**
Host bg here is bright sky/cloud (not the live app's navy) — capsule contrast vs host is even higher than assumed.

## R1 — Drop shadow: PRESENT (soft, below, biased left)

Bottom edge = y62.4. Deficit = control lum − measured lum (control: left x370–398 cloud rows; right x612–634 sky rows).

| below edge (px) | deficit x440–460 (cloud bg ~248) | α (of bg) | deficit x540–580 (blue bg ~125) | α |
|---|---|---|---|---|
| +1 (y63, edge AA) | +116…+133 | 0.47–0.53 | +76…+95 | — (AA) |
| +2 (y64) | +86 | 0.35 | ~0 (bg boundary) | — |
| +4 (y66) | +78…+86 | 0.32–0.35 | +17…+21 | 0.15 |
| +6 (y68) | +63…+68 | 0.26 | +12…+19 | 0.12 |
| +8 (y70) | +51…+55 | 0.21 | +3…+11 | 0.07 |
| +10 (y72) | +38…+39 | 0.15 | ≤ +7 | 0.05 |
| +12 (y74) | +22…+26 | 0.10 | ~0 | 0 |
| +14 (y76) | +9…+18 | 0.06 | 0 | 0 |
| +16…+20 (y78–82) | +5…+12 | ≤0.04 | 0 | 0 |

- Lateral: left of capsule rows 30–50 → deficit 15–19% at 2–5px out, ~4% at 9px out. Right side → ≤5%, gone by 8px.
- **Direction: strongest under left-center (x≈430–470), reaches ~18–20px below edge; light from upper-right.**
- Color: subtractive, slightly blue-preserving: cloud 249 → shadowed (140,156,170) at +4px; neutral-dark multiply, tint ≈ rgba(10,20,35).
- **Constants: offset (−3, +2) px, Gaussian σ ≈ 5.5 px, peak α ≈ 0.35 at bottom edge, half-decay ≈ 6px, visible extent ≈ 18px below / 8–10px left / 4–6px right.**

## R2 — Flank dot alpha ramp (whisper curve), dot line y-center 39 (51% of cap H)

36 dots, x434→581, pitch 4.08 (σ≈0.3). Body G: left 44, right 32. Peak G 242.

| pos% (0=first dot x434, 100=last x577) | x | core G | ΔG over body | G %peak |
|---|---|---|---|---|
| 0 | 434 | 60 | +16 | 25% |
| 2.8 | 438 | 68 | +24 | 28% |
| 5.6 | 442 | 83 | +39 | 34% |
| 8.4 | 446 | 88 | +44 | 36% |
| 11.9 | 451 | 93 | +49 | 38% |
| 14.0 | 454 | 104 | +60 | 43% |
| 17.5 | 459 | 122 | +78 | 50% |
| 20.3 | 463 | 141 | +97 | 58% |
| 23.1 | 467 | 167 | +123 | 69% |
| 25.9 | 471 | 184 | +140 | 76% |
| 51.7–60.1 | 508–520 | 238–242 | — | 98–100% |
| 74.1 | 540 | 125 | +93 | 52% |
| 76.9 | 544 | 112 | +80 | 46% |
| 79.7 | 548 | 121 | +89 | 50% |
| 82.5 | 552 | 100 | +68 | 41% |
| 85.3 | 556 | 85 | +53 | 35% |
| 88.1 | 560 | 72 | +40 | 30% |
| 90.9 | 564 | 62 | +30 | 26% |
| 94.4 | 569 | 74 | +42 | 31% |
| 97.2 | 573 | 70 | +38 | 29% |
| 100 | 577 | 60 | +28 | 25% |
| (+102.8) | 581 | 52 | +20 | 21% |

- **Zones: 0–20% climbs 25%→58% of peak G (steep); 80–100% sags 41%→25% (gentler, slight jitter).**
- **End treatment: HARD STOP. Last dot ~21–25% alpha, next slot = body. No taper-to-zero tail.**
- Line does NOT reach capsule edges: margins inner-edge→first/last dot = L 15px (8.1% W), R 23.5px (12.6% W); span 147px = 79% W.
- Every slot in 434..581 occupied (no holes); gaps between tail dots ≈ body (G 32–44).

## R3 — Cluster census (slots at 4.08 pitch; height = rows ≥lum100 incl. halo, y26..55)

| slot x | 471 | 475 | 479 | 483 | 487 | 491 | 495 | 499 | 503 | 508 | 512 | 516 | 520 | 524 | 528 | 532 | 536 | 540 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| h px (≥100) | 2 | 3 | 3 | 5 | 5 | 3 | 3 | 3 | 3 | 5 | 8 | 14 | 21 | 17 | 12 | 7 | 5 | 3 |
| peak G | 184 | 194 | 214 | 222 | 211 | 207 | 188 | 186 | 182 | 239 | 242 | 230 | 238 | 221 | 190 | 167 | 142 | 125 |
| peak G % | 76 | 80 | 88 | 92 | 87 | 86 | 78 | 77 | 75 | 99 | 100 | 95 | 98 | 91 | 79 | 69 | 59 | 52 |

- Tall-column counts: ≥8px: **5** (x512–528); ≥5px: 10; ≥3px: 16; ≥2px: 18.
- Widths (of W=186): ≥8px zone x491–540 ≈ 52px = **28% W** (≈ round-2's 26.1% anchor); ≥2px zone x471–540 ≈ 71px = 38% W; mound top (≥14px) x516–524 = 17px = 9% W.
- **Peak bar x520: 21px solid (y30–50) + 1px AA tip = 46% of cap H, centered at 54% of W span.**
- Flank bars (x454–463): 2–3px → peak:flank ≈ 21:3 = **7:1** ✓ (round-2 anchor holds).
- Mound is a smooth envelope over an intact dot line — not discrete spikes; slots 471–499 are "tall dots" (3–5px), 508–540 are bars.

## R4 — Top keyline / specular: ABSENT. Bottom edge: ABSENT.

- Top-edge vertical profile (x445/470/500), lum: y14: 133/129/107 → y15: 84/83/89 → y16: 83/83/78 → y17: 70/71/67 → y18: 63/61/60 → y20: 55/50/51 → y24: 41/42/42. Monotonic decay into body; **no row brighter than the AA blend ⇒ no rim line**. Edge AA is actually a soft dark crease: y15 measured 82–93 vs linear-blend prediction ≈128.
- Keyline RGB y15–16 across x440–580: (70–82, 74–92, 85–110) — never exceeds host sky (lum 123–124) or cloud (215–248).
- Interior (y20–30, x425–600): mean 44.9, p95 63, max 63 (excluding edge AA) ⇒ **no specular streak**. Same for y52–60: mean 42.9, p95 55.
- Bottom: y58–61 = 42–51 lum (≈body), y62 = 42–47, no darker-than-body line.
- Body shading instead: soft diagonal falloff, top-left (42,43,45) → bottom-right (30,31,32) (Δ≈−11 lum L→R). That plus the R1 shadow is the entire "objecthood" cue in the ref.
- **If C12 keyline/specular are added anyway for taste, keep far below brief values (ref supports none); shadow numbers above are the measured replacement.**

## R5 — Canvas constants the round-2 verdicts left open

- **Pitch: 4.08 px mean** (147px / 36 dots), range 3–5, σ≈0.3 → keep 4.0 nominal ✓.
- **Dot width: FWHM ≈ 2–2.5px** (e.g. x434: 55/59.7/55 over 433–435; x556: 107/85/132 pair) → fill ≈ 0.55 of pitch. **Bar width: FWHM ≈ 2.5–3px** (x516 y44: 132/198/142; x520 y44: 128/211/156).
- **Dot height: 1px core + AA (≈1.5–2px dia); bright dots swell to 3px via glow only.**
- **90–100% height zone (bar tips, x520/524/516):** top tip y+0: (143,206,216)/(117,177,190)/(115,177,182); y+1: (152,224,233)/(128,201,215)/(152,224,229). Bottom fade: (147,211,221)→(114,165,176) over 2px. **Core is constant along the bar; only 1–2px AA tip fade, desaturating to ≈(140,205,215), R/G 0.62→0.69, B/G 1.02→1.05. No white bleach: max G in whole band = 242, top-10 G = 235–242.**
- **Glow extents:** above peak tip: 94.7 lum at −1px, ~65 at −2..3px, body by −4px. Beside bars (gap, y36–42): lum 65–116, G 80–116 → **intra-cluster gap contrast G core/adjacent-gap-min = 2.05–2.99 (mean ≈2.4), NOT the 3.4–3.7 round-1 claimed — the ref cluster is a broad mound with lit gaps; tail dots keep 2.0–2.6 (85 vs 32–43).**
- Tail hues: R/G steady 0.70–0.85 whole line; B/G: 1.0–1.05 (0–66%), 1.14–1.42 (66–91%), 1.13–1.23 (94–100%). End dots: (60,85,115)@85%, (52,72,100)@88%, (45,62,88)@91%, (51,60,72)@100% → B−G = +30…+26 even at the last dot ✓ C8 floor holds to the end.
- Body RGB for constants: **(42,43,45) top-left → (30,31,32) bottom-right**, lum 39–44 mid; edge crease ≈ 0.6× blend.

## Paste-ready constants (ref scale; capsule 186×48, r24)

```
CAPSULE        W=186, H=48, r=24, dotline_y = 0.51*H from top
BODY           (42,43,45) TL -> (30,31,32) BR diagonal; edge crease 0.6x blend; NO keyline/specular/bottom-line
SHADOW         offset(-3,+2)px, blur sigma 5.5px, peak alpha 0.35, tint rgba(10,20,35), extent 18px below / 9px left / 5px right
DOTS           pitch 4.08, FWHM 2.2, span 79%W, margins L 8.1% / R 12.6%, y-center 51%H
WHISPER_RAMP   pos%:G%peak = 0:25, 5:34, 10:37, 14:43, 20:58, 26:76 ... 74:52, 85:35, 91:26, 100:25(+last-dot 21); HARD END, no tail fade
CLUSTER        18 slots x471..540; heights(px): 2,3,3,5,5,3,3,3,3,5,8,14,21,17,12,7,5,3; peak 21px @54%W; >=8px zone 28%W; gap contrast ~2.4 (mound, lit gaps)
BAR            FWHM 2.7; core constant, tip AA 1-2px -> (140,205,215); max G 242; glow: 0.4x at 1px, 0.28x at 3px, 0 by 4px above tip
HUES           R/G 0.72-0.85 all; B/G 1.0 (teal) -> 1.4 (66-91%) -> 1.2 (tail); end dots B-G >= +26
```

Commands: threshold scans `lum<100` per row/col for bbox; per-column `band[26:54].max(axis=0)` for cores;
4px-cell argmax for slot census; deficit maps vs `x370:398` (left) / `x612:634` (right) controls for shadow;
FWHM via half-max run-length on rows y39/y40/y44.
