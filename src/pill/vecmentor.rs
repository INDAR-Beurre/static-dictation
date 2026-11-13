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
//! 2. PER-BAR ENVELOPE: fast attack (0.8/push — a louder band closes ~80% of
//!    the gap in a single push), a short peak-hold (~0.3 s) so syllables pop,
//!    then a gentle release (~0.33 s time constant) for a graceful settle;
//!    broadband level folds in modestly.
//! 3. SPRING-DAMPER: per-bar underdamped spring toward the target — ~10-13%
//!    overshoot on a step, settled (±1%) within ~10-11 pushes (~350 ms at
//!    30 Hz). The overshoot IS the bounce; no linear-lerp snapping.
//! 4. EDGE LAG: bars near the edges react slightly slower; motion spreads
//!    outward from the center organically.
//! 5. IDLE: when silent, a slow (~1.2 s period) CENTER-EMPHASIZED breathing
//!    wave + gentle per-bar LFO wobble (±10%) breathe in over ~0.7 s. The
//!    traveling wave keeps a raised trough (never fully dark), so the resting
//!    state reads as a neat centered row of small dots — the Siri shimmer —
//!    not random noise.
//! 6. SPATIAL ENVELOPE: the whole per-bar SPEECH target (band bloom and
//!    broadband level alike) is scaled by 1 - 0.72·distance from center, then
//!    by a static 9-bar SHAPE MASK measured from the reference silhouette —
//!    not a plain pyramid but the asymmetric-looking island read: low
//!    outer dots, a dip, and a tall 5-bar cluster dominating the center.
//!    (Symmetry is preserved: bar i's mask value mirrors bar 8-i's; the
//!    reference's left-right asymmetry is a lighting trick, not physics.)
//! 7. IDLE UNAFFECTED: the mask gates speech energy only. The resting state
//!    stays the round-1 traveling-wave dot row (IDLE_TROUGH behavior).

/// Number of equalizer bars (symmetric: bar i mirrors bar NUM_BARS-1-i).
pub const NUM_BARS: usize = 9;
/// Band centers in Hz that `push`'s `bands` slice corresponds to (documentation).
pub const BAND_CENTERS_HZ: [f32; NUM_BARS] = [
    250.0, 350.0, 500.0, 700.0, 1000.0, 1400.0, 2000.0, 2800.0, 4000.0,
];

/// Static tuning constants. One struct so tsar (renderer) and tests can see them.
#[derive(Debug, Clone, Copy)]
pub struct WaveParams {
    /// Band-envelope attack coefficient (per push, ~30 Hz). 0.8 → syllables
    /// bloom almost instantly (gap to a louder band closes 80% per push).
    pub attack: f32,
    /// Band-envelope release coefficient (per push). 0.08 ≈ 0.33 s time
    /// constant: slow fall so speech settles gracefully.
    pub release: f32,
    /// Peak-hold time in pushes (30 Hz → 9 = 0.3 s) before the envelope may
    /// fall back. Short hold so each syllable pops.
    pub peak_hold: u32,
    /// How much broadband RMS lifts the whole silhouette:
    /// level term = level * level_gain (the shared spatial envelope is
    /// applied to the combined target afterwards — see `spatial_falloff`).
    pub level_gain: f32,
    /// How strongly amplitude falls off from bar center to edge. The WHOLE
    /// speech target (band envelope and level term) is scaled by
    /// `1 - falloff * distance`; 0.72 → edges sit at ~28% of center. The
    /// static `SHAPE_MASK` then bends that pyramid into the island
    /// silhouette (low dots, dip, dominant center cluster).
    pub spatial_falloff: f32,
    /// Spring stiffness k (per push, semi-implicit Euler). Tuned with the
    /// damping below for ~10% overshoot on a step at 30 Hz (complex discrete
    /// poles; see the math comment in `wave_params`).
    pub spring_k: f32,
    /// Spring damping c (per push). Underdamped on purpose: discrete-map
    /// underdamping needs c < 2·√k_eff, and stability needs k_eff < c.
    pub spring_c: f32,
    /// How much edge bars lag the center (stiffness scaled by
    /// 1 - edge_lag * distance).
    pub edge_lag: f32,
    /// Per-bar deterministic stiffness jitter amplitude — breaks the mirrored
    /// pair's mechanical cloning.
    pub pair_jitter: f32,
    /// Idle traveling-wave amplitude (0..1 bar height). 0.18 → idle crests
    /// reach ~0.18 at the center; troughs stay at ~0.65 of that: a small,
    /// always-visible centered dot row.
    pub idle_amp: f32,
    /// Pushes of silence before the idle wave is fully gated in (21 ≈ 0.7 s).
    pub idle_gate_pushes: u32,
    /// Idle blend strength per push once gated. 0.9 — the target snaps to the
    /// idle body almost immediately; the spring supplies all the smoothing.
    pub idle_blend: f32,
    /// Phase shift between adjacent bars in the idle traveling wave (rad).
    pub idle_phase_step: f32,
    /// Idle phase advance per push (rad). 0.175 → wave period 2π/0.175 ≈ 36
    /// pushes ≈ 1.2 s: slow, subtle breathing.
    pub idle_rate: f32,
    /// Per-bar LFO wobble amplitude (± fraction, e.g. 0.10 = ±10%).
    pub wobble_amp: f32,
    /// Per-bar wobble base rate (rad per push); bar i uses rate + 0.05 * i.
    pub wobble_rate: f32,
}

/// All tuning constants in one place, chosen for the production look.
pub fn wave_params() -> WaveParams {
    WaveParams {
        attack: 0.8,
        release: 0.08,
        peak_hold: 9,
        level_gain: 0.3,
        spatial_falloff: 0.72,
        // Underdamped spring, tuned at the center bar where the effective
        // stiffness is k_eff = spring_k * jitter(4) = 0.445 * 0.755 ≈ 0.336
        // with c = 0.56. Semi-implicit Euler characteristic polynomial:
        //   z² - (2-c)·z + (1-c+k_eff) = 0
        //   |z| = r = √(1-c+k_eff) ≈ 0.881,  θ = acos((2-c)/(2r)) ≈ 0.615 rad
        // → half-period π/θ ≈ 5 pushes, per-period decay r² ≈ 0.78
        // ⇒ ~10% overshoot on a step, settled (±1%) within ~11 pushes
        //   (~370 ms at 30 Hz). Stability needs k_eff < c; the worst bar
        //   (jitter 1.25, dist 0.25) lands at k_eff ≈ 0.50 < 0.9·c = 0.504,
        //   so wobble stays a decay, never a growl.
        spring_k: 0.445,
        spring_c: 0.56,
        edge_lag: 0.3,
        pair_jitter: 0.25,
        idle_amp: 0.18,
        idle_gate_pushes: 21,
        idle_blend: 0.9,
        idle_phase_step: 1.1,
        idle_rate: 0.175,
        wobble_amp: 0.1,
        wobble_rate: 0.4,
    }
}

/// Idle traveling-wave trough floor: the breathing wave never dips below this
/// fraction of its crest, so the resting pill reads as a neat centered row of
/// dots (the canvas layer renders small dim dots at low heights) instead of
/// going fully dark between breaths.
const IDLE_TROUGH: f32 = 0.65;

/// Static silhouette mask, bars 0..8, exactly mirrored (mask[i] == mask[8-i]).
/// Interpolated from the measured reference envelope (relative to tallest):
/// left ramp 5 bars [0.06,0.10,0.13,0.14,0.15], dip zone [0.08-0.21], tall
/// 5-bar cluster [0.60,0.83,1.00,0.84,0.62], mirrored right fall into dots.
/// 9-bar mapping: the cluster occupies bars 2-6 (its inner pair averaged to a
/// single mirrored value: 0.83/0.84 → 0.84, 0.60/0.62 → 0.61 — the reference's
/// left/right lopsidedness is a lighting trick, not physics); the ramp and dip
/// zone compress into the outer pair (bar 1/7 sits at the ramp top, inside the
/// dip range, so the valley into the cluster reads as the reference's dip).
/// Under speech the spring's outward lag and pair jitter keep the frozen
/// vector alive. Multiplies the spatial falloff; band energy modulates on top
/// (mask × physics, not instead of physics). NOTE: silhouette RATIOS, not
/// absolute heights — bars still scale with `spatial_falloff`.
pub const SHAPE_MASK: [f32; NUM_BARS] =
    [0.06, 0.15, 0.61, 0.84, 1.00, 0.84, 0.61, 0.15, 0.06];

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
    /// Decaying speech activity (1 on any voiced push, ~decays in silence);
    /// gates the shape mask in/out so idle keeps the round-1 dot row.
    speech_gate: f32,
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
            wobble_phase: [0.0, 0.9, 1.8, 2.7, 3.6, 4.5, 5.4, 0.45, 1.35],
            idle_phase: 0.0,
            idle_frames: u32::MAX, // start fully "silent": idle wave already gated
            speech_gate: 0.0,
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
            band[i] = if b.is_finite() {
                b.clamp(0.0, 1.0)
            } else {
                0.0
            };
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
        if !self.speech_gate.is_finite() {
            self.speech_gate = 0.0;
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
        // Speech gate: the shape mask applies to voiced energy only. Rises
        // instantly on a voiced push, fades in silence so the idle handoff
        // (dot-row look) is never fighting the mask.
        let speech_gate = if speaking {
            1.0
        } else {
            (self.speech_gate * 0.9).max(0.0)
        };
        self.speech_gate = speech_gate;

        let center = (NUM_BARS - 1) as f32 * 0.5;

        // 1) Band envelopes (per mirrored band index): fast attack, peak-hold
        //    ~0.3 s, then slow release. The peak never pins the envelope —
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
        // (Inherently index-based: mutates vel/bars/wobble_phase per bar.)
        #[allow(clippy::needless_range_loop)]
        for i in 0..NUM_BARS {
            let dist = ((i as f32 - center) / center).abs().clamp(0.0, 1.0);
            let spatial = 1.0 - p.spatial_falloff * dist;

            let j = core::cmp::min(i, NUM_BARS - 1 - i);
            let band_env = self.env[j];
            // Broadband folds in modestly so loud speech lifts everything.
            let level_term = level * p.level_gain;
            // The spatial envelope scales the WHOLE speech target (band
            // bloom and broadband alike), so even flat input renders the
            // island silhouette; the static shape mask (mask × physics) bends
            // it from a plain pyramid into the measured read. Gate it on
            // speech so the idle state keeps the round-1 dot row.
            let mask = 1.0 - speech_gate * (1.0 - SHAPE_MASK[i]);
            let mut target = band_env.max(level_term).clamp(0.0, 1.0) * spatial * mask;

            // Idle breathing: traveling sine + per-bar LFO wobble, gated in on
            // silence (wobble belongs to the idle look, not to speech). The
            // body is center-weighted via `spatial` and keeps a raised trough
            // (IDLE_TROUGH) so the resting silhouette is a dot row, not gaps.
            if idle_gate > 0.0 {
                self.wobble_phase[i] = (self.wobble_phase[i] + p.wobble_rate + 0.05 * i as f32)
                    % core::f32::consts::TAU;
                let wobble = 1.0 + p.wobble_amp * self.wobble_phase[i].sin();
                let wave = self.idle_phase - j as f32 * p.idle_phase_step;
                let body = (p.idle_amp
                    * (IDLE_TROUGH + (1.0 - IDLE_TROUGH) * wave.sin())
                    * wobble
                    * spatial)
                    .clamp(0.0, 1.0);
                target += (body - target) * (idle_gate * p.idle_blend);
            }

            // Edge lag: stiffness shrinks toward the edges; pair jitter makes
            // the mirrored pair drift apart organically.
            let jitter = 1.0 + p.pair_jitter * (2.0 * bar_jitter(i) - 1.0);
            let k = (p.spring_k * (1.0 - p.edge_lag * dist) * jitter).max(0.05);
            let c = p.spring_c;

            // Underdamped spring, semi-implicit Euler (stable while k < c;
            // pole math documented at `wave_params`).
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
            assert!(
                d < 0.15,
                "pair {i}/{} too far apart: {bars:?}",
                NUM_BARS - 1 - i
            );
        }
    }

    #[test]
    fn step_overshoots_once_then_settles() {
        // Full-loop step response (envelope attack + spring) at the center
        // bar, across target sizes: the bounce must land in the 10..14%
        // window and settle within ~12 pushes (≈0.4 s at 30 Hz).
        for target in [0.5f32, 0.7, 0.9] {
            let mut sim = Sim::new(wave_params());
            let mut bands = [0.0f32; NUM_BARS];
            bands[4] = target; // center bar target = band value (level 0)
            let mut peak = 0.0f32;
            let mut tail = [0.0f32; 6];
            for step in 0..40 {
                sim.push(0.0, &bands, true);
                let b = sim.bars()[4];
                peak = peak.max(b);
                tail[step % 6] = b;
            }
            let final_b = tail[39 % 6];
            let overshoot = peak / final_b - 1.0;
            assert!(
                (0.10..=0.14).contains(&overshoot),
                "target {target}: overshoot {overshoot:.3} outside 10-14%"
            );
            // Settled: the last 6 pushes all sit within 1% of the target.
            for &b in &tail {
                assert!(
                    (b - target).abs() < 0.01,
                    "target {target}: not settled, tail {tail:?}"
                );
            }
        }
    }

    #[test]
    fn flat_input_renders_island_silhouette() {
        let mut sim = Sim::new(wave_params());
        let bands = [0.5f32; NUM_BARS];
        for _ in 0..45 {
            sim.push(0.9, &bands, true);
        }
        let bars = sim.bars();
        let p = wave_params();
        // Steady state: bar i sits at band * spatial * mask (the speech gate
        // is 1 after the first voiced push). Assert the full 9-value
        // silhouette against SHAPE_MASK at ±0.02 so drift gets caught.
        for (i, &m) in SHAPE_MASK.iter().enumerate() {
            let dist = ((i as f32 - 4.0) / 4.0).abs();
            let spatial = 1.0 - p.spatial_falloff * dist;
            let expected = 0.5 * spatial * m;
            assert!(
                (bars[i] - expected).abs() < 0.02,
                "bar {i}: {} vs expected {expected:.4}: {bars:?}",
                bars[i]
            );
        }
        // Cluster dominant, edge dots low, mirrored symmetry.
        assert!(
            bars[4] > bars[2] * 2.0,
            "center cluster must dominate: {bars:?}"
        );
        assert!(
            bars[0] < 0.05 && bars[8] < 0.05,
            "edge dots must stay low: {bars:?}"
        );
        for i in 0..NUM_BARS / 2 {
            let d = (bars[i] - bars[NUM_BARS - 1 - i]).abs();
            assert!(d < 0.03, "pair {i} asymmetry {d}: {bars:?}");
        }
    }

    #[test]
    fn mask_ordering_ramp_dip_cluster() {
        let mut sim = Sim::new(wave_params());
        let bands = [0.5f32; NUM_BARS];
        for _ in 0..45 {
            sim.push(0.9, &bands, true);
        }
        let bars = sim.bars();
        // Ramp rises inward from the outermost dot...
        assert!(bars[0] < bars[1], "ramp must rise inward: {bars:?}");
        // ...into the dip: bars 1/7 sit in the reference's [0.08-0.21] dip
        // zone, a deep valley below the cluster's first bar (the 9-bar
        // compression of ramp + dip keeps the dip as the cliff into the
        // cluster; the outermost dot stays smallest, per the reference).
        assert!(
            bars[1] < bars[2] * 0.5 && bars[7] < bars[6] * 0.5,
            "dip must sit well below the cluster edge: {bars:?}"
        );
        // Cluster peaks at center; the right side falls back into dots.
        for &b in bars {
            assert!(b <= bars[4] + 1e-4, "center must peak: {bars:?}");
        }
        assert!(
            bars[6] < bars[5] && bars[7] < bars[6] && bars[8] < bars[7],
            "right side must fall into dots: {bars:?}"
        );
    }

    #[test]
    fn pair_jitter_varies_mask_shape_mid_transient() {
        // The mask sets the steady-state silhouette; pair_jitter must keep
        // the mirrored pair from filling it in in lockstep (living waveform,
        // not a frozen vector). Deterministic jitter → deterministic gap.
        let mut sim = Sim::new(wave_params());
        let bands = [0.5f32; NUM_BARS];
        sim.push(0.9, &bands, true);
        sim.push(0.9, &bands, true);
        let bars = sim.bars();
        let d = (bars[2] - bars[6]).abs();
        assert!(
            d > 0.002,
            "mirrored pair must diverge mid-transient (d={d}): {bars:?}"
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
        let bars = sim.bars();
        // Converged idle: small breathing dots, well below speech levels.
        for &b in bars {
            assert!(b < 0.25, "idle bar {b} too tall");
            assert!(b >= 0.0);
        }
        assert!(
            bars[4] > bars[0] && bars[4] > bars[8],
            "idle silhouette must stay center-weighted: {bars:?}"
        );
    }

    #[test]
    fn idle_center_dot_row_breathes() {
        let mut sim = Sim::new(wave_params());
        // Fresh state starts pre-gated; watch one full breathing cycle window.
        let mut min_center = 1.0f32;
        let mut max_center = 0.0f32;
        let mut sum_center = 0.0f32;
        let mut center_dominant = true;
        for _ in 0..120 {
            sim.push(0.0, &[0.0; NUM_BARS], false);
            let bars = sim.bars();
            let c = bars[4];
            min_center = min_center.min(c);
            max_center = max_center.max(c);
            sum_center += c;
            center_dominant &= c >= bars[0] && c >= bars[8];
        }
        let mean = sum_center / 120.0;
        // Small centered dot row: never dark, never tall, Siri-shimmer sized.
        assert!(
            min_center > 0.03 && max_center < 0.2,
            "center dot out of dot range: min {min_center:.3} max {max_center:.3}"
        );
        assert!(
            (0.07..=0.18).contains(&mean),
            "mean idle center {mean:.3} outside dot band"
        );
        assert!(
            max_center - min_center > 0.04,
            "idle must breathe over time: min {min_center:.3} max {max_center:.3}"
        );
        assert!(center_dominant, "idle must stay center-weighted");
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
            assert!(
                b.is_finite() && (0.0..=1.0).contains(&b),
                "bar {b} after absurd input"
            );
        }
        // State must be un-poisoned: normal speech afterwards behaves normally.
        let mut bands = [0.0f32; NUM_BARS];
        bands[4] = 0.9;
        for _ in 0..30 {
            sim.push(1.0, &bands, true);
        }
        assert!(
            sim.bars()[4] > 0.5,
            "state did not recover: {:?}",
            sim.bars()
        );
    }

    #[test]
    fn bars_len_always_num_bars() {
        let mut sim = Sim::new(wave_params());
        assert_eq!(sim.bars().len(), NUM_BARS);
        sim.push(1.0, &[0.5; NUM_BARS], true);
        assert_eq!(sim.bars().len(), NUM_BARS);
    }
}
