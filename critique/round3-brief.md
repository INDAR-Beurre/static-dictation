# Round 3 brief — pill fidelity push

Goal: 10/10 vs `critique/reference.png`. Round 2 committed `ebafd09`, scored **7.0/10 avg**
(taste 7, motion 8, color 7, bloom 7, geometry 6; pixel audit `critique/round2-scores.md`).
Targets + PIL probe recipe: `critique/targets.md`, `critique/design-spec.md`.

## Ownership (shared checkout — touch ONLY your files)

| Agent | Files |
|---|---|
| canvas fixer | `src/pill/canvas.rs` only |
| motion fixer | `src/pill/vecmentor.rs` + `src/pill/tsar.rs` |
| harness fixer | `scripts/*`, additive flag in `src/main.rs`, `src/pill/preview.rs` |
| ref-measure | read-only → `critique/ref-deep-measure.md` |
| matrix | doc only → `critique/round3-acceptance.md` |

## C — canvas.rs (waveform + color/bloom gates + glass edge)

- **C1 white-hot plateau kill.** Today: 256px flat (217,250,250) slab, 64 rows (x234–237 × y14–77 crop), second tier (177,230,231) 272px — ~20× ref's clipped area (ref: 13px ≥ lum 210, zero plateau; hottest ref pixel (171,239,238), R/G 0.72). Fix: field ≥ 1.0 renders **solid teal ≈(171,239,238)**; bleach only the extreme tip: `white_mix ≈ 0.35·smoothstep(0.90, 0.99, field)`. Target: px ≥ lum 230 → ≤ ~40; no flat plateau > 5 rows; core R/G ≥ 0.70.
- **C2 halo tint.** Glow pass samples post-lerp white (halo R/G 0.75–0.82; ref 0.57). Sample the **pre-lerp teal ramp** (or multiply halo RGB ×(0.82,1.0,1.02)). Target halo R/G ≈ 0.57–0.65, B/G ≈ 1.05–1.17 (already right).
- **C3 span.** Leader loop end `hw_c − 2.5·s` → `−15.0·s` → waveform span ~80% of W (live 96.3%, ref 78.8%, margins L 8.2% / R 13%).
- **C4 cluster width.** Peak gaussian `1+1.5·exp(−dc²/4.5)` → σ² ≈ 19, gain 1.5→1.0. Target cluster ~26% of W (live 11.4%, ref 26.1%), ~12 tall columns, mound profile like ref's 54/74/84/74/54 — a broad mound, not a thin spike.
- **C5 flank height.** env floor 0.65 → 0.75. Target peak:flank ≈ 7:1 (live ~17:1, ref ~7.3:1); flanks 7–11% of cap H.
- **C6 dropouts.** Three holes with no visible element: crop x30–38, x70–87, x383–401 (interior max G < 30, neighbors 64–85). Audit the slot loop — every slot renders ≥ DOT_R_MIN element; dot line unbroken across the full span (ref is unbroken at 4.0 pitch).
- **C7 pitch.** DOT_STEP 4.5 → 3.7 logical (live 4.84 vs ref 4.0; finer grain also fixes the ~9-dots-vs-25–30 "cheap execution" tell).
- **C8 tail blue.** Ramp-blue weight ×~2.5 in the 55–85% zone; tail cores ≈ (90,125,175); end-dot bias floor B−G ≥ +30 (live +6…+20, ref +28…+58; ref end dot (60,85,131)). Mid-height bars lean cyan (B>G ~+8), not mint (G≈B).
- **C9 tail whisper.** Tail dot glow lift over body → ~+17–25 lum (live +56–64, ref whisper +17); keep ≥ +10 visibility floor. This also scythes the over-bright cluster base (scorer's root cause: cluster reads 52px fused).
- **C10 emit gate: KEEP** `smoothstep((v−0.35)/0.65)` — do NOT raise to 0.40; it would kill the idle halo (C11). Flank contrast improves via C1/C4/C9 instead.
- **C11 idle halo hum.** Silence shows a dim teal halo on the posed peak, breathing ±20% in phase, ≈20–30% of the active peak's halo alpha. Requirement, not mechanism — canvas already receives the breathing signal via v; implement so the posed idle hums without shouting.
- **C12 glass-edge objecthood** (taste's biggest gap — capsule currently reads as a void darker than the navy host): 1px top keyline ≈ rgba(140,160,175,0.85), darker bottom edge line, elliptical specular streak on the upper half (stronger than the current 6px roll-off), soft blurred drop shadow under the capsule. Fits inside the existing surface (capsule bottom ≈48.5 phys of 95). If `critique/ref-deep-measure.md` exists when you integrate, use its shadow/keyline numbers; else taste's spec above.
- **C13 keep:** peak on capsule center (x≈232 crop), contrast direction (cores > gaps everywhere), body navy (16,20,30) + rim + specular, no channel > 255, field cap 1.0.
- Re-anchor affected tests honestly (taller_bar, wave_gradient_stops, third_peak, body_bg, idle_dots, twin blend) — document expected shifts.

## M — vecmentor.rs + tsar.rs

- **M1 idle breathe.** idle_rate 0.175 → ~0.075 (period ≈ 2.8 s; criterion ≥ 1.5 s). Posed peak breathes ±8–10% of posed height. **Pose height → ~35–45% of cap** (silence hums, not shouts — taste) while KEEPING the silhouette shape (dim dots, ramp, dip, center cluster) — scale height, not shape.
- **M2 twinkle.** wobble_amp 0.1 → 0.05 (one rim column popped 0→26px for a single frame — 6 LSB AA-threshold crossing).
- **M3 top cap.** `tsar.rs:337 set_margin(-6,0,0,0)` → `(0,0,0,0)`: grown-cap top lands ~3 logical px below y=0 — kills the flat-cut top (y=0 row spans 87% of max width). Verify bottom edge + entrance tests still pass (placement-only).
- **M4 hold (do not regress):** silence gate byte-exactness, springs ζ 0.45 / ceil 1.12 / kick 4.6, profile corr ≥ 0.998.
- Re-anchor idle tests (`silence_converges_to_idle_pose`, `idle_posed_center_breathes`) honestly for the new pose height/period.

## H — harness (scripts/ + additive hooks)

- **H1** in-process frame dump at fixed 100ms intervals to PNGs (grim at ~0.3s/shot aliases the 1.2–2.8s cycles). Preferred: additive env-gated flag (e.g. `STATIC_DICTATION_DUMP_FRAMES=<dir>`) driving the existing preview/canvas render path — zero behavior change otherwise.
- **H2** script building a 100ms gif + motion sheet from the dump (mean|ΔH|/step, profile corr, breathing period estimate).
- **H3** after integration + commit (coordinator pings you), run the live proof: hold + released captures + 100ms dump → rebuild side-by-side + live-anim gif.

## R — ref deep measure (read-only → critique/ref-deep-measure.md)

R1 drop shadow under ref capsule: present? offset/blur/extent/alpha. R2 flank dot alpha ramp (whisper curve end→center). R3 cluster census at matched scale (count, heights, profile). R4 top keyline/specular present? sample values. R5 anything the round-2 verdicts left unspecified that canvas needs (esp. 90–100% zone colors).

## A — acceptance matrix (doc → critique/round3-acceptance.md)

Per lens: criterion / ref target / round-2 measured / round-3 threshold / owner. Round-2 measured anchors: peak G 250 PASS; ΔG body 228 PASS; midgap 1.09× med / 1.78× max FAIL(≤1.05); gap haze +22 FAIL(≤20); gap contrast 2.4–2.7 FAIL(≥3.4); peak:flank 17:1 (ref 7.3); span 96.3% (ref 78.8%); cluster 11.4%W (ref 26.1%); pitch 4.84 (ref 4.0); halo R/G 0.75–0.82 (ref 0.57); plateau 280px ≥ lum230 (ref 13px ≥210); breathe ±1.5px/1.2s (spec ±8–10%/≥1.5s); pose full-height (spec 35–45%); top cap clipped (margin −6).
