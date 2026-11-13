//! Energy + zero-crossing-rate VAD with an adaptive EMA noise floor and a DC blocker.

/// Clamps the first-frame floor seed; a startup pop must not brick the VAD.
const FLOOR_SEED_CAP: f32 = 0.001;

pub struct Vad {
    /// Exponential moving average of frame energy over noise-only frames.
    noise_floor: f32,
    /// Whether noise_floor has been initialized with the first frame's energy.
    initialized: bool,
    /// First-order DC blocker (one-pole high-pass) state: y = x - x_prev + R*y_prev.
    x_prev: f32,
    y_prev: f32,
    filter_primed: bool,
}

impl Vad {
    pub fn new() -> Self {
        Self { noise_floor: 1e-7, initialized: false, x_prev: 0.0, y_prev: 0.0, filter_primed: false }
    }

    /// Removes constant DC offset so energy/ZCR see the AC content only.
    /// Without it a DC offset >= ~0.5 lifts every frame's energy above the
    /// adapted floor and voiced speech never crosses the gate.
    fn dc_block(&mut self, frame: &[f32]) -> Vec<f32> {
        if !self.filter_primed {
            // set x_prev to the first sample so the turn-on transient vanishes
            self.x_prev = frame[0];
            self.filter_primed = true;
        }
        frame
            .iter()
            .map(|&x| {
                let y = x - self.x_prev + 0.995 * self.y_prev;
                self.x_prev = x;
                self.y_prev = y;
                y
            })
            .collect()
    }

    /// frame = exactly 480 samples @ 16 kHz (30 ms). true if frame is voice.
    pub fn is_voice(&mut self, frame: &[f32]) -> bool {
        let len = frame.len();
        if len == 0 {
            return false;
        }
        let frame = self.dc_block(frame);
        let energy = frame.iter().map(|s| s * s).sum::<f32>() / len as f32;
        let zero_count = frame.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        let zcr = zero_count as f32 / (len - 1) as f32;

        if !self.initialized {
            // seed the floor with the first frame; min floor keeps later multiplications sane.
            // Cap the seed: a loud startup click here would pin the floor high and every real
            // frame below 8x of it reads as silence until slow adaptation drags it down (~8s).
            self.noise_floor = energy.max(1e-7).min(FLOOR_SEED_CAP);
            self.initialized = true;
        }

        let is_voice =
            energy > (self.noise_floor * 8.0).max(1e-6) // 8x floor: speech energy sits well above room noise
            && zcr > 0.005 // below ~0.5% crossings it's DC/flat hum, not voiced
            && zcr < 0.30; // above 30% crossings it's hiss/fricative noise, not voiced

        if !is_voice {
            // spike guard: a loud transient classified as noise would drag the floor up,
            // so cap its contribution
            if energy > 30.0 * self.noise_floor {
                // shortcut: clamps transient energy to 30x floor instead of rejecting it entirely, upgrade to a proper outlier test if false-ENDs appear
                self.noise_floor += 0.05 * (30.0 * self.noise_floor - self.noise_floor);
            } else {
                // alpha 0.05: slow adaptation (~20 noise frames to shift), resists a burst flipping the floor
                self.noise_floor += 0.05 * (energy - self.noise_floor);
            }
        }
        is_voice
    }
}

impl Default for Vad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Vad;

    const FRAME: usize = 480;

    fn flat(amp: f32) -> Vec<f32> {
        vec![amp; FRAME]
    }

    fn sine(amp: f32, freq: f32) -> Vec<f32> {
        (0..FRAME)
            .map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / 16000.0).sin())
            .collect()
    }

    #[test]
    fn constant_low_level_is_not_voice() {
        let mut v = Vad::new();
        for _ in 0..50 {
            assert!(!v.is_voice(&flat(0.001)));
        }
    }

    #[test]
    fn loud_sine_after_silence_is_voice() {
        let mut v = Vad::new();
        for _ in 0..20 {
            assert!(!v.is_voice(&flat(0.0001))); // build a very low noise floor
        }
        assert!(v.is_voice(&sine(0.5, 200.0)));
    }

    #[test]
    fn noise_floor_adaptation_changes_classification() {
        let frame = sine(0.05, 200.0); // medium frame, fixed across both phases

        let mut v = Vad::new();
        let before = v.is_voice(&frame); // classified before adaptation

        // feed quieter noise until the floor drops well below the frame
        for _ in 0..50 {
            v.is_voice(&flat(0.0005));
        }

        let after = v.is_voice(&frame);
        assert_ne!(before, after, "fixed frame must classify differently after floor adapts");
    }

    /// Speech-like signal riding on a 0.5 DC offset: the DC blocker must strip
    /// the offset so the AC energy still crosses the gate.
    #[test]
    fn dc_offset_with_speech_is_voice() {
        let mut v = Vad::new();
        // quiet pure-DC line first: floor seeds from (near-)zero AC energy
        for _ in 0..20 {
            v.is_voice(&flat(0.5));
        }
        // same DC level + a 200 Hz oscillation the old code mistook for noise
        for _ in 0..10 {
            let frame: Vec<f32> = (0..FRAME)
                .map(|i| 0.5 + 0.4 * (2.0 * std::f32::consts::PI * 200.0 * i as f32 / 16000.0).sin())
                .collect();
            assert!(v.is_voice(&frame), "0.5-DC speech frame must pass the gate");
        }
    }

    /// A loud startup pop must not seed the floor so high the VAD goes deaf.
    #[test]
    fn startup_pop_does_not_seed_absurd_floor() {
        let mut v = Vad::new();
        v.is_voice(&sine(0.9, 200.0)); // loud pop seeds the floor
        // normal speech afterwards must be detected immediately, not after ~8s of adaptation
        assert!(v.is_voice(&sine(0.2, 200.0)));
    }
}
