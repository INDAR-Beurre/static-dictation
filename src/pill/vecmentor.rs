//! vecmentor: waveform animation physics for the pill.
//!
//! Production rebuild: the pill renders a SYMMETRIC 9-band equalizer driven by
//! real spectral energies (`crate::bands::energies`, computed in main.rs per
//! 30 ms frame). This module owns all motion physics; it is std/core-only,
//! zero crate imports, no I/O, plain-old-data — trivially unit-testable.
//!
//! Contract (FROZEN — other agents code against this):
//! - `NUM_BARS == 9`, band centers `[250,350,500,700,1000,1400,2000,2800,4000] Hz`.
//! - `wave_params() -> WaveParams` — all static tuning constants in one place.
//! - `Sim::new(params) -> Sim` — fresh simulation state.
//! - `Sim::push(&mut self, level: f32, bands: &[f32; NUM_BARS], speaking: bool)`
//!   — feed one frame at ~30 Hz while recording. `level` = broadband RMS
//!   clamped 0.0..2.0. `bands[i]` = normalized 0..1 energy of band i.
//!   `speaking` = current VAD voice flag. NaN/inf inputs never poison state.
//! - `Sim::bars(&self) -> &[f32]` — current bar heights, len == NUM_BARS,
//!   values 0.0..1.0 (1.0 = full pill height allowance). No allocation.
//!
//! Physics (what makes the equalizer feel alive):
//! 1. SYMMETRIC MIRROR: bar i is driven by bands[min(i, NUM_BARS-1-i)] —
//!    center bar = voice core (1 kHz), edges = low freqs. The mirrored pair
//!    shares the band but gets a small per-bar stiffness jitter so it never
//!    looks mechanically cloned.
//! 2. PER-BAR ENVELOPE: fast attack, slow release, peak-hold so syllables
//!    bloom and settle; broadband level folds in modestly.
//! 3. SPRING-DAMPER: per-bar underdamped spring toward the target (~10%
//!    overshoot on a step, settles quickly) — no linear-lerp snapping.
//! 4. EDGE LAG: bars near the edges react slightly slower; motion spreads
//!    outward from the center organically.
//! 5. IDLE: when silent, a slow traveling sine wave + gentle per-bar LFO
//!    wobble breathe in over ~0.7 s so a silent-but-recording pill stays alive.
//! 6. SPATIAL ENVELOPE: center full, edges lower.

/// Number of equalizer bars (symmetric: bar i mirrors bar NUM_BARS-1-i).
pub const NUM_BARS: usize = 9;
/// Band centers in Hz that `push`'s `bands` slice corresponds to (documentation).
pub const BAND_CENTERS_HZ: [f32; NUM_BARS] =
    [250.0, 350.0, 500.0, 700.0, 1000.0, 1400.0, 2000.0, 2800.0, 4000.0];

/// Static tuning constants. One struct so tsar (renderer) and tests can see them.
#[derive(Debug, Clone, Copy)]
pub struct WaveParams {
    /// Band-envelope attack coefficient (per push, ~30 Hz). Fast rise so
    /// syllables bloom.
    pub attack: f32,
    /// Band-envelope release coefficient (per push). Slow fall so speech
    /// settles gracefully.
    pub release: f32,
    /// Peak-hold time in pushes (30 Hz → 12 = 0.4 s) before the envelope
    /// may fall back.
    pub peak_hold: u32,
    /// How much broadband RMS lifts the whole silhouette:
    /// level term = level * level_gain * spatial_envelope.
    pub level_gain: f32,
    /// How strongly amplitude falls off from bar center to edge
    /// (envelope = 1 - falloff * distance). 0.65 → edges sit at ~0.35.
    pub spatial_falloff: f32,
    /// Spring stiffness k (per push, semi-implicit Euler). With damping below
    /// this gives ~8% overshoot on a step at 30 Hz (complex discrete poles).
    pub spring_k: f32,
    /// Spring damping c (per push). Underdamped on purpose (c < 2 - 2·√(1-k)).
    pub spring_c: f32,
    /// How much edge bars lag the center (attack/stiffness scaled by
    /// 1 - edge_lag * distance).
    pub edge_lag: f32,
    /// Per-bar deterministic stiffness jitter amplitude — breaks the mirrored
    /// pair's mechanical cloning.
    pub pair_jitter: f32,
    /// Idle traveling-wave amplitude (0..1 bar height).
    pub idle_amp: f32,
    /// Pushes of silence before the idle wave is fully gated in (21 ≈ 0.7 s).
    pub idle_gate_pushes: u32,
    /// Idle blend strength per push once gated.
    pub idle_blend: f32,
    /// Phase shift between adjacent bars in the idle traveling wave (rad).
    pub idle_phase_step: f32,
    /// Idle phase advance per push (rad). 0.21 → ~1 s wave period.
    pub idle_rate: f32,
    /// Per-bar LFO wobble amplitude (± fraction, e.g. 0.12 = ±12%).
    pub wobble_amp: f32,
    /// Per-bar wobble base rate (rad per push); bar i uses rate + 0.05 * i.
    pub wobble_rate: f32,
}

/// All tuning constants in one place, chosen for the production look.
pub fn wave_params() -> WaveParams {
    WaveParams {
        attack: 0.7,
        release: 0.09,
        peak_hold: 12,
        level_gain: 0.3,
        spatial_falloff: 0.65,
        // Discrete poles 0.4±0.37i → ~8% overshoot, settles in ~10 pushes.
        spring_k: 0.5,
        spring_c: 0.7,
        edge_lag: 0.45,
        pair_jitter: 0.25,
        idle_amp: 0.14,
        idle_gate_pushes: 21,
        idle_blend: 0.35,
        idle_phase_step: 1.1,
        idle_rate: 0.21,
        wobble_amp: 0.12,
        wobble_rate: 0.4,
    }
}

/// Simulation state. Fixed-size arrays: plain-old-data, Copy, no allocation.
#[derive(Debug, Clone, Copy)]
pub struct Sim {
    params: WaveParams,
    /// Rendered bar heights, 0.0..1.0.
    bars: [f32; NUM_BARS],
    /// Spring velocity per bar.
    vel: [f32; NUM_BARS],
    /// Smoothed band envelope per MIRRORED band index (0..NUM_BARS/2+1),
    /// shared by each mirrored pair before per-bar jitter.
    env: [f32; NUM_BARS],
    /// Peak-hold value per band index.
    peak: [f32; NUM_BARS],
    /// Remaining pushes before each peak may fall.
    peak_timer: [u32; NUM_BARS],
    /// Per-bar wobble phase, advanced each push by a bar-specific rate.
    wobble_phase: [f32; NUM_BARS],
    /// Global idle-breathing phase (slow traveling wave).
    idle_phase: f32,
    /// Pushes since `speaking` was last true; gates the idle wave in.
    idle_frames: u32,
}

/// Deterministic per-bar jitter in [-1, 1]: sin over the golden-angle spread.
/// No rand crate, identical across runs/tests.
fn bar_jitter(i: usize) -> f32 {
    ((i as f32 + 0.5) * 2.399_963_2).sin() * 0.5 + 0.5
}

impl Sim {
    pub fn new(params: WaveParams) -> Sim {
        Sim {
            params,
            bars: [0.0; NUM_BARS],
            vel: [0.0; NUM_BARS],
            env: [0.0; NUM_BARS],
            peak: [0.0; NUM_BARS],
            peak_timer: [0; NUM_BARS],
            wobble_phase: [
                0.0, 0.9, 1.8, 2.7, 3.6, 4.5, 5.4, 0.45, 1.35,
            ],
            idle_phase: 0.0,
            idle_frames: u32::MAX, // start fully "silent": idle wave already gated
        }
    }

    /// Feed one input frame (~30 Hz). `level` = broadband RMS (0.0..2.0),
    /// `bands[i]` = normalized 0..1 energy of band i, `speaking` = VAD flag.
    /// NaN/inf inputs are sanitized and never poison state.
    pub fn push(&mut self, level: f32, bands: &[f32; NUM_BARS], speaking: bool) {
        let p = self.params;

        // Sanitize inputs: NaN/inf → 0, then clamp to contract ranges.
        let level = if level.is_finite() { level } else { 0.0 };
        let level = level.clamp(0.0, 2.0);
        let mut band = [0.0f32; NUM_BARS];
        for i in 0..NUM_BARS {
            let b = bands[i];
            band[i] = if b.is_finite() { b.clamp(0.0, 1.0) } else { 0.0 };
        }
        // Belt-and-braces: sanitize carried state too (defensive, cheap).
        for i in 0..NUM_BARS {
            if !self.bars[i].is_finite() {
                self.bars[i] = 0.0;
            }
            if !self.vel[i].is_finite() {
                self.vel[i] = 0.0;
            }
            if !self.env[i].is_finite() {
                self.env[i] = 0.0;
            }
            if !self.peak[i].is_finite() {
                self.peak[i] = 0.0;
            }
        }
        if !self.idle_phase.is_finite() {
            self.idle_phase = 0.0;
        }

        if speaking {
            self.idle_frames = 0;
        } else {
            self.idle_frames = self.idle_frames.saturating_add(1);
        }
        // Idle wave ramps in over ~0.7 s of silence.
        let idle_gate = if self.idle_frames == u32::MAX {
            1.0
        } else {
            (self.idle_frames as f32 / p.idle_gate_pushes as f32).clamp(0.0, 1.0)
        };
        self.idle_phase = (self.idle_phase + p.idle_rate) % core::f32::consts::TAU;

        let center = (NUM_BARS - 1) as f32 * 0.5;

        // 1) Band envelopes (per mirrored band index): fast attack, peak-hold
        //    ~0.4 s, then slow release. The peak never pins the envelope —
        //    hold expires and the envelope glides down to the current band.
        let half = NUM_BARS / 2 + 1; // indices 0..=4 cover all 9 bars mirrored
        for j in 0..half {
            let v = band[j.min(NUM_BARS - 1 - j)].max(band[j]);
            let env = &mut self.env[j];
            if v > *env {
                *env += (v - *env) * p.attack;
                self.peak_timer[j] = p.peak_hold;
            } else if self.peak_timer[j] > 0 {
                self.peak_timer[j] -= 1; // hold the bloom
            } else {
                *env += (v - *env) * p.release;
            }
        }

        // 2) Per-bar spring toward target, with edge lag + pair jitter + idle.
        for i in 0..NUM_BARS {
            let dist = ((i as f32 - center) / center).abs().clamp(0.0, 1.0);
            let spatial = 1.0 - p.spatial_falloff * dist;

            let j = core::cmp::min(i, NUM_BARS - 1 - i);
            let band_env = self.env[j];
            // Broadband folds in modestly so loud speech lifts everything.
            let level_term = level * p.level_gain * spatial;
            let mut target = band_env.max(level_term).clamp(0.0, 1.0);

            // Idle breathing: traveling sine + per-bar LFO wobble, gated in on
            // silence (wobble belongs to the idle look, not to speech).
            if idle_gate > 0.0 {
                self.wobble_phase[i] =
                    (self.wobble_phase[i] + p.wobble_rate + 0.05 * i as f32) % core::f32::consts::TAU;
                let wobble = 1.0 + p.wobble_amp * self.wobble_phase[i].sin();
                let wave = self.idle_phase - j as f32 * p.idle_phase_step;
                let body = (p.idle_amp * (0.5 + 0.5 * wave.sin()) * wobble).clamp(0.0, 1.0);
                target += (body - target) * (idle_gate * p.idle_blend);
            }

            // Edge lag: stiffness shrinks toward the edges; pair jitter makes
            // the mirrored pair drift apart organically.
            let jitter = 1.0 + p.pair_jitter * (2.0 * bar_jitter(i) - 1.0);
            let k = (p.spring_k * (1.0 - p.edge_lag * dist) * jitter).max(0.05);
            let c = p.spring_c;

            // Underdamped spring, semi-implicit Euler (stable at k+c < 2).
            self.vel[i] += k * (target - self.bars[i]) - c * self.vel[i];
            self.vel[i] = self.vel[i].clamp(-0.5, 0.5);
            self.bars[i] = (self.bars[i] + self.vel[i]).clamp(0.0, 1.0);
        }
    }

    /// Current symmetric bar heights, 0.0..1.0, len == NUM_BARS.
    pub fn bars(&self) -> &[f32] {
        &self.bars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_hot(mut bands: [f32; NUM_BARS], idx: usize, v: f32) -> [f32; NUM_BARS] {
        bands[idx] = v;
        bands
    }

    #[test]
    fn fresh_state_len_and_range() {
        let sim = Sim::new(wave_params());
        let bars = sim.bars();
        assert_eq!(bars.len(), NUM_BARS);
        for &b in bars {
            assert!((0.0..=1.0).contains(&b), "fresh bar {b} out of range");
        }
    }

    #[test]
    fn one_hot_band_drives_mirror_pair() {
        let mut sim = Sim::new(wave_params());
        let bands = one_hot([0.0; NUM_BARS], 2, 0.9);
        for _ in 0..30 {
            sim.push(0.5, &bands, true);
        }
        let bars = sim.bars();
        // Bars 2 and 8-2=6 ride band 2; bars 0 and 4 ride band 4 (silent).
        assert!(
            bars[2] > bars[0] + 0.2 && bars[2] > bars[4] + 0.2,
            "band-2 pair must lead: {bars:?}"
        );
        assert!(
            bars[6] > bars[0] + 0.2 && bars[6] > bars[4] + 0.2,
            "band-2 pair must lead: {bars:?}"
        );
    }

    #[test]
    fn mirrored_pair_close_after_steady_push() {
        let mut sim = Sim::new(wave_params());
        let mut bands = [0.0f32; NUM_BARS];
        bands[2] = 0.8;
        bands[5] = 0.6;
        for _ in 0..40 {
            sim.push(1.0, &bands, true);
        }
        let bars = sim.bars();
        for i in 0..NUM_BARS / 2 {
            let d = (bars[i] - bars[NUM_BARS - 1 - i]).abs();
            assert!(d < 0.15, "pair {i}/{} too far apart: {bars:?}", NUM_BARS - 1 - i);
        }
    }

    #[test]
    fn step_overshoots_once_then_settles() {
        let mut sim = Sim::new(wave_params());
        let mut bands = [0.0f32; NUM_BARS];
        bands[4] = 0.9; // center bar target ≈ 0.9 (level 0 → pure band env)
        let mut max_seen = 0.0f32;
        for step in 0..40 {
            sim.push(0.0, &bands, true);
            let b = sim.bars()[4];
            if step > 4 {
                max_seen = max_seen.max(b);
            }
        }
        let final_bars = sim.bars()[4];
        // Underdamped spring: pokes above the target once, then settles near it.
        assert!(
            max_seen > 0.92,
            "expected overshoot above target 0.9, max {max_seen}"
        );
        assert!(
            (final_bars - 0.9).abs() < 0.06,
            "must settle near target: {final_bars}"
        );
    }

    #[test]
    fn silence_decays_to_idle_range() {
        let mut sim = Sim::new(wave_params());
        let bands = [0.9f32; NUM_BARS];
        for _ in 0..30 {
            sim.push(1.8, &bands, true);
        }
        let loud: f32 = sim.bars().iter().sum();
        for _ in 0..90 {
            sim.push(0.0, &[0.0; NUM_BARS], false);
        }
        let quiet: f32 = sim.bars().iter().sum();
        assert!(quiet < loud * 0.3, "must decay: loud={loud} quiet={quiet}");
        // Converged idle: small, breathing, well below speech levels.
        for &b in sim.bars() {
            assert!(b < 0.3, "idle bar {b} too tall");
            assert!(b >= 0.0);
        }
    }

    #[test]
    fn ninety_silent_pushes_stay_in_range() {
        let mut sim = Sim::new(wave_params());
        for _ in 0..90 {
            sim.push(0.0, &[0.0; NUM_BARS], false);
        }
        for &b in sim.bars() {
            assert!((0.0..=1.0).contains(&b), "idle bar {b} out of range");
        }
    }

    #[test]
    fn absurd_inputs_no_panic_and_recover() {
        let mut sim = Sim::new(wave_params());
        sim.push(f32::NAN, &[f32::NAN; NUM_BARS], true);
        sim.push(f32::INFINITY, &[f32::NEG_INFINITY; NUM_BARS], true);
        sim.push(-3.0, &[-1.0; NUM_BARS], false);
        sim.push(2.0, &[1.5; NUM_BARS], true);
        for &b in sim.bars() {
            assert!(b.is_finite() && (0.0..=1.0).contains(&b), "bar {b} after absurd input");
        }
        // State must be un-poisoned: normal speech afterwards behaves normally.
        let mut bands = [0.0f32; NUM_BARS];
        bands[4] = 0.9;
        for _ in 0..30 {
            sim.push(1.0, &bands, true);
        }
        assert!(sim.bars()[4] > 0.5, "state did not recover: {:?}", sim.bars());
    }

    #[test]
    fn bars_len_always_num_bars() {
        let mut sim = Sim::new(wave_params());
        assert_eq!(sim.bars().len(), NUM_BARS);
        sim.push(1.0, &[0.5; NUM_BARS], true);
        assert_eq!(sim.bars().len(), NUM_BARS);
    }
}
