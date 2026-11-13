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
    let _ = (canvas, width, height, bars, t_secs);
    unimplemented!("subagent C: rasterize one pill frame")
}
