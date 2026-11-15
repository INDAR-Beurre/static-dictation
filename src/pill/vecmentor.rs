//! vecmentor: waveform animation physics for the pill.
//!
//! Production rebuild: the pill renders a SYMMETRIC 27-band equalizer driven by
//! real spectral energies (`crate::bands::energies`, computed in main.rs per
//! 30 ms frame). This module owns all motion physics; it is std/core-only,
//! zero crate imports, no I/O, plain-old-data — trivially unit-testable.
//!
//! Contract (FROZEN — other agents code against this):
//! - `NUM_BARS == 27`, band centers log-spaced over 250..4000 Hz with ratio
//!   16^(1/26) per step (bar 13 lands exactly on the 1000 Hz voice core;
//!   mirrored bars i / NUM_BARS-1-i sit at log-symmetric centers).
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
//! 5. IDLE: when silent, the body poses the reference silhouette scaled to
//!    ≈0.40 of the cap height (idle_pose 0.45 × idle_blend 0.9) — SHAPE_MASK
//!    ratios intact (dim outer dots, ramp, dip, tall center cluster) —
//!    swelling ±9% in unison on a slow cycle (idle_rate 0.075 → ≈2.8 s;
//!    criterion ≥ 1.5 s), with a gentle per-bar LFO wobble (±5%) on the
//!    residual wave — ramping in over ~0.7 s. The residual traveling wave
//!    (trough floor 0.30 of crest) animates the low-mask zones (dip, outer
//!    dots); the pose dominates the cluster so its shape never wobbles.
//! 6. SPATIAL ENVELOPE: the whole per-bar SPEECH target (band bloom and
//!    broadband level alike) is scaled by 1 - 0.72·distance from center, then
//!    by a static 27-bar SHAPE MASK measured from the reference silhouette —
//!    not a plain pyramid but the asymmetric-looking island read: low
//!    outer dots, a ramp, a dip, and a tall 5-bar cluster (bars 11-15)
//!    dominating the center.
//!    (Symmetry is preserved: bar i's mask value mirrors bar NUM_BARS-1-i's; the
//!    reference's left-right asymmetry is a lighting trick, not physics.)
//! 7. SPEECH-ONLY MASK: the shape mask poses the idle body directly and
//!    gates speech energy only.

/// Number of equalizer bars (symmetric: bar i mirrors bar NUM_BARS-1-i).
pub const NUM_BARS: usize = 27;
/// Band centers in Hz that `push`'s `bands` slice corresponds to (documentation).
/// 27 log-spaced centers over the same 250..4000 Hz range the 9-band pipeline
/// covered: f(i) = 250 · 16^(i/26), ratio 16^(1/26) ≈ 1.1125 per step — the
/// voice core (bar 13) lands exactly on 1000 Hz and mirrored bars i / 26-i
/// sit at log-symmetric centers (f(i)·f(26-i) = 250·4000).
pub const BAND_CENTERS_HZ: [f32; NUM_BARS] = [
    250.0, 278.0, 309.0, 344.0, 383.0, 426.0, 474.0, 527.0, 587.0, 653.0, //
    726.0, 808.0, 899.0, 1000.0, 1113.0, 1238.0, 1377.0, 1532.0, 1704.0, //
    1896.0, 2110.0, 2347.0, 2611.0, 2905.0, 3232.0, 3595.0, 4000.0,
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
    /// Residual idle traveling-wave amplitude (0..1 bar height). 0.25 with
    /// the half-wave breath (0..1) → the dip/outer-dot zones breathe
    /// ≈0..0.16 above the posed floor while the posed cluster
    /// (idle_pose·SHAPE_MASK·swell ≥ 0.39 at the center) always stays on
    /// top of the wave, so the wave animates the low-mask zones without
    /// ever reshaping the cluster.
    pub idle_amp: f32,
    /// Pushes of silence before the idle wave is fully gated in (21 ≈ 0.7 s).
    pub idle_gate_pushes: u32,
    /// Idle blend strength per push once gated. 0.9 — the target snaps to the
    /// idle body almost immediately; the spring supplies all the smoothing.
    pub idle_blend: f32,
    /// Phase shift between adjacent bars in the idle traveling wave (rad).
    /// 0.4 → one readable ripple crosses all 27 bars (27·0.4 ≈ 10.8 rad ≈
    /// 1.7 wavelengths) instead of the old 1.1 (≈ 4.7 wavelengths of
    /// incoherent shimmer).
    pub idle_phase_step: f32,
    /// Idle phase advance per push (rad). 0.075 → period 2π/0.075 ≈ 84
    /// pushes ≈ 2.8 s (round-3 spec: ≥ 1.5 s; was 0.175 ≈ 1.2 s).
    pub idle_rate: f32,
    /// Per-bar LFO wobble amplitude (± fraction, e.g. 0.05 = ±5%). Halved
    /// from 0.10 in round 3: one rim column popped 0→26px for a single frame
    /// (a 6-LSB AA-threshold crossing); ±5% on the ~0.1 rim wave keeps every
    /// step under the 2px @2x AA threshold.
    pub wobble_amp: f32,
    /// Unison breathing swell on the posed silhouette (± fraction of the
    /// posed height, e.g. 0.09 = ±9%): the whole island swells together on
    /// the idle phase so the cluster's SHAPE never wobbles while it breathes
    /// the spec's ±8–10% of posed height.
    pub idle_breathe: f32,
    /// Per-bar wobble base rate (rad per push); bar i uses rate + 0.05 * i.
    /// 0.1 → bar 26 wobbles at (0.1+1.3)/2π ≈ 0.22 Hz; base bar at ~1.6 s
    /// period. Calm breathing, not the old 0.4's 2–4 Hz flicker.
    pub wobble_rate: f32,
    /// Idle pose scale: the silent body holds SHAPE_MASK at this fraction of
    /// the cap height. 0.45 (round 3; was a 0.8 pose-blend weight at full
    /// height) — with the 0.9 idle_blend the effective pose lands at ≈0.40
    /// of cap (silence hums at 40% instead of shouting full-height); the
    /// mask ratios (shape) are untouched.
    pub idle_pose: f32,
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
        // stiffness is k_eff = spring_k * jitter(13) = 0.28 * 1.208 ≈ 0.338
        // with c = 0.56. Semi-implicit Euler characteristic polynomial:
        //   z² - (2-c)·z + (1-c+k_eff) = 0
        //   |z| = r = √(1-c+k_eff) ≈ 0.882,  θ = acos((2-c)/(2r)) ≈ 0.615 rad
        // → half-period π/θ ≈ 5 pushes, per-period decay r² ≈ 0.78
        // ⇒ ~10% overshoot on a step, settled (±1%) within ~11 pushes
        //   (~370 ms at 30 Hz). Stability needs k_eff < c; the worst bar
        //   (i=16: jitter 1.237, dist 0.23) lands at k_eff ≈ 0.32 < 0.9·c = 0.504,
        //   so wobble stays a decay, never a growl. (spring_k dropped from
        //   the 9-bar 0.445: at 27 bars the center index is 13, whose golden-
        //   angle jitter is 1.208, so k must shrink to keep k_eff ≈ 0.34.)
        spring_k: 0.28,
        spring_c: 0.56,
        edge_lag: 0.3,
        pair_jitter: 0.25,
        idle_amp: 0.25,
        idle_gate_pushes: 21,
        idle_blend: 0.9,
        idle_phase_step: 0.4,
        idle_rate: 0.075,
        wobble_amp: 0.05,
        idle_breathe: 0.09,
        wobble_rate: 0.1,
        idle_pose: 0.45,
    }
}

/// Static silhouette mask, bars 0..26, exactly mirrored (mask[i] == mask[26-i]).
/// Interpolated from the measured reference envelope (relative to tallest):
/// left ramp 5 bars [0.06,0.10,0.13,0.14,0.15], dip zone [0.08-0.21] (deepest
/// at bar 6, climbing back to 0.21 right before the cliff), tall 5-bar cluster
/// at the exact center [0.61,0.84,1.00,0.84,0.61] (bars 11-15; the reference's
/// 0.60/0.62 and 0.83/0.84 lopsidedness is a lighting trick, not physics, so
/// the cluster's edge pairs are averaged to mirrored values), then the mirrored
/// right fall through the dip back out to the outer dots. Under speech the
/// spring's outward lag and pair jitter keep the frozen vector alive.
/// Multiplies the spatial falloff; band energy modulates on top (mask ×
/// physics, not instead of physics). NOTE: silhouette RATIOS, not absolute
/// heights — bars still scale with `spatial_falloff`.
pub const SHAPE_MASK: [f32; NUM_BARS] = [
    0.06, 0.10, 0.13, 0.14, 0.15, 0.11, 0.08, 0.10, 0.14, 0.18, 0.21, 0.61, //
    0.84, 1.00, 0.84, 0.61, 0.21, 0.18, 0.14, 0.10, 0.08, 0.11, 0.15, 0.14, //
    0.13, 0.10, 0.06,
];

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
            wobble_phase: {
                let mut wp = [0.0; NUM_BARS];
                for (i, w) in wp.iter_mut().enumerate() {
                    *w = (i as f32 * 0.9) % core::f32::consts::TAU;
                }
                wp
            },
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
        // Unison breathing swell on the posed silhouette (±idle_breathe) —
        // phase-shared with canvas's idle halo (same idle_phase signal).
        let swell = 1.0 + p.idle_breathe * self.idle_phase.sin();
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
        let half = NUM_BARS / 2 + 1; // indices 0..=13 cover all 27 bars mirrored
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

            // Idle breathing: the body poses the reference silhouette scaled
            // to ≈0.40 of the cap (shape ratios intact), swelling ±9%
            // IN UNISON with the idle phase (the island breathes together —
            // the cluster's shape never wobbles), while the residual
            // traveling wave (center-weighted via `spatial`, half-wave
            // breath, ±wobble twinkle) floors and animates the low-mask
            // zones (dip, outer dots). The pose dominates the cluster, the
            // wave the dip, gated in on silence.
            if idle_gate > 0.0 {
                self.wobble_phase[i] = (self.wobble_phase[i] + p.wobble_rate + 0.05 * i as f32)
                    % core::f32::consts::TAU;
                let wobble = 1.0 + p.wobble_amp * self.wobble_phase[i].sin();
                let wave = self.idle_phase - j as f32 * p.idle_phase_step;
                // Half-wave breath 0..1: the wave's trough falls back to the
                // posed floor (low-mask zones go quiet between breaths), the
                // crest is the full residual wave.
                let breath = 0.5 * (1.0 + wave.sin());
                let wave_body = p.idle_amp * breath * wobble * spatial;
                let pose_body = p.idle_pose * SHAPE_MASK[i] * swell;
                let body = pose_body.max(wave_body).clamp(0.0, 1.0);
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
        // Band 12: cluster edge — loud enough that its mask-scaled target
        // clearly clears both the silent outer dot and the silent center.
        let bands = one_hot([0.0; NUM_BARS], 12, 0.9);
        for _ in 0..30 {
            sim.push(0.5, &bands, true);
        }
        let bars = sim.bars();
        // Bars 12 and 26-12=14 ride band 12; bars 0 and 13 ride other (silent)
        // bands.
        assert!(
            bars[12] > bars[0] + 0.2 && bars[12] > bars[13] + 0.2,
            "band-12 pair must lead: {bars:?}"
        );
        assert!(
            bars[14] > bars[0] + 0.2 && bars[14] > bars[13] + 0.2,
            "band-12 pair must lead: {bars:?}"
        );
    }

    #[test]
    fn mirrored_pair_close_after_steady_push() {
        let mut sim = Sim::new(wave_params());
        let mut bands = [0.0f32; NUM_BARS];
        bands[5] = 0.8;
        bands[12] = 0.6;
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
            bands[13] = target; // center bar target = band value (level 0)
            let mut peak = 0.0f32;
            let mut tail = [0.0f32; 6];
            for step in 0..40 {
                sim.push(0.0, &bands, true);
                let b = sim.bars()[13];
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
        // is 1 after the first voiced push). Assert the full 27-value
        // silhouette against SHAPE_MASK at ±0.02 so drift gets caught.
        for (i, &m) in SHAPE_MASK.iter().enumerate() {
            let dist = ((i as f32 - 13.0) / 13.0).abs();
            let spatial = 1.0 - p.spatial_falloff * dist;
            let expected = 0.5 * spatial * m;
            assert!(
                (bars[i] - expected).abs() < 0.02,
                "bar {i}: {} vs expected {expected:.4}: {bars:?}",
                bars[i]
            );
        }
        // Cluster dominant, edge dots low, mirrored symmetry. The center peak
        // (mask 1.0) must rise well above the pre-cluster dip bar 10 (mask
        // 0.21, gentler 27-bar spatial falloff keeps its neighbor closer).
        assert!(
            bars[13] > bars[10] * 2.0,
            "center cluster must dominate: {bars:?}"
        );
        assert!(
            bars[0] < 0.05 && bars[26] < 0.05,
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
        // Ramp rises inward from the outermost dot to its crest at bar 4...
        assert!(
            bars[0] < bars[1] && bars[1] < bars[3] && bars[3] < bars[4],
            "ramp must rise inward: {bars:?}"
        );
        // ...then falls into the dip (deepest at bar 6, inside the reference's
        // [0.08-0.21] dip zone), climbs back toward the cluster...
        assert!(
            bars[4] > bars[5] && bars[5] > bars[6],
            "dip must follow the ramp crest: {bars:?}"
        );
        assert!(
            bars[6] < bars[7] && bars[7] < bars[10],
            "dip must climb back: {bars:?}"
        );
        // ...and the dip sits well below the cluster's first bar (the cliff
        // into the tall 5-bar center read).
        assert!(
            bars[6] < bars[11] * 0.5 && bars[20] < bars[15] * 0.5,
            "dip must sit well below the cluster edge: {bars:?}"
        );
        // Cluster peaks at the exact center; the right side falls back through
        // the dip and the mirrored ramp into the outer dots.
        for &b in bars {
            assert!(b <= bars[13] + 1e-4, "center must peak: {bars:?}");
        }
        assert!(
            bars[15] > bars[16] && bars[16] > bars[17] && bars[17] > bars[18],
            "right side must fall off the cluster: {bars:?}"
        );
        assert!(
            bars[26] < bars[25] && bars[25] < bars[24],
            "outermost right dots must fall away: {bars:?}"
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
        let d = (bars[2] - bars[24]).abs();
        assert!(
            d > 0.002,
            "mirrored pair must diverge mid-transient (d={d}): {bars:?}"
        );
    }

    #[test]
    fn silence_converges_to_idle_pose() {
        let mut sim = Sim::new(wave_params());
        let bands = [0.9f32; NUM_BARS];
        for _ in 0..30 {
            sim.push(1.8, &bands, true);
        }
        for _ in 0..90 {
            sim.push(0.0, &[0.0; NUM_BARS], false);
        }
        let bars = sim.bars();
        // Converged idle (round 3): the posed silhouette at idle_pose 0.45 ×
        // idle_blend 0.9 ≈ 0.40 of cap (was: full-height pose, center ≈0.74).
        // SHAPE ratios intact: tall center cluster, dim outer dots.
        for &b in bars {
            assert!(b <= 1.0);
        }
        assert!(
            (0.30..=0.50).contains(&bars[13]),
            "idle center cluster must sit at ~40% of cap (spec 35-45%): {}",
            bars[13]
        );
        assert!(
            bars[0] < 0.10 && bars[26] < 0.10,
            "idle outer dots must stay dim: {bars:?}"
        );
        // Pose ordering: the dip must sit well below the cluster.
        assert!(
            bars[6] < bars[13] * 0.5,
            "idle dip must sit below the cluster: {bars:?}"
        );
        // Settled: the pose is stable, not chasing noise. Over 15 pushes
        // (0.5 s) the center moves only with the slow breath (a quarter
        // cycle of the 2.8 s swell ≈ ±2%), not per-frame.
        let first = bars[13];
        let mut max_drift = 0.0f32;
        for _ in 0..15 {
            sim.push(0.0, &[0.0; NUM_BARS], false);
            max_drift = max_drift.max((sim.bars()[13] - first).abs());
        }
        assert!(
            max_drift < 0.1,
            "idle pose must be calm, drift {max_drift:.3}"
        );
    }

    #[test]
    fn idle_posed_center_breathes() {
        let mut sim = Sim::new(wave_params());
        // Warm up past the spring run-in and idle gate: bars start at 0.0, so
        // the first pushes read artificially dark before the row converges to
        // its posed breathing steady state (which is what this test judges).
        for _ in 0..30 {
            sim.push(0.0, &[0.0; NUM_BARS], false);
        }
        // Watch 300 pushes (10 s ≈ 3.5 cycles of the 2.8 s breath).
        let mut min_center = 1.0f32;
        let mut max_center = 0.0f32;
        let mut sum_center = 0.0f32;
        let mut series = Vec::with_capacity(300);
        for _ in 0..300 {
            sim.push(0.0, &[0.0; NUM_BARS], false);
            let c = sim.bars()[13];
            series.push(c);
            min_center = min_center.min(c);
            max_center = max_center.max(c);
            sum_center += c;
        }
        let mean = sum_center / 300.0;
        // Posed center at ~40% of cap (was full-height ≈0.74; round-3 spec
        // 35-45%), breathing the spec's ±8-10% of posed height — never dark.
        assert!(
            min_center > 0.30 && max_center <= 0.50,
            "posed center out of range: min {min_center:.3} max {max_center:.3}"
        );
        assert!(
            (0.32..=0.48).contains(&mean),
            "mean idle center {mean:.3} outside posed cluster band"
        );
        let swing = max_center - min_center;
        assert!(
            (0.04..0.10).contains(&swing),
            "center breath swing {swing:.3} must be the spec's ±8-10% of the ~0.40 pose"
        );
        // Period: mean spacing between upward mean-crossings of the center
        // series. Was 1.2 s (idle_rate 0.175); round 3 wants ≈2.8 s ≥ 1.5 s.
        let mut crossings: Vec<usize> = Vec::new();
        for w in series.windows(2) {
            let below = w[0] < mean;
            let now_below = w[1] < mean;
            if below && !now_below {
                crossings.push(series.iter().position(|&c| c == w[1]).unwrap_or(0));
            }
        }
        assert!(
            crossings.len() >= 2,
            "no breath cycles found in 300 pushes: crossings {crossings:?}"
        );
        let mut gaps = Vec::with_capacity(crossings.len() - 1);
        for pair in crossings.windows(2) {
            gaps.push((pair[1] - pair[0]) as f32);
        }
        gaps.sort_unstable_by(|a, b| a.total_cmp(b));
        let median_gap = gaps[gaps.len() / 2];
        let period_s = median_gap / 30.0;
        assert!(
            (1.5..=4.5).contains(&period_s),
            "breath period {period_s:.2}s (median gap {median_gap:.0} pushes) outside 1.5-4.5s"
        );
        // Slow calm breathing in the dip zone: bar 6 (mask 0.08) rides the
        // residual wave; it must swing visibly but stay well below the
        // cluster (posed floor ≈0.03, wave crest ≈0.15).
        let mut min_dip = 1.0f32;
        let mut max_dip = 0.0f32;
        for _ in 0..300 {
            sim.push(0.0, &[0.0; NUM_BARS], false);
            let d = sim.bars()[6];
            min_dip = min_dip.min(d);
            max_dip = max_dip.max(d);
        }
        assert!(
            max_dip - min_dip > 0.05 && max_dip < 0.35,
            "idle dip must breathe gently: min {min_dip:.3} max {max_dip:.3}"
        );
        // Center-weighted on average: the pose dominates the dim edge dots.
        let mut sum_edge = 0.0f32;
        let mut sim2 = Sim::new(wave_params());
        for _ in 0..30 {
            sim2.push(0.0, &[0.0; NUM_BARS], false);
        }
        for _ in 0..300 {
            sim2.push(0.0, &[0.0; NUM_BARS], false);
            let bars = sim2.bars();
            sum_edge += bars[0].max(bars[26]);
        }
        assert!(
            mean > sum_edge / 300.0 * 3.0,
            "posed cluster must dominate edge dots on average: {mean:.3} vs edge {:.3}",
            sum_edge / 300.0
        );
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
        bands[13] = 0.9;
        for _ in 0..30 {
            sim.push(1.0, &bands, true);
        }
        assert!(
            sim.bars()[13] > 0.5,
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
