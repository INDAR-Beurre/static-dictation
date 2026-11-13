//! capture: default input device → 16k mono f32 chunks over a channel.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{bounded, Receiver, TryRecvError, TrySendError};

/// Start default input device; returns a handle yielding f32 chunks @ 16k mono.
pub fn start() -> Result<AudioIn> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("no default input device")?;

    // Prefer the device default config; fall back to any supported f32 config,
    // preferring 48000 then 44100, mono then stereo.
    let config: Option<cpal::SupportedStreamConfig> = match device.default_input_config() {
        Ok(c) if c.sample_format() == cpal::SampleFormat::F32 => Some(c),
        _ => {
            let mut candidates = device
                .supported_input_configs()
                .with_context(|| "querying supported input configs")?
                .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
                .flat_map(|c| {
                    // expand to concrete rates we care about
                    [48000u32, 44100, 16000]
                        .iter()
                        .filter(|&&r| c.min_sample_rate().0 <= r && r <= c.max_sample_rate().0)
                        .map(move |&r| {
                            cpal::SupportedStreamConfig::new(
                                c.channels().min(2),
                                cpal::SampleRate(r),
                                *c.buffer_size(),
                                cpal::SampleFormat::F32,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            candidates.sort_by_key(|c| {
                let rate_pref = if c.sample_rate().0 == 48000 { 0 } else { 1 };
                (rate_pref, c.channels())
            });
            candidates.into_iter().next()
        }
    };
    let config = config.context("no f32 input stream config available")?;

    let native_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let (tx, rx) = bounded::<Vec<f32>>(256);
    let (cb_tx, cb_rx) = (tx.clone(), rx.clone());
    let warned = AtomicBool::new(false);

    let err_cb = |e| eprintln!("capture stream error: {e}");
    // shortcut: cpal Stream is !Send, so build+play it on the thread that keeps it.
    let (init_tx, init_rx) = crossbeam_channel::bounded::<Result<()>>(1);
    std::thread::spawn(move || {
        let mut resampler = Resampler::new();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // first channel per frame
                    let mono: Vec<f32> = if channels == 1 {
                        data.to_vec()
                    } else {
                        data.chunks_exact(channels).map(|f| f[0]).collect()
                    };
                    // resample to 16k, carrying the tail across callbacks
                    let chunk = resampler.push(&mono, native_rate);
                    if chunk.is_empty() {
                        return;
                    }
                    // never block the audio callback: on a full queue drop the
                    // oldest chunk and retry once, else drop the new chunk
                    if let Err(TrySendError::Full(chunk)) = cb_tx.try_send(chunk) {
                        let _ = cb_rx.try_recv();
                        if let Err(TrySendError::Full(_)) = cb_tx.try_send(chunk) {
                            if !warned.swap(true, Ordering::Relaxed) {
                                eprintln!("capture: audio queue full, dropping audio");
                            }
                        }
                    }
                },
                err_cb,
                None,
            ),
            _ => unreachable!("f32 config selected above"),
        };
        match stream {
            Ok(s) => {
                if let Err(e) = s.play() {
                    let _ = init_tx.send(Err(anyhow!("starting input stream: {e}")));
                    return;
                }
                let _ = init_tx.send(Ok(()));
                // Stream stays alive while this thread parks forever.
                loop {
                    std::thread::park();
                }
            }
            Err(e) => {
                let _ = init_tx.send(Err(anyhow!("building input stream: {e}")));
            }
        }
    });
    init_rx
        .recv()
        .context("capture thread died before startup result")??;

    Ok(AudioIn { rx })
}

/// Handle to the capture stream: bounded queue of 16k mono f32 chunks.
pub struct AudioIn {
    rx: Receiver<Vec<f32>>,
}

impl AudioIn {
    /// Compat wrapper for main's `while let Ok(chunk) = audio.try_recv()` drain loop.
    pub fn try_recv(&self) -> Result<Vec<f32>, TryRecvError> {
        self.rx.try_recv()
    }

    /// Take every queued chunk into `buf`, leaving the queue empty.
    // shortcut: tests use it; main still on the try_recv compat loop — fine as-is
    #[cfg(test)]
    pub fn drain_into(&self, buf: &mut Vec<f32>) {
        while let Ok(chunk) = self.rx.try_recv() {
            buf.extend_from_slice(&chunk);
        }
    }

    /// Discard every queued chunk (audio captured while main was blocked in finalize).
    pub fn drop_backlog(&self) {
        while self.rx.try_recv().is_ok() {}
    }
}

/// Linear resampler that carries leftover tail samples across callback boundaries.
struct Resampler {
    carry: Vec<f32>,
}

impl Resampler {
    fn new() -> Self {
        Self { carry: Vec::new() }
    }

    /// Resample one callback's worth of input; the dropped tail is kept for next call.
    fn push(&mut self, input: &[f32], from: u32) -> Vec<f32> {
        if from == 16000 || input.is_empty() {
            return input.to_vec();
        }
        let mut data = std::mem::take(&mut self.carry);
        data.extend_from_slice(input);
        if data.is_empty() {
            return data;
        }
        let out = resample(&data, from);
        // keep everything from the next output's anchor sample on, so no
        // input sample is lost at chunk boundaries
        let ratio = from as f64 / 16000.0;
        let anchor = ((out.len() as f64) * ratio).floor() as usize;
        self.carry = data[anchor.min(data.len())..].to_vec();
        out
    }
}

/// Pure linear-interpolation resampler: `from` Hz → 16000 Hz.
fn resample(input: &[f32], from: u32) -> Vec<f32> {
    let to = 16000u32;
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (input.len() as f64 / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let t = (pos - i0 as f64) as f32;
        out.push(input[i0] + (input[i1] - input[i0]) * t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{resample, AudioIn};
    use crossbeam_channel::bounded;

    #[test]
    fn sine_48k_to_16k_length() {
        // 1 second of 440 Hz sine at 48000 Hz
        let input: Vec<f32> = (0..48000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
            .collect();
        let out = resample(&input, 48000);
        assert_eq!(out.len(), 16000);
    }

    #[test]
    fn dc_input_stays_constant() {
        let input = vec![0.25f32; 48000];
        let out = resample(&input, 48000);
        assert!(out.iter().all(|&s| s == 0.25));
        let last = *out.last().unwrap();
        assert!((last - 0.25).abs() < 1e-4);
    }

    #[test]
    fn empty_in_empty_out() {
        assert!(resample(&[], 44100).is_empty());
        assert!(resample(&[], 16000).is_empty());
    }

    #[test]
    fn drain_and_drop_backlog() {
        let (tx, rx) = bounded::<Vec<f32>>(256);
        let audio = AudioIn { rx };
        tx.send(vec![0.1; 10]).unwrap();
        tx.send(vec![0.2; 10]).unwrap();
        tx.send(vec![0.3; 10]).unwrap();
        let mut buf = Vec::new();
        audio.drain_into(&mut buf);
        assert_eq!(buf.len(), 30);
        tx.send(vec![1.0; 5]).unwrap();
        tx.send(vec![2.0; 5]).unwrap();
        audio.drop_backlog();
        assert!(audio.try_recv().is_err());
    }
}
