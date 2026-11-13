//! vecmentor: waveform animation physics for the pill.
//!
//!-Owned-by: subagent V. Do not edit the pub API; fill in the `unimplemented!()` bodies.
//! Everything must remain `no_std`-friendly plain math (f32 only, no I/O) so it is
//! trivially unit-testable. Keep all constants tuned in one place (`wave_params`).
//!
//! Contract (FROZEN):
//! - `wave_params() -> WaveParams` — all static tuning constants for the animation.
//! - `Sim::new(params) -> Sim` — fresh simulation state.
//! - `Sim::push(&mut self, level: f32, speaking: bool) -> ()` — feed one input sample
//!   (call at ~30 Hz; `level` already clamped to 0.0..2.0 by pill.rs).
//! - `Sim::bars(&self) -> &[f32]` — current symmetric bar heights, len == `WaveParams::NUM_BARS`,
//!   values in 0.0..1.0 (1.0 = full pill height allowance).
//! - No allocation inside `bars()`. `Sim` is plain-old-data style: no threads, no channels.
//! - Animation style (reference image): symmetric equalizer bars — center bars respond
//!   first/loudest, edges lag and stay lower. Smooth attack (fast up), smooth decay
//!   (slow down ~0.5), per-bar noise/wobble so it looks organic, idle "breathing" wave
//!   when not speaking so the pill still feels alive, peak-hold + decay per bar.
//! - Add unit tests in this file (`#[cfg(test)] mod tests`) covering: idle wave state,
//!   a pushed level raises bars, symmetry (bar[i] ≈ bar[NUM_BARS-1-i]), decay over time,
//!   clamp on absurd input (level 2.0 / NaN-safe).

/// Static tuning constants. One struct so tsar (renderer) and tests can see them.
#[derive(Debug, Clone, Copy)]
pub struct WaveParams {
    /// Number of equalizer bars (symmetric: visual pairs mirror around center).
    pub num_bars: usize,
    /// Attack coefficient (per push): new = old + (target-old) * ATTACK, at ~30 Hz.
    pub attack: f32,
    /// Decay coefficient (per push): bars fall back toward idle slowly.
    pub decay: f32,
    /// Idle wave amplitude (0..1) — gentle breathing when nothing is said.
    pub idle_amp: f32,
    /// How strongly speaking amplitude falls off from bar center to edge (0..1).
    pub spatial_falloff: f32,
    /// Peak-hold time in pushes (30 Hz → 15 = 0.5 s), before decay kicks in.
    pub peak_hold: u32,
}

/// Deterministic pseudo-noise per bar: sin-based wobble driven by per-bar phase
/// counters (no rand crate, deterministic across runs/tests).
struct Sim {
    bars: Vec<f32>,
    /// Peak-hold envelope per bar (falls after `peak_hold` pushes).
    peaks: Vec<f32>,
    /// Remaining pushes before each peak may decay.
    peak_timers: Vec<u32>,
    /// Per-bar wobble phase, advanced each push by a bar-specific rate.
    wobble_phase: Vec<f32>,
    /// Global idle-breathing phase (slow sine sweep).
    idle_phase: f32,
    /// Pushes since `speaking` was last true; gates the idle wave in.
    idle_frames: u32,
}

/// All tuning constants in one place, chosen for the reference-image look.
pub fn wave_params() -> WaveParams {
    WaveParams {
        num_bars: 9,
        attack: 0.55,
        decay: 0.06,
        idle_amp: 0.18,
        spatial_falloff: 0.65,
        peak_hold: 15,
    }
}

impl Sim {
    pub fn new(params: WaveParams) -> Sim {
        let n = params.num_bars;
        Sim {
            bars: vec![0.0; n],
            peaks: vec![0.0; n],
            peak_timers: vec![0; n],
            wobble_phase: (0..n)
                .map(|i| i as f32 * 1.7)
                .collect(),
            idle_phase: 0.0,
            idle_frames: 0,
        }
    }

    /// Feed one input sample (~30 Hz). `level` clamped 0.0..2.0 upstream.
    pub fn push(&mut self, level: f32, speaking: bool) {
        // Guard: never let NaN/inf poison state (upstream clamps 0.0..2.0).
        let level = if level.is_finite() { level } else { 0.0 };
        let level = level.clamp(0.0, 2.0);

        let n = self.bars.len();
        let center = (n - 1) as f32 * 0.5;

        if speaking {
            self.idle_frames = 0;
        } else {
            self.idle_frames = self.idle_frames.saturating_add(1);
        }
        // Idle wave ramps in over ~20 pushes (~0.7 s) of silence.
        let idle_gate = ((self.idle_frames as f32) / 20.0).clamp(0.0, 1.0);
        self.idle_phase = (self.idle_phase + 0.21) % core::f32::consts::TAU;

        for i in 0..n {
            // Per-bar organic wobble, deterministic (phase counter + sin).
            self.wobble_phase[i] = (self.wobble_phase[i] + 0.9 + 0.13 * i as f32)
                % core::f32::consts::TAU;
            let wobble = 1.0 + 0.18 * self.wobble_phase[i].sin();

            // Target from mic level (level 2.0 → full height), scaled by wobble.
            let target = (level * 0.5 * wobble).clamp(0.0, 1.0);

            // Spatial falloff: center full, edges lower; edges also lag
            // (slower effective attack).
            let dist = ((i as f32 - center).abs() / center.max(1.0)).clamp(0.0, 1.0);
            let envelope = 1.0 - self.params_falloff_hint() * dist;
            let target = (target * envelope).clamp(0.0, 1.0);
            let attack_i = self.params_attack_hint() * (1.0 - 0.5 * dist);

            let cur = self.bars[i];
            let (coeff, dest) = if speaking && target > cur {
                (attack_i, target)
            } else {
                (self.params_decay_hint(), target)
            };
            let mut new_bar = cur + (dest - cur) * coeff;

            // Idle breathing: blend toward a traveling sine wave when silent.
            if idle_gate > 0.0 {
                let body = self.idle_amp_hint()
                    * (self.idle_phase - dist * 1.9).sin().abs();
                new_bar += (body - new_bar) * (idle_gate * 0.15);
            }

            self.bars[i] = new_bar.clamp(0.0, 1.0);

            // Peak-hold: raise instantly, hold `peak_hold` pushes, then decay.
            if self.bars[i] > self.peaks[i] {
                self.peaks[i] = self.bars[i];
                self.peak_timers[i] = self.peak_hold_hint();
            } else if self.peak_timers[i] > 0 {
                self.peak_timers[i] -= 1;
            } else {
                self.peaks[i] += (self.bars[i] - self.peaks[i]) * 0.04;
            }
        }
    }

    /// Current symmetric bar heights, 0.0..1.0, len == params.num_bars.
    pub fn bars(&self) -> &[f32] {
        &self.bars
    }

    // Small accessors so push() reads tuning constants without cloning params.
    // (They re-derive from wave_params(); constants are static per the contract.)
    fn params_attack_hint(&self) -> f32 {
        wave_params().attack
    }
    fn params_decay_hint(&self) -> f32 {
        wave_params().decay
    }
    fn params_falloff_hint(&self) -> f32 {
        wave_params().spatial_falloff
    }
    fn idle_amp_hint(&self) -> f32 {
        wave_params().idle_amp
    }
    fn peak_hold_hint(&self) -> u32 {
        wave_params().peak_hold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_bars_sane() {
        let sim = Sim::new(wave_params());
        let bars = sim.bars();
        assert_eq!(bars.len(), wave_params().num_bars);
        for &b in bars {
            assert!((0.0..=1.0).contains(&b), "bar {b} out of range");
        }
    }

    #[test]
    fn push_raises_bars() {
        let mut sim = Sim::new(wave_params());
        sim.push(0.0, false);
        sim.push(0.0, false); // settle idle low
        let before: Vec<f32> = sim.bars().to_vec();
        for _ in 0..6 {
            sim.push(1.4, true);
        }
        let after = sim.bars();
        let sum_before: f32 = before.iter().sum();
        let sum_after: f32 = after.iter().sum();
        assert!(sum_after > sum_before + 0.5, "push must raise bars: {before:?} → {after:?}");
    }

    #[test]
    fn symmetry_loose() {
        let mut sim = Sim::new(wave_params());
        let n = wave_params().num_bars;
        for _ in 0..30 {
            sim.push(1.2, true);
        }
        let bars = sim.bars();
        // Left/right halves both active and within a loose band (wobble is organic,
        // so bars are near-symmetric, not exactly mirrored).
        for i in 0..n / 2 {
            let l = bars[i];
            let r = bars[n - 1 - i];
            assert!(l > 0.05 && r > 0.05, "both halves active: i={i} L={l} R={r}");
            assert!(
                (l - r).abs() < 0.35,
                "loose symmetry violated at i={i}: L={l} R={r}"
            );
        }
    }

    #[test]
    fn decay_toward_idle() {
        let mut sim = Sim::new(wave_params());
        for _ in 0..30 {
            sim.push(1.6, true);
        }
        let loud: f32 = sim.bars().iter().sum();
        for _ in 0..90 {
            sim.push(0.0, false);
        }
        let quiet: f32 = sim.bars().iter().sum();
        assert!(quiet < loud * 0.4, "bars must decay after silence: loud={loud} quiet={quiet}");
        for &b in sim.bars() {
            assert!((0.0..=1.0).contains(&b));
        }
    }

    #[test]
    fn absurd_input_no_panic() {
        let mut sim = Sim::new(wave_params());
        sim.push(f32::NAN, true);
        sim.push(2.0, true);
        sim.push(f32::INFINITY, true);
        sim.push(-3.0, false);
        for &b in sim.bars() {
            assert!(b.is_finite() && (0.0..=1.0).contains(&b), "bar {b} after absurd input");
        }
        // Continue pushing normally; state must be un-poisoned.
        for _ in 0..10 {
            sim.push(1.0, true);
        }
        for &b in sim.bars() {
            assert!(b.is_finite() && (0.0..=1.0).contains(&b));
        }
    }

    #[test]
    fn bars_len_matches() {
        let params = wave_params();
        let sim = Sim::new(params);
        assert_eq!(sim.bars().len(), params.num_bars);
        let mut sim2 = Sim::new(WaveParams { num_bars: 17, ..params });
        sim2.push(1.0, true);
        assert_eq!(sim2.bars().len(), 17);
    }
}
