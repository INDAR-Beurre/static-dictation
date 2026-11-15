# Round 9 — final motion critique

**Score: 8/10**

| Motion check | Result | Evidence |
|---|---|---|
| Idle breath is fluid and alive | **PASS** | In-process 159-frame measurement: mean |ΔH| 0.027 px/step, profile correlation 0.9988, ±8.8% excursion, 2.80 s cycle. These are slow, coherent changes—not random jitter or a static pose. |
| Idle pop gate | **PASS** | Worst per-column step 2.0 px against ≤3 px gate. |
| Voiced animation after onset | **PASS** | The active GIF has 99 frames; its settled waveform changes gradually and reads as breathing/pulsing rather than random bar flicker. |
| Voiced onset pop | **FAIL** | GIF frame 0→1 jumps 60→88 px peak height; summed per-column height change is 1,428 px, with one column changing 0→80 px. That is a conspicuous entrance pop, not the specified ≤3 px behavior. |

### Top remaining fixes
1. Smooth the voiced onset: initialize/warm the spring from the idle pose or ramp the speech target in, rather than starting voice from zero (`src/pill/vecmentor.rs:226-243`). Recheck the first active GIF transition; steady-state smoothness is already good.
2. Keep tuning the live voiced shoulders against the reference: the measured 22:1 peak-to-flank ratio remains much sharper than the reference’s ~7:1 despite the broader R9 mask (`src/pill/vecmentor.rs:182-191`). Validate on real speech as well as the QA envelope before changing its pedestal.

**What is right:** idle breathing is exceptionally calm and perceptible, with measured cadence and per-column motion comfortably inside the gates; the voiced animation is fluid once it gets past its initial pop.
