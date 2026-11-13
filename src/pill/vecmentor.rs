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

/// The simulation. Owns bar states; `bars()` must not allocate.
pub struct Sim {
    // TODO(subagent V): bar states, phases, peak-hold timers, etc.
}

/// All tuning constants in one place, chosen for the reference-image look.
pub fn wave_params() -> WaveParams {
    unimplemented!("subagent V: fill in tuning constants for the reference-image look")
}

impl Sim {
    pub fn new(params: WaveParams) -> Sim {
        let _ = params;
        unimplemented!("subagent V: init state")
    }

    /// Feed one input sample (~30 Hz). `level` clamped 0.0..2.0 upstream.
    pub fn push(&mut self, level: f32, speaking: bool) {
        let _ = (level, speaking);
        unimplemented!("subagent V: advance animation one tick")
    }

    /// Current symmetric bar heights, 0.0..1.0, len == params.num_bars.
    pub fn bars(&self) -> &[f32] {
        unimplemented!("subagent V: return current bar heights")
    }
}
