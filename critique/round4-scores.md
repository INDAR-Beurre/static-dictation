# Round 4 scores — committed evidence

| Lens | Score /10 | Basis |
|---|---:|---|
| Color | 5 | Core/body close; halo too white, left tail fails blue floor, rim is hot. |
| Geometry | 6 | Capsule proportions and centered mound are close; dot line/taper and cluster shape still diverge. |
| Bloom | 3 | Halo fuses the active cluster; gaps and idle dots glow too strongly. |
| Motion | 7 | Period 2.60s and slow breathing are good; ±11% vs ±8–10% and profile correlation 0.9951 miss. |
| Taste | 5 | Reads as a considered dark pill, but active glow and dotted idle train are too loud. |
| Pixel-audit | 4 | Some color/body checks pass, but multiple measured gates fail and probe masks are flagged unreliable. |
| **Average** | **5.0** | **Not 10/10.** |

Scores assess committed d5020d1 + b8403f5 evidence, not concurrent uncommitted edits. A canvas fix is in flight; rescore its rendered output before treating this as final.

Hard constraints: footprint audit reports ~3.6 MiB shipped release, under 50 MB; app implementation is Rust (QA scripts are not runtime); fluidity audit reports ~33 Hz live updates, ~7 ms worst render, and ~4× frame-budget headroom. AltGr+L was live-injected and toggled successfully on this host; the report is conditional on device/kernel code 100, uinput, and grab availability—not universal hardware proof.