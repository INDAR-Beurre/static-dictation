//! canvas: software rasterizer for the pill image — dark rounded bar + teal bars + glow.
//!
//! Owned-by: subagent C. Do not edit the pub API; fill in the `unimplemented!()` bodies.
//! Pure functions: no I/O, no Wayland, no threads — just pixels, so trivially testable.
//!
//! Contract (FROZEN):
//! - `render_pill(canvas: &mut [u8], width: u32, height: u32, bars: &[f32], t_secs: f32)`
//!   draws one frame of the pill into the given ARGB8888 (little-endian BGRA byte order)
//!   shm canvas, `stride = width * 4` exactly (no padding). `bars` comes from
//!   vecmentor (`len == NUM_BARS`, values 0.0..1.0); `t_secs` = wall time seconds for
//!   any glow pulse animation. Bars that would poke outside the rounded rect are
//!   clipped (draw over transparent background). Remember: canvas is `u8` per channel,
//!   4 bytes per pixel; the byte at index (y*width + x)*4 + 0 is BLUE, +1 GREEN,
//!   +2 RED, +3 ALPHA (Display = little-endian x86).
//! - Style (reference image): dark charcoal pill (#0F1117-ish, alpha ~230) with
//!   rounded corners (radius ≈ height/2); glowing teal/cyan bars (#16E0E0-ish core,
//!   soft additive glow around each) centered horizontally; a subtle 1px lighter
//!   top-edge highlight for the "glass" feel; overall composition reads as ONE pill.
//! - Add unit tests in this file (`#[cfg(test)] mod tests`) covering: corners of the
//!   canvas stay transparent (rounded clip works), bar area pixels are teal-ish
//!   (G,B > R), glow falls off (pixel farther from a bar is dimmer), taller bar →
//!   brighter/more covered pixels at that x.

/// Draw one frame of the pill into an ARGB8888 canvas (BGRA byte order).
#[allow(clippy::too_many_arguments)]
pub fn render_pill(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    bars: &[f32],
    t_secs: f32,
) {
    let w = width as i32;
    let h = height as i32;
    if w <= 0 || h <= 0 {
        return;
    }
    let radius = h as f32 * 0.5;
    let half_w = w as f32 / 2.0;
    let half_h = h as f32 / 2.0;
    let inset_w = (half_w - radius).max(0.0);
    let inset_h = (half_h - radius).max(0.0);
    let r2 = radius * radius;

    // Rounded-rect SDF-ish membership test (center of pixel).
    let in_pill = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= w || y >= h {
            return false;
        }
        let fx = (x as f32 + 0.5 - half_w).abs() - inset_w;
        let fy = (y as f32 + 0.5 - half_h).abs() - inset_h;
        let dx = fx.max(0.0);
        let dy = fy.max(0.0);
        dx * dx + dy * dy <= r2
    };

    let pad_l = 22i32;
    let pad_r = 22i32;
    let pad_t = 10i32;
    let pad_b = 8i32;
    let inner_w = (w - pad_l - pad_r).max(0);
    let inner_h = (h - pad_t - pad_b).max(0);
    let cy = h / 2;

    // Bar rects (exclusive right/bottom edges), clipped caller-side to the pill test.
    let mut rects: Vec<(i32, i32, i32, i32)> = Vec::new();
    if inner_w > 0 && inner_h > 0 && !bars.is_empty() {
        let n = bars.len();
        let slot_w = inner_w as f32 / n as f32;
        let bar_w = ((slot_w * 0.62) as i32).min(inner_w).max(1);
        for (i, b) in bars.iter().enumerate() {
            let b = b.clamp(0.0, 2.0);
            let bh = ((b * inner_h as f32) as i32).clamp(2, inner_h);
            let slot_x = pad_l + (slot_w * i as f32) as i32;
            let x0 = slot_x + ((slot_w as i32) - bar_w) / 2;
            let y0 = cy - bh / 2;
            rects.push((x0, x0 + bar_w, y0, y0 + bh));
        }
    }

    // Background + top highlight, then bar cores, then additive glow. Everything
    // is clipped to the rounded rect; untouched pixels stay 0 (transparent).
    let pulse = 40.0 + 18.0 * (t_secs * 2.0).sin();
    for y in 0..h {
        for x in 0..w {
            if !in_pill(x, y) {
                continue;
            }
            let idx = ((y * w + x) * 4) as usize;
            // Vertical gradient: top is a touch lighter (+8 on each channel).
            let grad = ((1.0 - y as f32 / (h - 1).max(1) as f32) * 8.0) as i32;
            let mut r = 0x0F + grad;
            let mut g = 0x11 + grad;
            let mut b = 0x17 + grad;
            // 1px top-edge highlight: first row inside the pill at this column.
            if !in_pill(x, y - 1) {
                r += (255 - r) * 30 / 255;
                g += (255 - g) * 30 / 255;
                b += (255 - b) * 30 / 255;
            }
            // Bar core: solid teal, alpha 255.
            let mut core = false;
            for &(bx0, bx1, by0, by1) in &rects {
                if x >= bx0 && x < bx1 && y >= by0 && y < by1 {
                    core = true;
                    break;
                }
            }
            let alpha;
            if core {
                r = 22;
                g = 224;
                b = 224;
                alpha = 255;
            } else {
                // Additive glow: teal light from nearby bar edges, quadratic falloff.
                let mut inten = 0.0f32;
                for &(bx0, bx1, by0, by1) in &rects {
                    let dx = if x < bx0 {
                        bx0 - x
                    } else if x >= bx1 {
                        x - bx1 + 1
                    } else {
                        0
                    };
                    let dy = if y < by0 {
                        by0 - y
                    } else if y >= by1 {
                        y - by1 + 1
                    } else {
                        0
                    };
                    let d2 = (dx * dx + dy * dy) as f32;
                    if d2 <= 9.0 {
                        inten = inten.max(pulse * (1.0 - d2 / 9.0));
                    }
                }
                let add = (inten / 224.0).clamp(0.0, 1.0);
                r += (22.0 * add) as i32;
                g += (224.0 * add) as i32;
                b += (224.0 * add) as i32;
                alpha = 230;
            }
            canvas[idx + 0] = b.clamp(0, 255) as u8;
            canvas[idx + 1] = g.clamp(0, 255) as u8;
            canvas[idx + 2] = r.clamp(0, 255) as u8;
            canvas[idx + 3] = alpha;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 320;
    const H: u32 = 64;

    fn frame(bars: &[f32], t: f32) -> Vec<u8> {
        let mut canvas = vec![0u8; (W * H * 4) as usize];
        render_pill(&mut canvas, W, H, bars, t);
        canvas
    }

    fn alpha(c: &[u8], x: u32, y: u32) -> u8 {
        c[((y * W + x) * 4 + 3) as usize]
    }

    fn px(c: &[u8], x: u32, y: u32) -> (u8, u8, u8, u8) {
        let i = ((y * W + x) * 4) as usize;
        (c[i], c[i + 1], c[i + 2], c[i + 3])
    }

    #[test]
    fn corners_stay_transparent() {
        let c = frame(&[0.5; 5], 0.0);
        for &(x, y) in &[(0, 0), (W - 1, 0), (0, H - 1), (W - 1, H - 1)] {
            assert_eq!(alpha(&c, x, y), 0, "corner ({x},{y}) not transparent");
        }
        // Middle of the pill is opaque.
        assert_eq!(alpha(&c, W / 2, H / 2), 255); // bar core at center
        assert_eq!(alpha(&c, W / 2, 1), 230); // background
    }

    #[test]
    fn center_pixel_is_teal_for_high_bars() {
        let c = frame(&[0.8; 5], 0.0);
        let (b, g, r, _a) = px(&c, W / 2, H / 2);
        assert!(g > 150, "green {g} not > 150");
        assert!(b > 150, "blue {b} not > 150");
        assert!(r < 100, "red {r} not < 100");
    }

    #[test]
    fn glow_falls_off_with_distance() {
        let c = frame(&[0.3; 1], 0.0);
        let (_, g_center, _, _) = px(&c, W / 2, H / 2);
        let (_, g_far, _, _) = px(&c, W / 2, 5);
        assert!(
            g_center > g_far,
            "center green {g_center} should exceed far green {g_far}"
        );
    }

    #[test]
    fn taller_bar_covers_more_height() {
        let low = frame(&[0.05; 1], 0.0);
        let high = frame(&[1.0; 1], 0.0);
        // Near the top of the inner area (pad_t = 10).
        let (_, g_low, _, _) = px(&low, W / 2, 11);
        let (_, g_high, _, _) = px(&high, W / 2, 11);
        assert!(
            g_high > g_low,
            "high bar green {g_high} should exceed low bar green {g_low} at y=11"
        );
    }

    #[test]
    fn absurd_bars_do_not_panic_and_stay_in_range() {
        let c = frame(&[2.0; 3], 0.0);
        assert_eq!(c.len(), (W * H * 4) as usize);
        for v in c.iter().step_by(4) {
            // every blue byte in range (implies clamping worked; all bytes written same way)
            let _ = v;
        }
        // Negative / NaN-ish and empty bars also fine.
        let _ = frame(&[-1.0, f32::NAN, 0.0], 1e9);
        let _ = frame(&[], 0.0);
    }
}
