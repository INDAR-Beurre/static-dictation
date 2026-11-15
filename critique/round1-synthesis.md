# Round 1 Synthesis — Pill Widget Fix Plan

Avg score: **3.8/10** across six lenses vs `critique/reference.png`.

## 1. Scoreboard

| Lens | Score | Root cause |
|---|---|---|
| COLOR | 3 | Luminance failure, not hue: live peak G≈54 vs ref ≈250; dots G≈44–50 nearly fused with body; bright=0.5+0.5·v keeps mids at half palette; BLOOM 0.85 over GLOW_K 4.5 spreads energy 4× too wide. |
| GEOMETRY | 3 | `glow += w` sums neighbors (mid-gap ≈1.3× single-bar glow) so gaps can't read; no dominant center peak (all elements within 1.5×); DOT_R_MIN 0.80 flattens envelope; bar fills 100% of pitch. |
| BLOOM | 4 | gw 0.15+0.85·v makes dots emit (dot G≈78 vs gap haze G≈74 = 4 LSB); unnormalized sum saturates 2.5–3×; (1.0−bcov_max) lift inverts contrast (glow brightest between cores). |
| MOTION | 3 | Per-frame max-norm renders room tone full-height; wobble 2–4Hz + 5 wraps = incoherent; springs fine — mush is vecmentor + canvas. |
| COMPOSITION | 5 | Dark-on-dark invisible; green-gray vs navy host; hangs below clock center; covers workspace badges; wave too dim. |
| TASTE | 5 | Jewel-is-fog (cores must read first, glow is garnish); obsidian-is-gray-plastic (body too light); idle is dead (most-lived state has no charm). |

## 2. Unified causal chain

- **Glow sum without normalization** → neighbor bars bleed into gaps (mid-gap ≈1.3× single-bar glow), gaps erased; unnormalized sum then saturates 2.5–3×.
- **Dot emission + no threshold** → every dot glows (4 LSB dot-vs-gap delta), and BLOOM 0.85 × GLOW_K 4.5 smears that fog outward — jewel becomes fog.
- **Per-frame max-norm** → room tone is normalized to full height; no peak floor → the true center peak vanishes at idle.
- **bright=0.5+0.5·v midpoint curve** → peaks never reach the top of the palette (G≈54 vs ≈250); dark-on-dark: body (35,38,37) ≈ host navy → capsule invisible.

## 3. Accepted fixes

| Fix | File | Mechanism | Demanded by |
|---|---|---|---|
| Bloom max-field + min(1.0) cap; threshold `smoothstep((v−0.35)/0.65)`; GLOW_K 4.5→2.5–3 (two-scale); BLOOM 0.85→0.5; teal bias | `src/pill/canvas.rs` | Per-band max before sum; only cores ≥0.35 emit; narrower falloff kills inter-bar fog | COLOR, BLOOM, GEOMETRY |
| Bright curve `0.35+0.65·v^0.6` + env floor 0.65 | `src/pill/canvas.rs` | Peaks reach top palette; envelope never collapses to zero | COLOR |
| White-hot peak: lerp to (255,255,250) at mix v² | `src/pill/canvas.rs` | Center peak reads white-hot like ref | COLOR, TASTE |
| Peak-emphasis gaussian on half_h | `src/pill/canvas.rs` | One dominant center element (breaks ≤1.5× parity) | GEOMETRY |
| ELEM_R_MAX 1.35→1.0; DOT_R 0.75→1.0; DOT_STEP 3.5→4.5; DOT_R_MIN 0.80→0.55 | `src/pill/canvas.rs` | Gap breathing room; tapered envelope restored; cores at pitch | GEOMETRY |
| DOT_A 60→100 + softer edge_dim | `src/pill/canvas.rs` | Cores brighten, edges feather — dots read before glow | COLOR, TASTE |
| BG (35,38,37)→(16,20,30); 1px rim 8–10%; top specular | `src/pill/canvas.rs` | Separation from navy desktop; obsidian not gray-plastic | COMPOSITION, TASTE |
| AGC slow_peak decay 0.995 + rms gate 0.01 | `src/pill/vecmentor.rs` | Norm over ~seconds, not per frame; silence gated | MOTION |
| wobble_rate 0.4→0.1; idle_phase_step 1.1→0.4; idle_pose 0.8 SHAPE_MASK blend | `src/pill/vecmentor.rs` | Slow coherent idle breathing + posed silhouette, not fuzz | MOTION, TASTE |
| ζ 0.28→0.45; overshoot 1.25→1.12 | `src/pill/tsar.rs` | Springs settle without jelly wobble | MOTION |
| Clock-center vertical alignment | `src/pill/pill.rs` | Pill orbits clock center, not below | COMPOSITION |

## 4. Deferred

- **SURF_W 220→170** — only if round 2 still shows workspace-badge occlusion.
- **Landing flash on spring settle** — charm pass after fidelity passes.
- **Idle target rate-limiting** — only if round 2 shows noise chasing through the gate.
- **Gamma-correct glow** — only if halos still band in round 2.

## 5. Round-2 acceptance criteria

1. **Peak luminance**: live peak G ≥ 200 (ref ≈250); peak-vs-body G delta ≥ 100.
2. **Gap legibility**: mid-gap glow ≤ 1.05× single-bar glow at its band height; gap haze G within 20 of capsule body G.
3. **Bloom discipline**: only bands with v ≥ 0.35 emit; halo spread ≤ 0.5× current width; no saturation beyond 1.0 anywhere.
4. **Visibility on navy**: body (16,20,30) vs host navy delta visible in screenshot; rim + specular make capsule edge unambiguous.
5. **Motion honesty**: idle shows slow posed breathing (≤0.4 phase step, no full-height noise), dominant center peak present in both idle and active; springs settle with ζ≈0.45 and no visible wobble after overshoot.
