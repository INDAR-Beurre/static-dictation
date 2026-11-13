//! Visual QA for the dictation pill: renders REAL frames with the REAL
//! simulator (`vecmentor`) and REAL renderer (`canvas`), asserts
//! production-quality gates, and dumps viewable 24-bit BMP files.
//!
//! Scenarios rendered:
//! - 3 idle frames (level 0, zero bands, not speaking)
//! - 8 speech frames (one-hot loud band rotating across all 9 bars,
//!   sine-varied level 0.6–1.3, speaking)
//! - 3 fade frames (speech state at master_alpha 0.25 / 0.6 / 1.0)
//!
//! BMPs land in `{CARGO_MANIFEST_DIR}/target/pill-preview/` as
//! `idle-N.bmp`, `speech-N.bmp`, `fade-NN.bmp` (written by the default
//! test run) plus `anim-00.bmp … anim-59.bmp` (written by the ignored
//! animation test). All filesystem errors are silently ignored so tests
//! never fail on FS issues.
//!
//! Viewing: run `cargo test -- --ignored`, then flip through
//! `target/pill-preview/anim-*.bmp` as a flipbook (any image viewer with
//! arrow keys, or `ffmpeg -framerate 30 -i anim-%02d.bmp out.mp4`).

use std::fs;
use std::path::PathBuf;

use super::canvas::{render_pill, SURF_H, SURF_W};
use super::vecmentor::{wave_params, Sim, NUM_BARS};

const SCALE: u32 = 2;

enum Scenario {
    /// level 0, zero bands, speaking=false
    Idle,
    /// one-hot band at `idx`, speaking=true
    Speech(usize),
    /// speech state rendered at a given master_alpha
    Fade(f32),
}

struct Frame {
    name: String,
    scenario: Scenario,
    t: f32,
}

fn scenarios() -> Vec<Frame> {
    let mut frames = Vec::new();
    for i in 0..3 {
        frames.push(Frame { name: format!("idle-{i}.bmp"), scenario: Scenario::Idle, t: i as f32 * 0.17 });
    }
    for i in 0..8 {
        frames.push(Frame { name: format!("speech-{i}.bmp"), scenario: Scenario::Speech(i), t: i as f32 * 0.13 });
    }
    for (a, tag) in [(0.25, "25"), (0.6, "60"), (1.0, "100")] {
        frames.push(Frame { name: format!("fade-{tag}.bmp"), scenario: Scenario::Fade(a), t: 0.42 });
    }
    frames
}

fn one_hot(idx: usize) -> [f32; NUM_BARS] {
    let mut bands = [0.0; NUM_BARS];
    bands[idx] = 1.0;
    bands
}

/// Level 0.6–1.3, sine-varied so consecutive speech frames differ.
fn speech_level(t: f32) -> f32 {
    0.95 + 0.35 * (t * 5.0).sin()
}

/// (level, bands, speaking, master_alpha) for a scenario.
fn input(scenario: &Scenario) -> (f32, [f32; NUM_BARS], bool, f32) {
    match scenario {
        Scenario::Idle => (0.0, [0.0; NUM_BARS], false, 1.0),
        Scenario::Speech(idx) => (speech_level(0.7), one_hot(*idx), true, 1.0),
        Scenario::Fade(a) => (speech_level(0.7), one_hot(2), true, *a),
    }
}

/// Drive a fresh sim with the same input several steps so smoothed bars
/// reach a representative steady state, then snapshot them.
fn drive(level: f32, bands: &[f32; NUM_BARS], speaking: bool) -> Vec<f32> {
    let mut sim = Sim::new(wave_params());
    for _ in 0..30 {
        sim.push(level, bands, speaking);
    }
    sim.bars().to_vec()
}

fn render(bars: &[f32], t: f32, alpha: f32) -> (u32, u32, Vec<u8>) {
    let (w, h) = (SURF_W * SCALE, SURF_H * SCALE);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    render_pill(&mut buf, w, h, bars, t, alpha);
    (w, h, buf)
}

struct Stats {
    semi_alpha: usize,   // 0 < a < 255 (soft AA edges, shadow, glow)
    teal: usize,         // bar-core teal: g>200 && b>200 && r<90
    bright_teal: usize,  // teal AND mostly opaque
    alpha_sum: u64,
}

fn stats(w: u32, h: u32, buf: &[u8]) -> Stats {
    let mut s = Stats { semi_alpha: 0, teal: 0, bright_teal: 0, alpha_sum: 0 };
    for px in buf.chunks_exact(4) {
        let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
        if a > 0 && a < 255 {
            s.semi_alpha += 1;
        }
        if g > 200 && b > 200 && r < 90 {
            s.teal += 1;
            if a > 200 {
                s.bright_teal += 1;
            }
        }
        s.alpha_sum += a as u64;
    }
    let _ = (w, h);
    s
}

/// Std-only 24-bit BMP writer: 54-byte header, BGR bottom-up rows,
/// 4-byte row padding. Alpha is composited over near-black so the
/// viewer sees what the compositor would. FS errors silently ignored.
fn write_bmp(name: &str, w: u32, h: u32, buf: &[u8]) {
    let row = (w * 3) as usize;
    let pad = (4 - row % 4) % 4;
    let data_size = (row + pad) * h as usize;
    let mut out = Vec::with_capacity(54 + data_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(54u32 + data_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    out.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset
    out.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER size
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&24u16.to_le_bytes()); // bpp
    out.extend_from_slice(&0u32.to_le_bytes()); // no compression
    out.extend_from_slice(&(data_size as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes()); // 72 dpi
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // palette
    out.extend_from_slice(&0u32.to_le_bytes());
    for y in (0..h).rev() {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let a = buf[i + 3] as f32 / 255.0;
            out.push((buf[i] as f32 * a) as u8);
            out.push((buf[i + 1] as f32 * a) as u8);
            out.push((buf[i + 2] as f32 * a) as u8);
        }
        for _ in 0..pad {
            out.push(0);
        }
    }
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("target");
    dir.push("pill-preview");
    if fs::create_dir_all(&dir).is_ok() {
        let _ = fs::write(dir.join(name), &out);
    }
}

#[test]
fn preview_frames_pass_production_gates() {
    let frames = scenarios();
    let mut rendered: Vec<(String, u32, u32, Vec<u8>, Stats)> = Vec::new();

    for f in &frames {
        let (level, bands, speaking, alpha) = input(&f.scenario);
        assert!(!level.is_nan(), "level is NaN in {}", f.name);
        assert!(bands.iter().all(|b| !b.is_nan()), "NaN band in {}", f.name);
        let bars = drive(level, &bands, speaking);
        assert!(bars.iter().all(|b| !b.is_nan()), "NaN bar in {}", f.name);

        let (w, h, buf) = render(&bars, f.t, alpha);
        // Buffer size sanity: BGRA, stride = width*4, full coverage.
        assert_eq!(buf.len(), (w * h * 4) as usize);

        // Gate: canvas corner alpha == 0 (padding around pill, no bleed).
        let corners = [
            3,                                     // top-left
            ((w - 1) * 4 + 3) as usize,            // top-right
            (((h - 1) * w) * 4 + 3) as usize,      // bottom-left
            (((h - 1) * w + (w - 1)) * 4 + 3) as usize, // bottom-right
        ];
        for (ci, &c) in corners.iter().enumerate() {
            assert_eq!(buf[c], 0, "corner {ci} alpha != 0 in {}", f.name);
        }

        let st = stats(w, h, &buf);
        rendered.push((f.name.clone(), w, h, buf, st));
    }

    let by_name = |n: &str| rendered.iter().find(|(name, ..)| name == n).unwrap();

    // Gate: soft AA edges + shadow + glow exist (not binary jaggies).
    // Generous floor: at scale 2 the pill outline alone yields thousands.
    for (name, .., st) in &rendered {
        assert!(st.semi_alpha > 300, "{}: only {} semi-transparent px (soft edges/shadow/glow missing)", name, st.semi_alpha);
    }

    // Gate: a bar-core pixel in a speech frame is teal (g>200, b>200, r<90).
    let speech = by_name("speech-4.bmp");
    assert!(speech.4.teal > 30, "speech-4: only {} teal px — bars not rendering teal", speech.4.teal);

    // Gate: master_alpha scales — mean alpha at 0.25 < half of mean at 1.0.
    let fade25 = by_name("fade-25.bmp");
    let fade100 = by_name("fade-100.bmp");
    let mean = |st: &Stats, w: u32, h: u32| st.alpha_sum as f64 / ((w * h) as f64);
    let m25 = mean(&fade25.4, fade25.1, fade25.2);
    let m100 = mean(&fade100.4, fade100.1, fade100.2);
    assert!(m25 < 0.5 * m100, "fade-25 mean alpha {m25} not < half of fade-100 mean alpha {m100}");

    // Gate: speech has more bright teal pixels than idle.
    let idle = by_name("idle-0.bmp");
    assert!(
        speech.4.bright_teal > idle.4.bright_teal,
        "speech bright_teal {} <= idle bright_teal {}",
        speech.4.bright_teal, idle.4.bright_teal
    );

    // All gates passed — dump the frames for human inspection.
    for (name, w, h, buf, _) in &rendered {
        write_bmp(name, *w, *h, buf);
    }
}

/// Renders a 60-frame (2 s @ 30 Hz) voice-like animation to
/// `target/pill-preview/anim-00.bmp … anim-59.bmp`.
///
/// Input: alternating speaking windows of 1.2 s with one-hot bands
/// rotating per burst, level pulses between 0.3 and 1.3; idle windows
/// are level 0 with zero bands. Frames are driven continuously through
/// one sim so the bars ease naturally.
///
/// Run with `cargo test -- --ignored`, then view as a flipbook
/// (arrow-key image viewer, or `ffmpeg -framerate 30 -i anim-%02d.bmp out.mp4`).
#[ignore]
#[test]
fn write_human_preview_animation() {
    const FPS: f32 = 30.0;
    const BURST_SECS: f32 = 1.2; // speaking window
    const CYCLE_SECS: f32 = 2.0; // 1.2 s speaking + 0.8 s idle
    let mut sim = Sim::new(wave_params());
    for f in 0..60u32 {
        let t = f as f32 / FPS;
        let in_burst = t % CYCLE_SECS < BURST_SECS;
        let burst_idx = ((t / CYCLE_SECS) as usize) % NUM_BARS;
        let (level, bands, speaking) = if in_burst {
            // pulse 0.3..1.3, two pulses per burst
            (0.8 + 0.5 * (t * 5.0).sin(), one_hot(burst_idx), true)
        } else {
            (0.0, [0.0; NUM_BARS], false)
        };
        sim.push(level, &bands, speaking);
        let (w, h, buf) = render(sim.bars(), t, 1.0);
        write_bmp(&format!("anim-{f:02}.bmp"), w, h, &buf);
    }
}
