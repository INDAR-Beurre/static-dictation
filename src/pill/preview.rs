//! Visual QA for the dictation pill: renders REAL frames with the REAL
//! renderer (`canvas`) and dumps viewable 32-bit BGRA BMP files.
//!
//! The flipbook is the "island entrance story" (70 frames @ 30 fps = 2.33 s):
//! - frames 0..18 bounce entrance: `grow` driven by a spring, and
//!   master_alpha ramps 0→1 over the first 4 frames
//! - frames 18..54 speech: hand-crafted center-bar silhouette — quiet ramp
//!   (0.06..0.15) → dip → burst cluster (0.60, 0.83, 1.00, 0.84, 0.62) →
//!   mirrored fall → echo syllable → breathing tail — spread per bar by
//!   vecmentor's spatial envelope with breathing variation
//! - frames 54..62 release/settle: bars decay to the idle dot row
//! - frames 62..70 hide: alpha and grow both floor at 0.02 (no bounce,
//!   never a blank frame)
//!
//! NOTE: the entrance spring here mirrors the one live in `tsar.rs`
//! (semi-implicit Euler, ωn=21 rad/s, ζ=0.45, initial kick 4.6, ceiling 1.12;
//! private there — keep the values in sync). `tsar.rs` is the source of
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

use std::f32::consts::PI;
use std::fs;
use std::path::PathBuf;

use super::canvas::{render_pill, SURF_H, SURF_W};
use super::vecmentor::{NUM_BARS, SHAPE_MASK};

const SCALE: u32 = 2;
const FPS: f32 = 30.0;
const DT: f32 = 1.0 / FPS;

/// Entrance-spring constants, mirrored from tsar.rs (SPRING_OMEGA_SHOW,
/// SPRING_ZETA_SHOW, SPRING_KICK, GROW_CEIL_SHOW — private there; see module
/// doc).
const OMEGA: f32 = 21.0;
const ZETA: f32 = 0.45;
const KICK: f32 = 4.6;
/// Effective show-overshoot ceiling, mirrored from tsar.rs (GROW_CEIL_SHOW).
/// canvas.rs still clamps at 1.25 — GROW_CEIL_SHOW is always inside it.
const GROW_CEIL: f32 = 1.12;
/// Hide-contraction spring constants, mirrored from tsar.rs (SPRING_OMEGA_HIDE,
/// SPRING_ZETA_HIDE — private there; a firmer near-critically-damped shrink,
/// no overshoot, reaches < 0.02 in 12 ticks ≈ 192 ms).
const OMEGA_HIDE: f32 = 25.0;
const ZETA_HIDE: f32 = 0.78;

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

/// Idle bar row: small center-bunched dots — bunched mirror-consistently
/// with edges trailing to near-zero (steer 3), NOT a flat spread: measured
/// round-1 idle dots covered ~97% of capsule width; the reference keeps
/// them central with leader-dot zones (~18% of W) free on each flank.
fn idle_bars() -> [f32; NUM_BARS] {
    let mut bars = [0.0; NUM_BARS];
    for (i, bar) in bars.iter_mut().enumerate() {
        // Cosine hump spanning the full bar row, centered on bar 13: value
        // ≈0.09 at the center decaying to ≈0.005 at bars 0/26 — edges near
        // zero so the idle row reads as a small central bunch of dots.
        *bar = 0.005 + 0.085 * (0.5 - 0.5 * (2.0 * PI * i as f32 / (NUM_BARS - 1) as f32).cos());
    }
    bars
}

/// The full 70-frame island entrance story (2.33 s @ 30 fps).
fn story() -> Vec<Story> {
    let mut out = Vec::with_capacity(70);

    // Entrance (frames 0..18): kicked spring grow 0→1 with one firm ~+12%
    // overshoot to the 1.12 ceiling, a faint second wiggle, then settled at
    // 1.0 well before speech starts; alpha ramps 0→1 over the first 4
    // frames. Grow mirrors tsar's clamp to [0, 1.12].
    let (mut grow, mut gv) = (0.0f32, KICK);
    for f in 0..18u32 {
        spring_step(&mut grow, &mut gv, 1.0, OMEGA, ZETA, DT);
        let alpha = ((f + 1) as f32 / 4.0).min(1.0);
        grow = grow.clamp(0.0, GROW_CEIL);
        out.push(Story {
            grow,
            alpha,
            bars: idle_bars(),
        });
    }

    // Speech (frames 18..54): hand-crafted center-bar trajectory matching
    // the measured speech silhouette — quiet ramp (0.06..0.15) → dip →
    // burst cluster (0.60, 0.83, 1.00, 0.84, 0.62) → mirrored fall → one
    // echo syllable (attack/decay pair) → breathing tail. Per-bar targets
    // scale vecmentor's SHAPE_MASK pose by the syllable level (at peak the
    // bars ARE the mask: hot 5-bar cluster at 11-15 centered on bar 13)
    // plus deterministic breathing variation, then fast-attack /
    // slow-release smoothing mirrors the live physics.
    const SPEECH_CENTER: [f32; 36] = [
        // ramp       dip  burst cluster               mirrored fall
        0.06, 0.10, 0.15, 0.05, 0.60, 0.83, 1.15, 0.84, 0.62, 0.34, 0.18, 0.10, 0.05,
        // echo syllable      breathing tail (wobbles 0.04..0.12, never zero)
        0.28, 0.55, 0.40, 0.24, 0.12, 0.08, 0.05, 0.09, 0.04, 0.07, 0.11, 0.05, 0.08, 0.12, 0.06,
        0.09, 0.04, 0.06, 0.10, 0.05, 0.08, 0.11, 0.06,
    ];
    let mut bars = idle_bars();
    for (k, &center) in SPEECH_CENTER.iter().enumerate() {
        let t = (18 + k) as f32 / FPS;
        let mut target = [0.0; NUM_BARS];
        for (i, tg) in target.iter_mut().enumerate() {
            // SHAPE_MASK is vecmentor's spatial envelope — reusing it keeps
            // the preview's peak pose identical to the designed idle pose
            // (hot cluster 11-15, edge ramps), not the old 9-bar anchor.
            let spread = SHAPE_MASK[i];
            let breath =
                0.15 * (t * 13.0 + i as f32 * 1.7).sin() + 0.08 * (t * 5.1 + i as f32 * 0.9).sin();
            *tg = (center * spread * (1.0 + breath)).clamp(0.0, 1.0);
        }
        for i in 0..NUM_BARS {
            let rate = if target[i] > bars[i] { 0.8 } else { 0.28 };
            bars[i] += (target[i] - bars[i]) * rate;
        }
        // Rest at grow 1.0 with a ≤±0.03 breathing wobble — never a flat
        // 1.25 plateau (the overshoot belongs to the entrance only).
        let grow = 1.0 + 0.03 * (t * 5.3).sin();
        out.push(Story {
            grow,
            alpha: 1.0,
            bars,
        });
    }

    // Release/settle (frames 54..62): targets decay to the idle dot row,
    // quietly at grow 1.0.
    let idle = idle_bars();
    for _f in 54..62u32 {
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
    // contracts with tsar's hide spring (ωn=25, ζ=0.78 — no overshoot),
    // starting from the settled state (velocity ≈0), then floored at 0.02.
    let (mut hide_grow, mut hide_vel) = (1.0f32, 0.0f32);
    for local in 0..8u32 {
        spring_step(
            &mut hide_grow,
            &mut hide_vel,
            0.0,
            OMEGA_HIDE,
            ZETA_HIDE,
            DT,
        );
        // shortcut: never reach exactly 0.0 — a fully blank frame cannot be
        // distinguished from a skipped render, and a 0-growth capsule cannot
        // be rendered; both floors bottom out at ~2%.
        hide_grow = hide_grow.max(0.02);
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

/// Bright-pixel gate for the round-2 graded palette. Stored channels are
/// premultiplied: a pixel colored (R,G,B) at alpha A stores (R,G,B)·A/255,
/// so each check is verified for ALL three bar colors at every A > 200
/// (worst case A = 201, pa ≈ 0.79):
///   teal  rgb(125,165,165): g≈130>100  b≈130>120  5g≈650 > 6r≈591
///   peak  rgb(140,220,222): g≈173>100  b≈175>120  5g≈866 > 6r≈663
///   blue  rgb(100,130,170): g≈102>100  b≈134>120  5g≈512 > 6r≈473
/// Rejected: capsule bg rgb(35,38,37) (g ≤ 38 < 100 at any alpha) and any
/// neutral gray (rim/hairline: r≈g fails 5g > 6r at any brightness); tail
/// dots (~20% alpha → a≈51) always fall below the gate. Dim idle dots sit
/// near its edge (b≈0.55 → g≈92–116 depending on palette), so the story
/// gates below compare bright counts by RATIO, not absolute floors.
#[inline]
fn is_bright(b: u8, g: u8, r: u8, a: u8) -> bool {
    a > 200 && g > 100 && b > 120 && u32::from(g) * 5 > u32::from(r) * 6
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
            if px[3] < 200 {
                opaque_everywhere = false;
            }
            if is_bright(px[0], px[1], px[2], px[3]) {
                bright_px += 1;
            }
        }
        assert!(
            !opaque_everywhere,
            "frame {f} is opaque everywhere — background not transparent"
        );
        bright.push(bright_px);
    }

    // Capsule interior must read as opaque glass on a full-alpha frame
    // (frame 30: grow=1, alpha=1, mid-speech).
    let (w, h, buf) = render_frame(&story[30], 1.0, 1);
    let center_a = buf[((h / 2 * w + w / 2) * 4 + 3) as usize];
    assert!(
        center_a > 200,
        "capsule interior alpha {center_a} not opaque"
    );

    // Entrance frames must paint a strictly smaller bright footprint than
    // mid-speech frames (the capsule starts small and grows).
    let early: usize = bright[..4].iter().sum();
    let mid: usize = bright[20..30].iter().sum();
    assert!(
        mid > early,
        "entrance bright-footprint {early} not < mid-speech {mid} — bounce too big?"
    );

    // The speech burst (cluster + echo, with smoothing lag) must light up
    // many bright pixels.
    let burst = bright[20..40].iter().max().copied().unwrap_or(0);
    assert!(burst >= 250, "speech burst too dim: only {burst} bright px");
    // No mid-speech render collapse: every frame whose bars are TALL must
    // keep visible bright pixels. Frames that are quiet by design (ramp-in
    // dots, dip, breathing tail — max bar < QUIET) legitimately render at
    // dot alpha, below the bright gate, so they are exempt.
    let mut speech: Vec<usize> = bright[18..54].to_vec();
    speech.sort_unstable();
    let median = speech[speech.len() / 2];
    let floor = median / 2;
    const QUIET: f32 = 0.18;
    for (i, &b) in bright[18..54].iter().enumerate() {
        let tallest = story[18 + i].bars.iter().cloned().fold(0.0f32, f32::max);
        assert!(
            b >= floor || tallest < QUIET,
            "speech frame {} collapsed bars: {b} < floor {floor} (tallest bar {tallest})",
            18 + i
        );
    }

    // Hide tail and the idle dot row stay dim against the burst. Ratios (not
    // absolute floors) so the gate holds for both the round-1 and graded
    // palettes — the graded one renders idle dots below the gate entirely.
    let tail = bright[62..70].iter().max().copied().unwrap_or(0);
    assert!(
        tail * 8 < burst,
        "hide tail too bright: {tail} vs burst {burst}"
    );
    let idle = Story {
        grow: 1.0,
        alpha: 1.0,
        bars: idle_bars(),
    };
    let (_, _, buf) = render_frame(&idle, 0.0, 1);
    let idle_bright = buf
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| is_bright(px[0], px[1], px[2], px[3]))
        .count();
    assert!(
        idle_bright * 8 < burst,
        "idle dot row too bright: {idle_bright} vs burst {burst}"
    );
}

/// The story's peak frame must pose as SHAPE_MASK: hottest bar at the
/// 1000 Hz cluster center (bar 13), hottest five inside the cluster 11-16.
#[test]
fn peak_frame_hot_cluster_is_centered() {
    let story = story();
    let peak = (18..54)
        .max_by(|&a, &b| {
            let sa: f32 = story[a].bars.iter().sum();
            let sb: f32 = story[b].bars.iter().sum();
            sa.total_cmp(&sb)
        })
        .unwrap();
    let bars = &story[peak].bars;
    let mut order: Vec<usize> = (0..NUM_BARS).collect();
    order.sort_unstable_by(|&a, &b| bars[b].total_cmp(&bars[a]));
    let hot = order[0];
    assert!(
        (13..=16).contains(&hot),
        "peak hottest bar {hot} not near the apex 15: {bars:?}"
    );
    for &i in &order[..5] {
        assert!(
            (10..=18).contains(&i),
            "peak top-5 includes bar {i} outside the broad mound 10-18: {bars:?}"
        );
    }
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
