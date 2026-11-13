//! canvas: software rasterizer for the pill overlay — SDF anti-aliased dark rounded
//! pill with symmetric teal equalizer capsules, soft drop shadow and additive glow.
//!
//! Style (macOS-dictation look): dark charcoal stadium-shaped pill (#10131A, interior
//! alpha ≈235) centered in the surface with 24px side / 20px top+bottom padding
//! (logical surface 368×104 → pill core 320×64); vertical glass gradient (top
//! slightly lighter), a 1–2px lighter inner highlight along the top inner edge, and
//! a subtle 1px white outer stroke (alpha 0.08). Nine (or any count of) teal
//! capsules (#16E0E0, slightly brighter center lane) sit in a centered row and grow
//! symmetrically up/down from the row center; each carries an additive teal bloom
//! (exp falloff, ~8px kernel, ~14px reach) masked to the pill interior, plus a faint
//! exterior halo (≤10% strength, ~10px reach) so the edge reads lit on dark
//! wallpapers. A soft dark drop shadow (peak alpha ≈90, offset +8px, ~16px
//! quadratic falloff) sits under everything. Every edge is anti-aliased with
//! signed-distance-field smoothstep over a ±1.25px band, and computed float values
//! are Bayer-dithered (±1 LSB) before quantization so the shadow/glow gradients
//! don't band.
//!
//! Contract (FROZEN):
//! - `render_pill(canvas: &mut [u8], width: u32, height: u32, bars: &[f32],
//!   t_secs: f32, master_alpha: f32)` draws one frame into a BGRA (little-endian
//!   ARGB8888) shm canvas, `stride = width * 4` exactly. `width`/`height` are the
//!   BUFFER pixel size (logical 368×104 × compositor scale) — geometry is derived
//!   from them, never assumed. `bars`: symmetric equalizer heights 0.0..1.0 (any
//!   length, may be empty; non-finite values are treated as 0). `t_secs`: wall time
//!   seconds driving a small sin-based glow pulse. `master_alpha`: global fade
//!   0.0..1.0 multiplied into EVERY output alpha (colors are written premultiplied,
//!   so they fade with it). Untouched bytes stay 0. Byte +0 = Blue, +1 = Green,
//!   +2 = Red, +3 = Alpha.
//! - Pure functions: no I/O, no threads, std-only; no allocation inside the pixel
//!   loop (bar capsules are precomputed once per frame).

/// Logical surface width (pill core 320 + 24px pad each side).
pub const SURF_W: u32 = 368;
/// Logical surface height (pill core 64 + 20px pad top/bottom).
pub const SURF_H: u32 = 104;

const PAD_X: f32 = 24.0;
const PAD_Y: f32 = 20.0;
const BODY_A: f32 = 235.0;
const SHADOW_A: f32 = 90.0;
const SHADOW_OFF: f32 = 8.0;
const SHADOW_SPAN: f32 = 16.0;
const AA_BAND: f32 = 1.25;
const BAR_W_FRAC: f32 = 0.62; // bar width as fraction of its slot
const GLOW_K: f32 = 8.0; // exp kernel width, logical px
const GLOW_REACH: f32 = 14.0; // glow cut-off distance, logical px

/// 4×4 Bayer matrix, normalized to [-0.4375, 0.4375].
const BAYER: [f32; 16] = [
    -0.4375, 0.0625, -0.3125, 0.1875, //
    0.3125, -0.1875, 0.4375, -0.0625, //
    -0.1875, 0.3125, -0.0625, 0.4375, //
    0.4375, -0.0625, 0.3125, -0.1875,
];

#[inline]
fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[inline]
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Anti-aliased coverage from an SDF: 1 fully inside, 0 fully outside,
/// smoothstep across the ±1.25px edge band.
#[inline]
fn sdf_cov(d: f32) -> f32 {
    smooth(clamp01(0.5 - d / (2.0 * AA_BAND)))
}

/// Signed distance to a rounded rect (stadium when r == hh), centered at (cx, cy).
#[inline]
fn sd_rounded_rect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let r = r.min(hw).min(hh);
    let qx = (px - cx).abs() - (hw - r);
    let qy = (py - cy).abs() - (hh - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r
}

/// Signed distance to a vertical capsule: segment (cx, cy-seg)..(cx, cy+seg), radius r.
#[inline]
fn sd_capsule(px: f32, py: f32, cx: f32, cy: f32, seg: f32, r: f32) -> f32 {
    let t = if seg > 0.0 {
        clamp01((py - (cy - seg)) / (2.0 * seg))
    } else {
        0.5
    };
    let dy = py - (cy - seg + t * 2.0 * seg);
    ((px - cx) * (px - cx) + dy * dy).sqrt() - r
}

/// Straight-alpha "src over dst" on (r, g, b, a) floats.
#[inline]
#[allow(clippy::too_many_arguments)] // rgba tuples; a struct would not be clearer
fn over(r: f32, g: f32, b: f32, a: f32, sr: f32, sg: f32, sb: f32, sa: f32) -> (f32, f32, f32, f32) {
    let sa01 = sa / 255.0;
    let ia = 1.0 - sa01;
    (
        sr * sa01 + r * ia,
        sg * sa01 + g * ia,
        sb * sa01 + b * ia,
        sa + a * ia,
    )
}

/// Quantize a float channel with proportional Bayer dither (±1 LSB) so large
/// gradients (shadow, glow) don't band. Zero stays exactly zero.
#[inline]
fn q(v: f32, dith: f32) -> u8 {
    if v <= 0.0 {
        0
    } else {
        (v + dith * (v / 255.0)).round().min(255.0) as u8
    }
}

struct Bar {
    cx: f32,
    hw: f32,  // half width
    r: f32,   // capsule radius (min(half width, half height))
    seg: f32, // half length of the inner segment (half_h - r, >= 0)
    // Inclusive-exclusive pixel bounds covering core + glow reach (fast path).
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
}

/// Draw one frame of the pill into a BGRA canvas (see module docs for the contract).
pub fn render_pill(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    bars: &[f32],
    t_secs: f32,
    master_alpha: f32,
) {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || canvas.len() < w * h * 4 {
        return;
    }
    let s = width as f32 / SURF_W as f32; // compositor scale, derived — never assumed
    let t = if t_secs.is_finite() { t_secs } else { 0.0 };
    let ma = if master_alpha.is_finite() {
        master_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let pulse = 1.0 + 0.10 * (t * 2.4).sin();

    let cx = width as f32 * 0.5;
    let cy = height as f32 * 0.5;
    let hw = (width as f32 * 0.5 - PAD_X * s).max(1.0);
    let hh = (height as f32 * 0.5 - PAD_Y * s).max(1.0);
    let rad = hh; // stadium: radius = core height / 2
    let reach = GLOW_REACH * s;

    // Precompute bar capsules once per frame — nothing allocates in the pixel loop.
    let mut caps: Vec<Bar> = Vec::with_capacity(bars.len());
    if !bars.is_empty() {
        let n = bars.len() as f32;
        let slot = (2.0 * hw) / n;
        let bar_hw = (slot * BAR_W_FRAC * 0.5).max(0.4);
        // Bars keep a little breathing room from the pill's top/bottom edge.
        let inner_h = (2.0 * hh - 24.0 * s).max(bar_hw * 2.0);
        for (i, &v) in bars.iter().enumerate() {
            let v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
            let half_h = (v * inner_h * 0.5).max(1.0);
            let r = bar_hw.min(half_h);
            let bcx = cx - hw + slot * (i as f32 + 0.5);
            caps.push(Bar {
                cx: bcx,
                hw: bar_hw,
                r,
                seg: (half_h - r).max(0.0),
                x0: ((bcx - bar_hw - reach).floor() as i32).max(0),
                x1: ((bcx + bar_hw + reach).ceil() as i32).min(width as i32),
                y0: ((cy - half_h - reach).floor() as i32).max(0),
                y1: ((cy + half_h + reach).ceil() as i32).min(height as i32),
            });
        }
    }

    for y in 0..h {
        let py = y as f32 + 0.5;
        let dy_top = py - (cy - hh); // 0 at the pill's top edge
        for x in 0..w {
            let px = x as f32 + 0.5;
            let d = sd_rounded_rect(px, py, cx, cy, hw, hh, rad);

            // Drop shadow: pill silhouette shifted +8px, quadratic falloff over ~16px.
            let ds = sd_rounded_rect(px, py - SHADOW_OFF * s, cx, cy, hw, hh, rad);
            let st = clamp01(1.0 - ds.max(0.0) / (SHADOW_SPAN * s));
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = smooth(st) * SHADOW_A;

            // 1px outer stroke rgba(255,255,255,0.08), straddling the body edge.
            let sb = clamp01(1.0 - (d - 0.9 * s).abs() / (0.8 * s));
            if sb > 0.0 {
                (r, g, b, a) = over(r, g, b, a, 255.0, 255.0, 255.0, sb * 20.0);
            }

            // Pill body: vertical glass gradient + inner top-edge highlight.
            let cov = sdf_cov(d);
            if cov > 0.0 {
                let gy = clamp01(dy_top / (2.0 * hh));
                let light = (1.0 - gy) * 9.0; // top slightly lighter
                let mut br = 16.0 + light;
                let mut bg = 19.0 + light;
                let mut bb = 26.0 + light;
                // Highlight band hugs 0.5..2.7px inside the edge, fading out by
                // ~70% down the pill so it reads as a top light source.
                let band = clamp01(1.0 - (d + 1.6 * s).abs() / (1.1 * s));
                let tf = clamp01(1.0 - dy_top / (hh * 0.7));
                let hi = band * tf * 45.0;
                br += hi;
                bg += hi;
                bb += hi;
                (r, g, b, a) = over(r, g, b, a, br, bg, bb, BODY_A * cov);
            }

            // Bar cores (teal capsules) + accumulated glow intensity.
            let mut glow = 0.0f32;
            for bar in &caps {
                let (xi, yi) = (x as i32, y as i32);
                if xi < bar.x0 || xi >= bar.x1 || yi < bar.y0 || yi >= bar.y1 {
                    continue; // fast path: outside this bar's precomputed range
                }
                let db = sd_capsule(px, py, bar.cx, cy, bar.seg, bar.r);
                let bcov = sdf_cov(db);
                if bcov > 0.0 {
                    // Slightly brighter center lane across the capsule width.
                    let lane = clamp01(1.0 - (px - bar.cx).abs() / bar.hw);
                    let lift = 24.0 * lane;
                    (r, g, b, a) = over(r, g, b, a, 22.0, 224.0 + lift, 224.0 + lift, 255.0 * bcov);
                }
                if db < reach {
                    glow += (-db.max(0.0) / (GLOW_K * s)).exp();
                }
            }

            // Additive teal bloom, masked to the pill interior.
            if glow > 0.0 {
                let gi = glow * pulse * cov;
                let k = 0.85 * gi;
                r += 22.0 * k;
                g += 224.0 * k;
                b += 224.0 * k;
                a += 40.0 * gi;
            }

            // Faint exterior halo: the pill edge feels lit on dark wallpapers.
            if d > 0.0 && cov < 1.0 {
                let halo = 0.10 * (-d / (3.5 * s)).exp() * (1.0 - cov);
                r += 22.0 * halo;
                g += 224.0 * halo;
                b += 224.0 * halo;
                a += halo * 255.0;
            }

            // Global fade, premultiply (Wayland ARGB8888 expects premultiplied),
            // dither, quantize. Clamp a to 255 BEFORE the fade so master_alpha
            // halves the same stored value everywhere — otherwise saturated glow
            // pixels (a > 255) would end up >half at ma = 0.5.
            a = (a.min(255.0)) * ma;
            let pa = a / 255.0;
            let dith = BAYER[((y & 3) << 2) | (x & 3)];
            let idx = (y * w + x) * 4;
            canvas[idx] = q(b * pa, dith);
            canvas[idx + 1] = q(g * pa, dith);
            canvas[idx + 2] = q(r * pa, dith);
            canvas[idx + 3] = q(a, dith);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = SURF_W;
    const H: u32 = SURF_H;

    fn frame(bars: &[f32], t: f32, ma: f32) -> Vec<u8> {
        let mut c = vec![0u8; (W * H * 4) as usize];
        render_pill(&mut c, W, H, bars, t, ma);
        c
    }

    fn alpha(c: &[u8], x: u32, y: u32) -> u8 {
        c[((y * W + x) * 4 + 3) as usize]
    }

    fn chan(c: &[u8], x: u32, y: u32, ch: u32) -> u8 {
        c[((y * W + x) * 4 + ch) as usize]
    }

    // 9 bars: middle bar (index 4) is centered exactly at x = 184 at scale 1.
    const MID_X: u32 = 184;

    #[test]
    fn corners_stay_transparent() {
        let c = frame(&[0.5; 9], 0.0, 1.0);
        for &(x, y) in &[(0, 0), (W - 1, 0), (0, H - 1), (W - 1, H - 1)] {
            let i = ((y * W + x) * 4) as usize;
            assert_eq!(c[i], 0, "corner ({x},{y}) blue not 0");
            assert_eq!(c[i + 3], 0, "corner ({x},{y}) alpha not 0");
        }
        // The pill interior is essentially opaque.
        assert!(alpha(&c, W / 2, H / 2) > 200);
    }

    #[test]
    fn shadow_outside_core_is_soft_and_under_interior() {
        let c = frame(&[0.5; 9], 0.0, 1.0);
        // ~4px below the pill core bottom edge (core bottom at y = 84).
        let near = alpha(&c, MID_X, 88);
        let far = alpha(&c, MID_X, 96);
        assert!(near > 0, "shadow missing just below the pill");
        assert!(near < 235, "shadow {near} must be softer than interior 235");
        assert!(far < near, "shadow must fall off with distance: {far} >= {near}");
    }

    #[test]
    fn glow_falls_off_with_distance() {
        // Bar 4 at value 0.5: core half-height 20 logical px (scale 1), so y=40 and
        // y=34 are both above the core but inside the pill, in the glow field.
        let c = frame(&[0.5; 9], 0.0, 1.0);
        let g_near = chan(&c, MID_X, 40, 1); // green, ~1.5px past the capsule end
        let g_far = chan(&c, MID_X, 34, 1); // green, ~7.5px past the capsule end
        assert!(
            g_near > g_far + 20,
            "glow must decay: near {g_near} vs far {g_far}"
        );
    }

    #[test]
    fn taller_bar_covers_more_pixels_at_same_x() {
        let low = frame(&[0.3; 9], 0.0, 1.0);
        let high = frame(&[0.9; 9], 0.0, 1.0);
        let count = |c: &[u8]| -> u32 {
            let mut n = 0;
            for y in 0..H {
                // Teal core: strong green, almost no red (glow peaks below this).
                if chan(c, MID_X, y, 1) > 210 && chan(c, MID_X, y, 2) < 60 {
                    n += 1;
                }
            }
            n
        };
        let cl = count(&low);
        let ch = count(&high);
        assert!(ch > cl + 10, "taller bar must cover more rows: {ch} vs {cl}");
    }

    #[test]
    fn master_alpha_half_halves_alpha() {
        let full = frame(&[0.6; 9], 0.0, 1.0);
        let half = frame(&[0.6; 9], 0.0, 0.5);
        for y in 0..H {
            for x in 0..W {
                let a1 = alpha(&full, x, y) as i32;
                let a2 = alpha(&half, x, y) as i32;
                assert!(
                    (a2 - a1 / 2).abs() <= 3,
                    "alpha ({x},{y}): {a2} not ~half of {a1}"
                );
            }
        }
    }

    #[test]
    fn master_alpha_zero_writes_all_zero() {
        let c = frame(&[0.8; 9], 3.0, 0.0);
        assert!(c.iter().all(|&b| b == 0), "master_alpha 0 must blank the frame");
    }

    #[test]
    fn absurd_inputs_do_not_panic_and_stay_in_range() {
        // Non-finite bars / time / alpha, plus a too-small canvas: none may panic.
        let mut tiny = [0u8; 10];
        render_pill(&mut tiny, W, H, &[f32::NAN, 2.0, f32::NEG_INFINITY], 0.0, 1.0);
        let c = frame(&[f32::NAN, 2.0, -1.0, f32::INFINITY, 0.0], f32::NAN, f32::NAN);
        assert!(c.iter().all(|&b| b == 0), "NaN master_alpha must blank the frame");
        let c = frame(&[f32::NAN, 2.0, -1.0, f32::INFINITY, 0.0], 1e9, 1.0);
        // Glow-saturated pixels must have been clamped, not wrapped: a core pixel
        // must be full teal and fully opaque.
        let (b, g, r, a) = (
            chan(&c, MID_X, 52, 0),
            chan(&c, MID_X, 52, 1),
            chan(&c, MID_X, 52, 2),
            chan(&c, MID_X, 52, 3),
        );
        assert_eq!(a, 255);
        assert!(g > 200 && b > 200 && r < 60, "core pixel not teal: {r},{g},{b}");
        // Empty bars: body and shadow still render, corners stay clean.
        let c = frame(&[], 0.0, 1.0);
        assert!(alpha(&c, W / 2, H / 2) > 200);
        assert_eq!(alpha(&c, 0, 0), 0);
    }

    #[test]
    fn scale_two_buffer_matches_logical_frame() {
        let mut small = vec![0u8; (SURF_W * SURF_H * 4) as usize];
        render_pill(&mut small, SURF_W, SURF_H, &[0.5; 9], 0.0, 1.0);
        let mut big = vec![0u8; (SURF_W * 2 * SURF_H * 2 * 4) as usize];
        render_pill(&mut big, SURF_W * 2, SURF_H * 2, &[0.5; 9], 0.0, 1.0);
        let a_big = |x: u32, y: u32| big[((y * SURF_W * 2 + x) * 4 + 3) as usize];
        // Interior core, body-only and shadow probes must agree within AA noise.
        let probes = [(184, 52, 368, 104), (184, 30, 368, 60), (184, 88, 368, 176)];
        for &(xs, ys, xb, yb) in &probes {
            let a1 = alpha(&small, xs, ys) as i32;
            let a2 = a_big(xb, yb) as i32;
            assert!(
                (a2 - a1).abs() <= 12,
                "scale mismatch at logical ({xs},{ys}): {a1} vs {a2}"
            );
        }
    }
}
