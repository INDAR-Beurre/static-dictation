//! Voice-band spectrum analysis for the dictation pill equalizer.
//!
//! [`energies`] turns one 30 ms audio frame (f32 samples @ 16 kHz) into 27
//! log-spaced band energies in `0.0..=1.0`, one per equalizer bar.
//!
//! Pipeline, in order:
//!
//! 1. **DC removal** — the frame mean is subtracted so rumble / DC offset does
//!    not light up the lowest bands.
//! 2. **Goertzel per band** — 27 log-spaced frequencies spanning
//!    `[250 … 4000]` Hz (see `vecmentor::BAND_CENTERS_HZ`, ratio 16^(1/26)
//!    per step, voice core 1000 Hz at bar 13); magnitude is
//!    `sqrt(power) / N` (normalized, frame-length independent).
//! 3. **Silence gate** — if the strongest band magnitude is below a small
//!    floor, the frame is treated as silence and all 27 outputs are `0.0`.
//!    This keeps the pill from flickering on mic noise floor.
//! 4. **dB mapping** — `20*log10(mag + 1e-9)`, with `(-50..0 dB)` mapped
//!    linearly onto `0.0..1.0`.
//! 5. **Per-frame max normalization** — the strongest band of THIS frame is
//!    scaled to exactly `1.0`, so the pill always shows contrast relative to
//!    the current voice level rather than an absolute calibration.
//! 6. **Perceptual curve** — values are raised to the power 1.5 to expand the
//!    dynamic range visually (quiet bands sink, loud bands pop).
//!
//! The function is pure, stateless, std-only, and total: an empty slice, a
//! frame containing NaN/inf, or an all-zero frame returns all zeros instead of
//! panicking or producing NaN. Any frame length is handled (Goertzel does not
//! care about alignment with DFT bins).

/// Sample rate the frame data is expected at (hard constant by design).
const SAMPLE_RATE: f32 = 16000.0;

/// Log-spaced voice-band center frequencies, low → high. Re-exported from
/// `vecmentor::BAND_CENTERS_HZ` so both modules share one source of truth.
use crate::pill::vecmentor::{BAND_CENTERS_HZ as BAND_HZ, NUM_BARS as NUM_BANDS};

/// Silence gate: frames whose best band magnitude is below this are silence.
///
/// shortcut: tuned empirically, not calibrated against real mic hardware —
/// revisit when real input levels are known. Chosen so a 0.001-amplitude tone
/// (band mag ≈ 5e-4) gates off while full-scale white noise (best band mag
/// ≈ 0.03–0.06 for a 480-sample frame) still passes.
const SILENCE_FLOOR: f32 = 0.02;

/// 27 log-spaced voice-band energies, each 0.0..=1.0, normalized so the
/// strongest band of THIS frame ≈ 1.0. Silent / near-DC / corrupted frames
/// return all zeros.
/// frame: f32 samples at 16 kHz, typically 480 (30 ms), any len handled safely.
pub fn energies(frame: &[f32]) -> [f32; NUM_BANDS] {
    let n = frame.len();
    if n == 0 {
        return [0.0; NUM_BANDS];
    }

    // Reject corrupted input outright; also accumulate for the DC mean.
    let mut sum = 0.0f32;
    for &s in frame {
        if !s.is_finite() {
            return [0.0; NUM_BANDS];
        }
        sum += s;
    }
    let mean = sum / n as f32;

    // Goertzel per band (stateless, per call).
    let mut mags = [0.0f32; NUM_BANDS];
    for (bi, &f0) in BAND_HZ.iter().enumerate() {
        let coeff = 2.0 * (2.0 * core::f32::consts::PI * f0 / SAMPLE_RATE).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &s in frame {
            let s0 = (s - mean) + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
        mags[bi] = power.max(0.0).sqrt() / n as f32;
    }

    // Silence gate on the raw best magnitude (before dB mapping).
    let best_mag = mags.iter().copied().fold(0.0f32, f32::max);
    if best_mag < SILENCE_FLOOR {
        return [0.0; NUM_BANDS];
    }

    // dB scale: (-50..0 dB) -> (0..1).
    let mut mapped = [0.0f32; NUM_BANDS];
    for i in 0..NUM_BANDS {
        let db = 20.0 * (mags[i] + 1e-9).log10();
        mapped[i] = ((db + 50.0) / 50.0).clamp(0.0, 1.0);
    }

    // Per-frame max-normalize (strongest band -> 1.0), then perceptual curve.
    // best_mag >= floor guarantees mapped max > 0, so no division by zero.
    let max_mapped = mapped.iter().copied().fold(0.0f32, f32::max);
    let mut out = [0.0f32; NUM_BANDS];
    for i in 0..NUM_BANDS {
        out[i] = (mapped[i] / max_mapped).powf(1.5);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 480; // 30 ms @ 16 kHz

    fn sine(freq: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (2.0 * core::f32::consts::PI * freq * i as f32 / SAMPLE_RATE).sin())
            .collect()
    }

    fn lcg_noise(n: usize) -> Vec<f32> {
        let mut seed: u32 = 12345;
        (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                // full-scale uniform noise in [-1, 1)
                (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect()
    }

    #[test]
    fn silence_is_all_zeros() {
        assert_eq!(energies(&[0.0; N]), [0.0; NUM_BANDS]);
        assert_eq!(energies(&[]), [0.0; NUM_BANDS]);
        assert_eq!(energies(&[0.0]), [0.0; NUM_BANDS]);
    }

    #[test]
    fn corrupted_input_is_all_zeros_no_nan() {
        let mut bad = vec![0.1f32; N];
        bad[10] = f32::NAN;
        assert_eq!(energies(&bad), [0.0; NUM_BANDS]);
        let mut inf = vec![0.1f32; N];
        inf[10] = f32::INFINITY;
        assert_eq!(energies(&inf), [0.0; NUM_BANDS]);
    }

    #[test]
    fn one_khz_tone_peaks_band_13() {
        let out = energies(&sine(1000.0, 0.5, N));
        assert!(out[13] > 0.9, "band 13 should be ~1.0, got {}", out[13]);
        for (i, v) in out.iter().enumerate() {
            assert!(!v.is_nan());
            assert!(*v >= 0.0 && *v <= 1.0);
            if i != 13 {
                assert!(
                    *v < 0.5,
                    "neighbor band {} should be clearly lower, got {}",
                    i,
                    v
                );
            }
        }
    }

    #[test]
    fn four_khz_tone_peaks_band_26() {
        let out = energies(&sine(4000.0, 0.5, N));
        assert!(out[26] > 0.9, "band 26 should be ~1.0, got {}", out[26]);
        for (i, v) in out.iter().enumerate() {
            if i != 26 {
                assert!(
                    *v < 0.5,
                    "neighbor band {} should be clearly lower, got {}",
                    i,
                    v
                );
            }
        }
    }

    #[test]
    fn low_tone_peaks_band_0() {
        let out = energies(&sine(250.0, 0.5, N));
        assert!(out[0] > 0.9, "band 0 should be ~1.0, got {}", out[0]);
        for (i, v) in out.iter().enumerate() {
            if i != 0 {
                // 0.9, not 0.7: the 27-band log grid puts band 1 at ~322 Hz,
                // just 72 Hz up — the 250 Hz test tone's skirt legitimately
                // reaches it; farther bands must stay clearly lower.
                assert!(*v < 0.9, "band {} should be clearly lower, got {}", i, v);
            }
        }
    }

    #[test]
    fn quiet_but_audible_passes_very_quiet_gates() {
        let audible = energies(&sine(1000.0, 0.3, N));
        assert!(
            audible.iter().any(|&v| v > 0.0),
            "audible tone must be non-zero"
        );
        let very_quiet = energies(&sine(1000.0, 0.001, N));
        assert_eq!(
            very_quiet, [0.0; NUM_BANDS],
            "very quiet tone must gate to zeros"
        );
    }

    #[test]
    fn noise_fills_bands_within_range() {
        let out = energies(&lcg_noise(N));
        let nonzero = out.iter().filter(|&&v| v > 1e-3).count();
        assert!(
            nonzero >= 10,
            "expected >=10 non-zero bands, got {}",
            nonzero
        );
        for v in out {
            assert!(!v.is_nan());
            assert!((0.0..=1.0).contains(&v), "band value {} out of range", v);
        }
    }

    #[test]
    fn output_is_always_num_bands() {
        assert_eq!(energies(&sine(1000.0, 0.5, N)).len(), NUM_BANDS);
        assert_eq!(energies(&sine(1000.0, 0.5, 7)).len(), NUM_BANDS);
        assert_eq!(energies(&lcg_noise(1024)).len(), NUM_BANDS);
    }
}
