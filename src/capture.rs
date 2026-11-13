//! capture: default input device → 16k mono f32 chunks over a channel.

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::unbounded;

/// Start default input device; returns rx of f32 chunks @ 16k mono.
pub fn start() -> Result<crossbeam_channel::Receiver<Vec<f32>>> {
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
                .into_iter()
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
                                c.buffer_size().clone(),
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
    let (tx, rx) = unbounded::<Vec<f32>>();

    let err_cb = |e| eprintln!("capture stream error: {e}");
    // shortcut: cpal Stream is !Send, so build+play it on the thread that keeps it.
    let (init_tx, init_rx) = crossbeam_channel::bounded::<Result<()>>(1);
    std::thread::spawn(move || {
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
                    // main.rs buffers and cuts 30 ms frames; pass through as-is
                    let _ = tx.send(resample(&mono, native_rate));
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
    init_rx.recv().context("capture thread died before startup result")??;

    Ok(rx)
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
    use super::resample;

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
}
