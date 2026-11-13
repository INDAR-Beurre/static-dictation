//! Endpointer: 30ms-frame voice/silence verdicts -> Ongoing/End decisions.

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Ongoing,
    End,
}

pub struct EndPointer {
    silence_ms: u32,      // budget: max silence after last voice frame
    silence_acc: u32,     // ms of consecutive silence since last voice
    total_ms: u32,        // ms since first push
    seen_voice: bool,     // startup silence must not kill the utterance
}

const FRAME_MS: u32 = 30;
const CAP_MS: u32 = 60_000;

impl EndPointer {
    pub fn new(silence_ms: u32) -> Self {
        EndPointer { silence_ms, silence_acc: 0, total_ms: 0, seen_voice: false }
    }

    /// One call = exactly 30 ms of audio.
    pub fn push(&mut self, is_voice: bool) -> Decision {
        self.total_ms += FRAME_MS;
        if is_voice {
            self.seen_voice = true;
            self.silence_acc = 0;
        } else {
            self.silence_acc += FRAME_MS;
        }

        let end = self.total_ms >= CAP_MS
            || (self.seen_voice && self.silence_acc > self.silence_ms);
        if end {
            *self = EndPointer::new(self.silence_ms); // reset for reuse
            return Decision::End;
        }
        Decision::Ongoing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_budget_700ms() {
        let mut ep = EndPointer::new(700);
        for _ in 0..10 {
            assert_eq!(ep.push(true), Decision::Ongoing);
        }
        for _ in 0..23 {
            assert_eq!(ep.push(false), Decision::Ongoing); // 690ms <= 700
        }
        assert_eq!(ep.push(false), Decision::End); // 720ms > 700
        // fresh after reset
        assert_eq!(ep.push(true), Decision::Ongoing);
    }

    #[test]
    fn hard_cap_60s_even_if_voice() {
        let mut ep = EndPointer::new(100_000);
        for i in 1..2000 {
            assert_eq!(ep.push(true), Decision::Ongoing, "push {} was Ongoing", i);
        }
        assert_eq!(ep.push(true), Decision::End); // 2000*30 = 60000
    }

    #[test]
    fn never_voice_stays_ongoing() {
        let mut ep = EndPointer::new(700);
        for _ in 0..100 {
            assert_eq!(ep.push(false), Decision::Ongoing);
        }
    }
}
