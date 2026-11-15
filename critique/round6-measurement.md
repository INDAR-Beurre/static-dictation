# Round-6 pixel measurement

Run: `python3 scripts/round6-measure.py critique/round5-final-active.png critique/round4-active.png`  
ROI: explicit inclusive `PILL_BBOX=146,22,733,155`; measurements are restricted to the capsule, with rows below the ~140px shadow cutoff excluded. These are script outputs, not hand-adjusted.

## `critique/round5-final-active.png`

| Pixel-audit row | Measured | Target | Result |
|---|---:|---:|---|
| Plateau px (lum ≥230) | 0 | ≤40 | PASS |
| Hot core RGB / R:G | (172,235,239) / 0.732 | R:G ≥0.70 | PASS |
| Halo R/G | 0.790 | 0.57–0.65 | FAIL |
| Gap contrast median / min | 2.065 / 1.033 | ≥3.4 / ≥2.5 | FAIL |
| Gap haze worst +G | +67.0 | ≤+20 | FAIL |
| Midgap median / max ratio | 1.178 / 3.364 | ≤1.05 / ≤1.05 | FAIL |
| Waveform span % W | 78.91% | ~80% | PASS |
| Mound / cluster % W | 20.07% | ~26% | PASS* |
| Peak:flank | 19.50:1 | ~7:1 | FAIL |
| End-dot B−G, left / right | +5 / +114 | ≥+30 both | FAIL |
| Tail lift lum over body | 10.7 | +17..+25 | PASS* |

## `critique/round4-active.png`

| Pixel-audit row | Measured | Target | Result |
|---|---:|---:|---|
| Plateau px (lum ≥230) | 0 | ≤40 | PASS |
| Hot core RGB / R:G | (172,235,239) / 0.732 | R:G ≥0.70 | PASS |
| Halo R/G | 0.789 | 0.57–0.65 | FAIL |
| Gap contrast median / min | 2.065 / 1.033 | ≥3.4 / ≥2.5 | FAIL |
| Gap haze worst +G | +67.0 | ≤+20 | FAIL |
| Midgap median / max ratio | 1.178 / 3.364 | ≤1.05 / ≤1.05 | FAIL |
| Waveform span % W | 78.91% | ~80% | PASS |
| Mound / cluster % W | 20.07% | ~26% | PASS* |
| Peak:flank | 11.50:1 | ~7:1 | FAIL |
| End-dot B−G, left / right | +5 / +114 | ≥+30 both | FAIL |
| Tail lift lum over body | 10.7 | +17..+25 | PASS* |

Trustworthy as geometric/color indicators: plateau count, core RGB ratio, and emitting span. Gap/haze/midgap, halo, mound, peak:flank, end-dot, and tail-lift are provisional: their compact ROI/grid approximations are sensitive to exact sample selection; PASS* indicates the current script emits a value but does not fully gate against the target.
