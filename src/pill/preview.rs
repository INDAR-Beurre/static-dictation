//! Visual QA for the dictation pill: renders REAL frames with the REAL
//! renderer (`canvas`) and dumps viewable 32-bit BGRA BMP files.
//!
//! The flipbook is the "island entrance story" (70 frames @ 30 fps = 2.33 s):
//! - frames 0..18 bounce entrance: `grow` driven by a spring, and
//!   master_alpha ramps 0→1 over the first 4 frames
//! - frames 18..54 speech: symmetric 9-band pyramid with syllable bursts
//! - frames 54..62 release/settle: bars decay to the idle dot row
//! - frames 62..70 hide: alpha fades to 0 while grow contracts (no bounce)
//!
//! NOTE: the entrance spring here approximates the one live in `tsar.rs`
//! (semi-implicit Euler, ωn≈20 rad/s, ζ≈0.35). `tsar.rs` is the source of
//! truth for the real animation; this preview only mirrors its constants so
//! offline flipbooks look like the real thing.
//!
//! BMPs land in `{CARGO_MANIFEST_DIR}/target/pill-preview/`:
//! - `island-000.bmp … island-069.bmp` flipbook (736x208, 32-bit BGRA)
//! - `island-idle.bmp` (full-size idle capsule), `island-peak.bmp`
//!   (mid-speech loud frame), `island-half.bmp` (grow≈0.5 mid-bounce)
//!
//! Viewing: run `cargo test -- --ignored`, then flip through
//! `target/pill-preview/island-*.bmp` as a flipbook (any image viewer with
//! arrow keys, or `ffmpeg -framerate 30 -i island-%03d.bmp out.mp4`).

use std::fs;
use std::path::PathBuf;

use super::canvas::{render_pill, SURF_H, SURF_W};
use super::vecmentor::NUM_BARS;

const SCALE: u32 = 2;
const FPS: f32 = 30.0;
const DT: f32 = 1.0 / FPS;

/// Entrance-spring constants, mirrored from tsar.rs (see module doc).
const OMEGA: f32 = 20.0;
const ZETA: f32 = 0.35;
/// canvas.rs clamps grow to this; clamp here too so keyframes show real size.
const GROW_MAX: f32 = 1.25;

/// One generated frame of the island story.
struct Story {
    grow: f32,
    alpha: f32,
    bars: [f32; NUM_BARS],
}

/// Semi-implicit Euler step of the damped spring driving `grow`.
fn spring_step(x: &mut f32, v: &mut f32, target: f32, omega: f32, zeta: f32, dt: f32) {
    *v += (omega * omega * (target - *x) - 2.0 * zeta * omega * *v) * dt;
    *x += *v * dt;
}

/// Idle bar row: small center-weighted dots.
fn idle_bars() -> [f32; NUM_BARS] {
    let mut bars = [0.0; NUM_BARS];
    for (i, bar) in bars.iter_mut().enumerate() {
        *bar = 0.12 * (1.0 - 0.75 * (i as f32 - 4.0).abs() / 4.0);
    }
    bars
}

/// The full 70-frame island entrance story (2.33 s @ 30 fps).
fn story() -> Vec<Story> {
    let mut out = Vec::with_capacity(70);

    // Entrance (frames 0..18): spring grow 0→1 with overshoot wiggles,
    // alpha ramps 0→1 over the first 4 frames.
    let (mut grow, mut gv) = (0.0f32, 0.0f32);
    for f in 0..18u32 {
        spring_step(&mut grow, &mut gv, 1.0, OMEGA, ZETA, DT);
        let alpha = ((f + 1) as f32 / 4.0).min(1.0);
        out.push(Story {
            grow: grow.min(GROW_MAX),
            alpha,
            bars: idle_bars(),
        });
    }

    // Speech (frames 18..54): symmetric pyramid targets with syllable
    // bursts (3 frames loud, 3 soft) and deterministic per-frame variation,
    // smoothed fast-attack / slow-release so motion stays smooth.
    let mut bars = idle_bars();
    for f in 18..54u32 {
        let t = f as f32 / FPS;
        let loud = (f - 18) / 3 % 2 == 0;
        let envelope = if loud { 0.95 } else { 0.35 } * (0.9 + 0.1 * (t * 2.1).sin());
        let mut target = [0.0; NUM_BARS];
        for (i, tg) in target.iter_mut().enumerate() {
            let nominal = 1.0 - 0.75 * (i as f32 - 4.0).abs() / 4.0;
            let var =
                0.22 * (t * 17.0 + i as f32 * 2.1).sin() + 0.12 * (t * 7.3 + i as f32 * 0.9).sin();
            *tg = (envelope * nominal * (1.0 + var)).clamp(0.0, 1.0);
        }
        for i in 0..NUM_BARS {
            let k = if target[i] > bars[i] { 0.55 } else { 0.22 };
            bars[i] += (target[i] - bars[i]) * k;
        }
        out.push(Story {
            grow: 1.0,
            alpha: 1.0,
            bars,
        });
    }

    // Release/settle (frames 54..62): targets decay to the idle dot row.
    let idle = idle_bars();
    for _ in 54..62u32 {
        for i in 0..NUM_BARS {
            bars[i] += (idle[i] - bars[i]) * 0.18;
        }
        out.push(Story {
            grow: 1.0,
            alpha: 1.0,
            bars,
        });
    }

    // Hide (frames 62..70): alpha fades to 0 over ~5 frames while grow
    // contracts fast toward 0 (exponential — no overshoot).
    let mut hide_grow = 1.0f32;
    for local in 0..8u32 {
        hide_grow *= 0.55;
        // shortcut: never reach exactly 0.0 — a fully blank frame cannot be
        // distinguished from a skipped render, so fade bottoms out at ~2%.
        let alpha = (1.0 - local as f32 / 5.0).max(0.02);
        for bar in &mut bars {
            *bar *= 0.8;
        }
        out.push(Story {
            grow: hide_grow,
            alpha,
            bars,
        });
    }
    out
}

/// Three single-keyframe BMPs for judging the look at a glance.
fn keyframes(story: &[Story]) -> Vec<(&'static str, Story)> {
    let mut out = vec![(
        "island-idle.bmp",
        Story {
            grow: 1.0,
            alpha: 1.0,
            bars: idle_bars(),
        },
    )];
    let peak = (18..54)
        .max_by(|&a, &b| {
            let sa: f32 = story[a].bars.iter().sum();
            let sb: f32 = story[b].bars.iter().sum();
            sa.total_cmp(&sb)
        })
        .unwrap();
    out.push((
        "island-peak.bmp",
        Story {
            grow: 1.0,
            alpha: 1.0,
            bars: story[peak].bars,
        },
    ));
    let half = (0..18)
        .min_by(|&a, &b| {
            (story[a].grow - 0.5)
                .abs()
                .total_cmp(&(story[b].grow - 0.5).abs())
        })
        .unwrap();
    out.push((
        "island-half.bmp",
        Story {
            grow: story[half].grow,
            alpha: 1.0,
            bars: story[half].bars,
        },
    ));
    out
}

fn render_frame(s: &Story, t_secs: f32, scale: u32) -> (u32, u32, Vec<u8>) {
    let (w, h) = (SURF_W * scale, SURF_H * scale);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    render_pill(&mut buf, w, h, &s.bars, t_secs, s.alpha, s.grow);
    (w, h, buf)
}

/// 32-bit BGRA BMP writer (BITMAPV4HEADER + BI_BITFIELDS so the alpha
/// channel survives), bottom-up rows. FS errors silently ignored so tests
/// never fail on FS issues.
fn write_bmp(name: &str, w: u32, h: u32, buf: &[u8]) {
    let data_size = w * h * 4;
    let mut out = Vec::with_capacity(122 + data_size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(122u32 + data_size).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    out.extend_from_slice(&122u32.to_le_bytes()); // pixel data offset
                                                  // BITMAPV4HEADER, 108 bytes.
    out.extend_from_slice(&108u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bpp
    out.extend_from_slice(&3u32.to_le_bytes()); // BI_BITFIELDS
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes()); // 72 dpi
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // colors used
    out.extend_from_slice(&0u32.to_le_bytes()); // colors important
    out.extend_from_slice(&0x00FF_0000u32.to_le_bytes()); // R mask
    out.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // G mask
    out.extend_from_slice(&0x0000_00FFu32.to_le_bytes()); // B mask
    out.extend_from_slice(&0xFF00_0000u32.to_le_bytes()); // A mask
    out.extend_from_slice(&0x7352_4742u32.to_le_bytes()); // LCS sRGB
    out.extend(std::iter::repeat_n(0u8, 108 - 60)); // endpoints + gammas

    for y in (0..h).rev() {
        let row = ((y * w * 4) as usize)..(((y + 1) * w * 4) as usize);
        out.extend_from_slice(&buf[row]);
    }
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("target");
    dir.push("pill-preview");
    if fs::create_dir_all(&dir).is_ok() {
        let _ = fs::write(dir.join(name), &out);
    }
}

/// End-to-end headless gate run: generates the whole island story, renders
/// every frame at scale 1 (small/fast), and asserts the story reads right.
#[test]
fn island_story_passes_gates() {
    let story = story();
    assert_eq!(story.len(), 70, "story must produce all 70 frames");

    let mut bright: Vec<usize> = Vec::new();
    for (f, s) in story.iter().enumerate() {
        assert!(
            s.grow.is_finite() && s.alpha.is_finite(),
            "frame {f} non-finite"
        );
        assert!(
            s.bars.iter().all(|b| b.is_finite() && *b >= 0.0),
            "frame {f} has bad bars"
        );

        let (w, h, buf) = render_frame(s, f as f32 / FPS, 1);
        assert_eq!(buf.len(), (w * h * 4) as usize);
        assert!(buf.iter().any(|&b| b != 0), "frame {f} is fully blank");

        let mut opaque_everywhere = true;
        let mut bright_px = 0usize;
        for px in buf.as_chunks::<4>().0 {
            let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
            if a < 200 {
                opaque_everywhere = false;
            }
            // Cyanish bar pixels, mostly opaque (alpha > 200).
            if a > 200 && g > 140 && b > 140 && r < 120 {
                bright_px += 1;
            }
        }
        assert!(
            !opaque_everywhere,
            "frame {f} is opaque everywhere — background not transparent"
        );
        bright.push(bright_px);
    }

    // Entrance frames must paint a strictly smaller bright footprint than
    // mid-speech frames (the capsule starts small and grows).
    let early: usize = bright[..4].iter().sum();
    let mid: usize = bright[20..30].iter().sum();
    assert!(
        mid > early,
        "entrance bright-footprint {early} not < mid-speech {mid} — bounce too big?"
    );
}

/// Writes the 70-frame flipbook (`island-000.bmp … island-069.bmp`, 736x208)
/// plus the three judge keyframes to `target/pill-preview/`.
///
/// Run with `cargo test -- --ignored`, then view as a flipbook
/// (arrow-key image viewer, or `ffmpeg -framerate 30 -i island-%03d.bmp out.mp4`).
#[ignore]
#[test]
fn write_island_flipbook() {
    let story = story();
    for (f, s) in story.iter().enumerate() {
        let (w, h, buf) = render_frame(s, f as f32 / FPS, SCALE);
        write_bmp(&format!("island-{f:03}.bmp"), w, h, &buf);
    }
    for (name, s) in keyframes(&story) {
        let (w, h, buf) = render_frame(&s, 0.0, SCALE);
        write_bmp(name, w, h, &buf);
    }
}
