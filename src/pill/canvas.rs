//! canvas: software rasterizer for the pill overlay — SDF anti-aliased
//! near-black pill with a teal→cyan→blue equalizer wave, soft bloom and a
//! below-only drop shadow.
//!
//! Style (measured from an iPhone Dynamic-Island reference): a near-black
//! stadium-shaped pill with a green-gray cast (rgb 35,38,37) centered in the
//! surface with 24px side / 20px top+bottom padding (logical surface 368×104 →
//! pill core 320×64). A subtle +10 RGB sheen crowns the body's top-left and
//! fades darker rightward; there is deliberately no rim light and no hairline —
//! just a soft SDF anti-aliased edge. The waveform is ONE element kind: a
//! vertical capsule with fully rounded caps (r = w/2) that reads as a ~2px dim
//! dot at idle and grows into a ~5px-wide bar; a gentle center-weight envelope
//! tapers element heights toward the edges so silence reads as a centered
//! pyramid of dots, and peak brightness scales with element height (never
//! invisible at idle). Brightness also ramps down toward both row ends —
//! smoothstep from the mid-row out to ×(1−EDGE_DIM) at the outermost elements
//! — so the flanks read as dim tail dots under the hot center cluster.
//! Element color interpolates along x — teal
//! rgb(125,165,165) at the left, peak cyan rgb(140,220,222) at the center, blue
//! rgb(100,130,170) at the right — and the flanking dotted leader lines pick up
//! the same gradient at ~24% alpha. The grown (grow=1) capsule draws at
//! 260×64 logical — the 676×167 reference capsule's aspect 4.05 rescaled into
//! the 368×104 surface (the 320×64 core bounds the overshoot bulge). A soft
//! additive bloom in each element's own color (≈7px logical kernel — 15px at
//! the 676px reference scale, faint to ~28px, strongest over the tall center cluster,
//! pulsing slowly with `t_secs`) bridges neighboring bars; it is masked to the
//! pill interior so the frame outside stays clean. A 25% black
//! drop shadow sits below the capsule only (offset +6px, ~18px falloff, masked
//! below the capsule midline so the sides stay clean).
//!
//! Entrance (iPhone Dynamic-Island style): the caller drives a spring via
//! `grow`. The capsule's width leads and its height lags (squash-stretch), its
//! center rides from the collapsed clock-pill position (29/104 of the height)
//! up to its rest position hugging the top edge (52/104), and overshoot above
//! 1.0 bulges it wider and lets it dip below rest before settling. At grow 0
//! the twin is exactly the real clock pill — 76×24 logical (half-extents
//! 38/12), flat #1F1E1D — and blends into the capsule bg as grow leaves the
//! 0.15 flat zone. The waveform and leader dots fade in between grow 0.25 and
//! 0.7. Every edge is anti-aliased with signed-distance-field smoothstep over
//! a ±1.25px band, and computed float values are Bayer-dithered (±1 LSB)
//! before quantization so the shadow/glow gradients don't band.
//!
//! Contract (FROZEN):
//! - `render_pill(canvas: &mut [u8], width: u32, height: u32, bars: &[f32],
//!   t_secs: f32, master_alpha: f32, grow: f32)` draws one frame into a BGRA
//!   (little-endian ARGB8888) shm canvas, `stride = width * 4` exactly.
//!   `width`/`height` are the BUFFER pixel size (logical 368×104 × compositor
//!   scale) — geometry is derived from them, never assumed. `bars`: symmetric
//!   equalizer heights 0.0..1.0 (any length, may be empty; non-finite values are
//!   treated as 0). `t_secs`: wall time seconds driving a small sin-based glow
//!   pulse. `master_alpha`: global fade 0.0..1.0 multiplied into EVERY output
//!   alpha (colors are written premultiplied, so they fade with it). `grow`:
//!   entrance spring — 0.0 collapsed clock-pill twin (no waveform), 1.0 full
//!   pill, overshoot up to 1.25 bulges wider and dips lower; clamped to
//!   [0.0, 1.25], non-finite treated as 0.0. Untouched bytes stay 0. Byte +0 =
//!   Blue, +1 = Green, +2 = Red, +3 = Alpha.
//! - Pure functions: no I/O, no threads, std-only; no allocation inside the pixel
//!   loop (bar capsules and leader dots are precomputed once per frame).

use std::f32::consts::PI;

/// Logical surface width (pill core 320 + 24px pad each side).
pub const SURF_W: u32 = 368;
/// Logical surface height (pill core 64 + 20px pad top/bottom).
pub const SURF_H: u32 = 104;
pub const GROW_MAX: f32 = 1.25; // spring overshoot ceiling: canvas clamp + tsar/preview springs

const PAD_X: f32 = 24.0;
const PAD_Y: f32 = 20.0;
const BODY_A: f32 = 255.0; // opaque: stored RGB == palette RGB
const SHADOW_A: f32 = 64.0; // 25% black drop shadow
const SHADOW_OFF: f32 = 6.0; // shadow offset down, logical px
const SHADOW_SPAN: f32 = 18.0; // shadow falloff distance, logical px
const AA_BAND: f32 = 1.25;
const BAR_SPAN_FRAC: f32 = 0.68; // waveform occupies the central span of the pill width
const DOT_R: f32 = 1.2; // leader-dot radius, logical px (≈2.4px diameter)
const DOT_STEP: f32 = 5.5; // leader-dot spacing, logical px
const DOT_A: f32 = 60.0; // leader-dot peak alpha (≈24% — spec tail ≈20–25% of peak)
// Leader-zone dimming: brightness falls off toward both ends of the row so the
// island reads as a lit center cluster with dim tail dots (reference: tail ramp
// ≈35/255 ≈ bright-step of the peak). Measured effect at idle: outermost leader
// dots land at ≈25% of the center dot's bg-excess (raw-channel ratio ≈0.36,
// spec tail 0.10 relative to cluster 1.00 + body ≈42/255) — the multiplier is
// per-channel color, never below zero.
const EDGE_DIM: f32 = 0.215; // end-of-row brightness floor: 1.0 - EDGE_DIM at |u-0.5| = 0.5
const EDGE_DIM_INNER: f32 = 0.30; // falloff starts this far from the row center (of the half-row)
const GLOW_K: f32 = 7.0; // bloom exp kernel, logical px — spec halo r≈15px at the
// 676px reference width rescales to a measured half-excess of ≈7px logical
// (≈14px @2x) past a bar's core edge (K·ln2 plus the core-AA lift); at the
// 27-bar pitch (≈8px logical) the kernel stays under one slot so gaps read.
const GLOW_REACH: f32 = 28.0; // faint bloom halo cut-off (~25–30px); at K=7
// the cutoff step is ≈2% of the edge value — dithered away.
const BLOOM: f32 = 0.85; // additive bloom strength
// Bloom alpha gain: must stay ≥ max gradient channel (220) × BLOOM so the
// additive step can never push a color channel above alpha (premultiplied).
const GLOW_A: f32 = 197.0;
// 27 bars at pitch 2·0.68·160/27 ≈ 8.06 logical px (reference pitch ≈14.5px
// at the 676px-wide reference = 0.68 of its width) → ~5px bar width max.
const ELEM_R_MAX: f32 = 2.5; // element half-width max → ~5px bar width, logical px
const DOT_R_MIN: f32 = 1.3; // idle element radius floor → ~2.6px dot diameter
const SHEEN: f32 = 10.0; // top-left body sheen, +RGB

/// Measured palette (verbatim from the reference spec).
const BG: (f32, f32, f32) = (35.0, 38.0, 37.0); // near-black green-gray cast
const TWIN: (f32, f32, f32) = (31.0, 30.0, 29.0); // #1F1E1D clock-pill twin
const GRAD_L: (f32, f32, f32) = (125.0, 165.0, 165.0); // teal, left end
const GRAD_C: (f32, f32, f32) = (140.0, 220.0, 222.0); // peak cyan, center
const GRAD_R: (f32, f32, f32) = (100.0, 130.0, 170.0); // blue, right end

/// Leader-zone dimming: 1.0 over the inner 30% of the half-row (the hot
/// cluster stays undimmed), smoothstep down to 1.0 - EDGE_DIM at the
/// outermost elements — a smooth tail, no hard cliff. u: 0 left → 1 right.
#[inline]
fn edge_dim(u: f32) -> f32 {
    let t = clamp01(((u - 0.5).abs() - EDGE_DIM_INNER) / (0.5 - EDGE_DIM_INNER));
    1.0 - EDGE_DIM * smooth(t)
}

/// Wave gradient along the span (u: 0 left → 1 right): teal → peak cyan → blue.
#[inline]
fn wave_color(u: f32) -> (f32, f32, f32) {
    let u = clamp01(u);
    if u <= 0.5 {
        let t = u * 2.0;
        (
            GRAD_L.0 + (GRAD_C.0 - GRAD_L.0) * t,
            GRAD_L.1 + (GRAD_C.1 - GRAD_L.1) * t,
            GRAD_L.2 + (GRAD_C.2 - GRAD_L.2) * t,
        )
    } else {
        let t = (u - 0.5) * 2.0;
        (
            GRAD_C.0 + (GRAD_R.0 - GRAD_C.0) * t,
            GRAD_C.1 + (GRAD_R.1 - GRAD_C.1) * t,
            GRAD_C.2 + (GRAD_R.2 - GRAD_C.2) * t,
        )
    }
}

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
fn over(
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    sr: f32,
    sg: f32,
    sb: f32,
    sa: f32,
) -> (f32, f32, f32, f32) {
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
    r: f32,   // capsule radius = element half-width (a dot when seg == 0)
    seg: f32, // half length of the inner segment (half_h - r, >= 0)
    gw: f32,  // bloom weight, scales with bar height
    cr: f32,  // gradient color at this element's x position,
    cg: f32,  // pre-scaled by height brightness and shimmer
    cb: f32,
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
    grow: f32,
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

    // Entrance spring (FROZEN contract): 0.0 collapsed clock-pill twin, 1.0
    // full pill, up to 1.25 overshoot. Width leads, height lags: squash-stretch.
    let gr = if grow.is_finite() { grow.clamp(0.0, 1.25) } else { 0.0 };
    let width_ease = if gr <= 1.0 { gr } else { 1.0 + (gr - 1.0) * 0.35 };
    // Waveform fades in mid-bounce: 0 below grow 0.25, 1 from grow 0.7.
    let wave_op = smooth(clamp01((gr - 0.25) / 0.45));

    let cx = width as f32 * 0.5;
    let hw = (width as f32 * 0.5 - PAD_X * s).max(1.0);
    let hh = (height as f32 * 0.5 - PAD_Y * s).max(1.0);
    // grow=0 must BE the clock pill: 76×24 logical (half-extents 38/12).
    // grow=1 lands on the reference capsule aspect 4.05 (676×167 @1x,
    // rescaled): 260×64 logical — half-width 130 = hw·0.8125, while hh_c
    // already reaches the 64px pill core at g=1. Overshoot g=1.25 → half-
    // width 138 ≤ hw: nothing clips (footprint 276×64 in 368×104).
    let hw_c = hw * (0.2375 + 0.575 * width_ease);
    // Height lags width (quadratic ease) — wider arrives before taller.
    let hh_c = hh * (0.375 + 0.625 * gr.clamp(0.0, 1.0).powi(2));
    // The capsule grows out of the collapsed clock pill (local y 29) and lands
    // hugging the top edge (local y 52); overshoot dips below rest first.
    let cy = height as f32 * (29.0 + 23.0 * gr) / SURF_H as f32;
    let rad = hh_c; // stadium: radius = core height / 2
    let reach = GLOW_REACH * s;

    // Precompute bar capsules + leader dots once per frame — nothing allocates
    // in the pixel loop.
    let mut caps: Vec<Bar> = Vec::with_capacity(bars.len());
    let mut dots: Vec<(f32, f32)> = Vec::new(); // (center x, radius) per leader dot
    let mut flank_lo = 0.0f32; // fast-path x-range (distance from cx) holding dots
    let mut flank_hi = 0.0f32;
    if !bars.is_empty() && wave_op > 0.0 {
        let n = bars.len() as f32;
        // The waveform occupies the central span; leader dots flank it out
        // toward the capsule ends.
        let bar_span = BAR_SPAN_FRAC * hw_c;
        let slot = (2.0 * bar_span) / n;
        // Bars keep a little breathing room from the pill's top/bottom edge.
        let inner_h = (2.0 * hh_c - 24.0 * s).max(ELEM_R_MAX * 2.0 * s);
        for (i, &v) in bars.iter().enumerate() {
            let v = if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Idle elements stay visible as small dim dots (never zero height).
            // Gentle center-weight envelope: heights taper from the middle
            // element to the edges, so silence reads as a pyramid of dots.
            let env = 0.55 + 0.45 * (PI * ((i as f32 + 0.5) / n)).sin();
            let half_h = (v * inner_h * 0.5).max(DOT_R_MIN * s) * env;
            // ONE element kind: width scales with height, 1.1→2.5 logical px —
            // a ~2px dot at idle, a ~5px bar when tall; caps fully rounded.
            let r = (ELEM_R_MAX * s).min(half_h).max(DOT_R_MIN * s);
            let bcx = cx - bar_span + slot * (i as f32 + 0.5);
            // Peak brightness scales with height: dim dot at idle, full
            // gradient color when tall. Gradient interpolates along x.
            let bright = 0.5 + 0.5 * v;
            let u = i as f32 / (n - 1.0).max(1.0);
            let (cr, cg, cb) = wave_color(u);
            // Leader-zone dimming scales the element color toward the row
            // ends (bloom weights and the capsule body stay untouched). The
            // idle floor keeps every element above the body, so dimmed tails
            // read as progressively dimmer dots, never pits.
            let dim = edge_dim(u);
            // Slow per-element shimmer (brighten-only, 1.0..1.10) keeps a
            // constant input alive without distorting the palette stops.
            let shimmer = 1.0 + 0.05 * (1.0 - (t * 3.2 + i as f32 * 1.05).cos());
            caps.push(Bar {
                cx: bcx,
                r,
                seg: (half_h - r).max(0.0),
                gw: 0.15 + 0.85 * v,
                cr: cr * bright * shimmer * dim,
                cg: cg * bright * shimmer * dim,
                cb: cb * bright * shimmer * dim,
                x0: ((bcx - r - reach).floor() as i32).max(0),
                x1: ((bcx + r + reach).ceil() as i32).min(width as i32),
                y0: ((cy - half_h - reach).floor() as i32).max(0),
                y1: ((cy + half_h + reach).ceil() as i32).min(height as i32),
            });
        }
        // Dotted leader lines: small dim cyan dots at the wave row's centerline,
        // running from near the outermost bar toward each capsule end.
        let dr = (DOT_R * s).max(0.6);
        let edge = bar_span - slot * 0.5 + ELEM_R_MAX * s; // outermost element's outer edge
        flank_lo = edge + 0.5 * s;
        flank_hi = hw_c - 0.5 * s;
        let mut off = edge + 3.5 * s + DOT_STEP * s; // one full step clear of
        // the outermost bar: at the 27-bar pitch (≈8px) the old 3.5px offset
        // put the first leader dot ~2.3px from the bar edge, reading as a
        // 28th wave element in the flipbook element-count probe.
        let end = hw_c - 2.5 * s;
        while off + dr < end {
            dots.push((cx - off, dr));
            dots.push((cx + off, dr));
            off += DOT_STEP * s;
        }
    }

    for y in 0..h {
        let py = y as f32 + 0.5;
        let dy_top = py - (cy - hh_c); // 0 at the pill's top edge
        for x in 0..w {
            let px = x as f32 + 0.5;
            let d = sd_rounded_rect(px, py, cx, cy, hw_c, hh_c, rad);
            // Drop shadow: silhouette shifted +6px down, quadratic falloff over
            // ~18px, masked to below the capsule midline (sides stay clean).
            let ds = sd_rounded_rect(px, py - SHADOW_OFF * s, cx, cy, hw_c, hh_c, rad);
            let st = clamp01(1.0 - ds.max(0.0) / (SHADOW_SPAN * s));
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = smooth(st) * SHADOW_A
                * clamp01((py - cy - 2.0 * s) / hh_c) // below the capsule only
                * clamp01((hw_c + 2.0 * s - (px - cx).abs()) / (4.0 * s)); // no side spill

            // Pill body: near-black with a green-gray cast (rgb 35,38,37),
            // a subtle +10 sheen at the top-left fading darker rightward. No
            // rim, no hairline. While collapsed (grow < 0.15) it relaxes to a
            // flat #1F1E1D twin of the clock pill, blending out of the flat
            // zone as grow rises.
            let cov = sdf_cov(d);
            if cov > 0.0 {
                let twin = smooth(clamp01((0.15 - gr) / 0.15)); // 1 when collapsed
                let flat = 1.0 - twin;
                let sheen = SHEEN
                    * (1.0 - clamp01((px - (cx - hw_c)) / (2.0 * hw_c)))
                    * (1.0 - clamp01(dy_top / (2.0 * hh_c)));
                let br = TWIN.0 * twin + (BG.0 + sheen) * flat;
                let bg = TWIN.1 * twin + (BG.1 + sheen) * flat;
                let bb = TWIN.2 * twin + (BG.2 + sheen) * flat;
                (r, g, b, a) = over(r, g, b, a, br, bg, bb, BODY_A * cov);
            }

            // Element cores (teal→cyan→blue capsules, brightness × height) +
            // accumulated bloom intensity, faded in with the entrance so the
            // wave blooms mid-bounce. bcov_max lifts the bloom out from under
            // the cores so each keeps its own color instead of saturating.
            let mut glow = 0.0f32;
            let mut glow_r = 0.0f32;
            let mut glow_g = 0.0f32;
            let mut glow_b = 0.0f32;
            let mut bcov_max = 0.0f32;
            if wave_op > 0.0 {
                for bar in &caps {
                    let (xi, yi) = (x as i32, y as i32);
                    if xi < bar.x0 || xi >= bar.x1 || yi < bar.y0 || yi >= bar.y1 {
                        continue; // fast path: outside this element's precomputed range
                    }
                    let db = sd_capsule(px, py, bar.cx, cy, bar.seg, bar.r);
                    let bcov = sdf_cov(db);
                    if bcov > bcov_max {
                        bcov_max = bcov;
                    }
                    if bcov > 0.0 {
                        (r, g, b, a) = over(
                            r,
                            g,
                            b,
                            a,
                            bar.cr,
                            bar.cg,
                            bar.cb,
                            255.0 * bcov * wave_op,
                        );
                    }
                    if db < reach {
                        let w = bar.gw * (-db.max(0.0) / (GLOW_K * s)).exp();
                        glow += w;
                        glow_r += bar.cr * w;
                        glow_g += bar.cg * w;
                        glow_b += bar.cb * w;
                    }
                }
            }

            // Additive bloom in each element's own color (weighted by its own
            // gradient contribution), masked to the pill interior and lifted
            // from under the cores.
            if glow > 0.0 {
                let gi = glow * pulse * cov * (1.0 - bcov_max) * wave_op;
                let k = BLOOM * gi / glow;
                r += glow_r * k;
                g += glow_g * k;
                b += glow_b * k;
                a += GLOW_A * gi;
            }

            // Dotted leader lines: dim cyan dots at the wave row's centerline,
            // masked to the capsule interior (alpha carries cov · wave_op).
            if !dots.is_empty() {
                let adx = (px - cx).abs();
                if adx > flank_lo && adx < flank_hi {
                    for &(dx, dr) in &dots {
                        if (px - dx).abs() > dr + AA_BAND {
                            continue; // fast path: outside this dot's AA reach
                        }
                        let dcov = sdf_cov(sd_capsule(px, py, dx, cy, 0.0, dr));
                        if dcov > 0.0 {
                            // Same gradient family, sampled at the dot's x,
                            // dimmed to ~24% (spec tail ≈20–25% of peak) and
                            // multiplied by the same leader-zone falloff the
                            // row elements get, so tails fade like the row.
                            let u = 0.5 - 0.5 * (px - cx) / cx;
                            let (or_, og, ob) = wave_color(u);
                            let d = edge_dim(u);
                            let (or_, og, ob) = (or_ * d, og * d, ob * d);
                            (r, g, b, a) = over(
                                r,
                                g,
                                b,
                                a,
                                or_,
                                og,
                                ob,
                                DOT_A * dcov * cov * wave_op,
                            );
                        }
                    }
                }
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
        frame_g(bars, t, ma, 1.0)
    }

    fn frame_g(bars: &[f32], t: f32, ma: f32, grow: f32) -> Vec<u8> {
        let mut c = vec![0u8; (W * H * 4) as usize];
        render_pill(&mut c, W, H, bars, t, ma, grow);
        c
    }

    fn alpha(c: &[u8], x: u32, y: u32) -> u8 {
        c[((y * W + x) * 4 + 3) as usize]
    }

    fn chan(c: &[u8], x: u32, y: u32, ch: u32) -> u8 {
        c[((y * W + x) * 4 + ch) as usize]
    }

    // 27 bars: middle bar (index 13) is centered exactly at x = 184 at scale 1.
    const MID_X: u32 = 184;
    /// Element count used by the generic wave tests (the pipeline's NUM_BARS;
    /// spelled locally so the canvas layer stays len-agnostic).
    const N27: usize = 27;
    /// grow=1 capsule half-width: 0.8125 · hw (hw = 160 at scale 1).
    const HW1: f32 = 130.0;
    /// Flat bar array with a single tall element at `idx` (mirrored center
    /// index 13 for 27 bars).
    fn one_hot(n: usize, idx: usize) -> Vec<f32> {
        let mut v = vec![0.0; n];
        v[idx] = 1.0;
        v
    }
    /// Element-center x at scale 1 for n bars: central span 0.68·130 around
    /// x=184, slot = 2*span/n, centers at slot*(i+0.5) from the left end.
    fn cx_of(i: usize, n: usize) -> f32 {
        let span = 0.68f32 * HW1;
        let slot = 2.0 * span / n as f32;
        184.0 - span + slot * (i as f32 + 0.5)
    }

    #[test]
    fn corners_stay_transparent() {
        let c = frame(&[0.5; N27], 0.0, 1.0);
        // Corners plus mid-edge probes beyond the halo reach. The bottom edge
        // is intentionally exempt: the drop shadow lives there.
        let probes = [
            (0u32, 0u32),
            (W - 1, 0),
            (0, H - 1),
            (W - 1, H - 1),
            (MID_X, 2),
            (2, H / 2),
            (W - 3, H / 2),
        ];
        for &(x, y) in &probes {
            let i = ((y * W + x) * 4) as usize;
            assert!(
                c[i..i + 4].iter().all(|&v| v == 0),
                "px ({x},{y}) not fully transparent"
            );
        }
        // The pill interior is essentially opaque.
        assert!(alpha(&c, W / 2, H / 2) > 200);
    }

    #[test]
    fn shadow_outside_core_is_soft_and_under_interior() {
        let c = frame(&[0.5; N27], 0.0, 1.0);
        // ~4px below the pill core bottom edge (core bottom at y = 84).
        let near = alpha(&c, MID_X, 88);
        let far = alpha(&c, MID_X, 96);
        assert!(near > 0, "shadow missing just below the pill");
        assert!(near < 235, "shadow {near} must be softer than interior 235");
        assert!(
            far < near,
            "shadow must fall off with distance: {far} >= {near}"
        );
    }

    #[test]
    fn glow_falls_off_with_distance() {
        // Center bar (13) at value 0.5: core half-height 10 logical px (scale
        // 1), so y=40 and y=34 are both above the core but inside the pill,
        // in the bloom field.
        let c = frame(&[0.5; N27], 0.0, 1.0);
        let g_near = chan(&c, MID_X, 40, 1); // green, ~1.5px past the capsule end
        let g_far = chan(&c, MID_X, 34, 1); // green, ~7.5px past the capsule end
        assert!(
            g_near > g_far + 20,
            "bloom must decay: near {g_near} vs far {g_far}"
        );
    }

    #[test]
    fn taller_bar_covers_more_pixels_at_same_x() {
        let low = frame(&[0.3; N27], 0.0, 1.0);
        let high = frame(&[0.9; N27], 0.0, 1.0);
        let count = |c: &[u8]| -> u32 {
            let mut n = 0;
            for y in 0..H {
                // Wave core: strong green/blue vs the near-neutral body.
                if chan(c, MID_X, y, 1) > 100 && chan(c, MID_X, y, 1) > chan(c, MID_X, y, 2) + 25 {
                    n += 1;
                }
            }
            n
        };
        let cl = count(&low);
        let ch = count(&high);
        assert!(
            ch > cl + 10,
            "taller bar must cover more rows: {ch} vs {cl}"
        );
    }

    #[test]
    fn capsule_peaks_reach_bright_teal() {
        // Argmax of the green channel inside the body: mid-range bars must
        // already show clearly bright peaks (>= 180) ...
        let scan = |c: &[u8]| -> u8 {
            let mut m = 0u8;
            for y in 24..80 {
                for x in 28..(W - 28) {
                    m = m.max(chan(c, x, y, 1));
                }
            }
            m
        };
        let t0 = 0.0f32; // shimmer is brighten-only now; t=0 shows the exact palette
        let mid = scan(&frame(&[0.5; N27], t0, 1.0));
        // v=0.5 -> brightness 0.75 -> G = 220*0.75 ≈ 165 at the exact palette.
        assert!(mid >= 160, "mid bars: peak G only {mid}");
        // ... and tall bars reach near-full-bright peak cyan rgb(140,220,222).
        let tall = scan(&frame(&[1.0; N27], 0.0, 1.0));
        assert!(tall >= 220, "tall bars: peak G only {tall}");
        // Peak brightness scales with bar height.
        let short = scan(&frame(&[0.25; N27], 0.0, 1.0));
        assert!(
            tall > short + 40,
            "peak must scale with height: {tall} vs {short}"
        );
    }

    #[test]
    fn wave_gradient_matches_spec_stops() {
        // Element color interpolates teal → peak cyan → blue along the span.
        // Slot layout at g=1 (hw_c = 130): span 0.68·130 ≈ 88, slot ≈ 6.6,
        // element centers at x = 184 - 88 + slot·(i+0.5); sampled at y = 52
        // (wave centerline).
        let c = frame(&[1.0; N27], 0.0, 1.0);
        let cx_at = |i: u32| -> u32 { cx_of(i as usize, N27).round() as u32 };
        // Tolerance: sheen/bloom/dither + quantization + envelope variance.
        let tol = 20i32;
        // Left end (i=0): teal rgb(125,165,165) × the leader-zone falloff
        // (edge_dim(0) = 0.785) = rgb(98,130,130).
        let (r, g, b) = (
            chan(&c, cx_at(0), 52, 2) as i32,
            chan(&c, cx_at(0), 52, 1) as i32,
            chan(&c, cx_at(0), 52, 0) as i32,
        );
        assert!((r - 98).abs() <= tol, "left dimmed teal R {r} vs 98");
        assert!((g - 130).abs() <= tol, "left dimmed teal G {g} vs 130");
        assert!((b - 130).abs() <= tol, "left dimmed teal B {b} vs 130");
        // Center (i=4, x=184): peak cyan rgb(140,220,222).
        let (r, g, b) = (
            chan(&c, MID_X, 52, 2) as i32,
            chan(&c, MID_X, 52, 1) as i32,
            chan(&c, MID_X, 52, 0) as i32,
        );
        assert!((r - 140).abs() <= tol, "center cyan R {r} vs 140");
        assert!((g - 220).abs() <= tol, "center cyan G {g} vs 220");
        assert!((b - 222).abs() <= tol, "center cyan B {b} vs 222");
        // Right end (i=26): blue rgb(100,130,170) × 0.785 = rgb(79,102,134)
        // (measured picks up a small edge bloom lift, inside tol).
        let (r, g, b) = (
            chan(&c, cx_at(26), 52, 2) as i32,
            chan(&c, cx_at(26), 52, 1) as i32,
            chan(&c, cx_at(26), 52, 0) as i32,
        );
        assert!((r - 79).abs() <= tol, "right dimmed blue R {r} vs 79");
        assert!((g - 102).abs() <= tol, "right dimmed blue G {g} vs 102");
        assert!((b - 134).abs() <= tol, "right dimmed blue B {b} vs 134");
    }

    #[test]
    fn body_bg_is_green_gray_near_black_not_navy() {
        // Dither jitters single pixels ±1 per channel independently, so the
        // green-gray cast is asserted on patch MEANS (dither is zero-mean).
        // Interior body away from the sheen (right half) and away from wave
        // rows' verticals: must average ≈ rgb(35,38,37), g >= r >= b.
        // Patches sit inside the grow-1 capsule (right hull edge at x≈314).
        let c = frame(&[], 0.0, 1.0);
        let patch = |x0: u32, y0: u32, ch: u32| -> f32 {
            let mut s = 0f32;
            for y in y0..y0 + 8 {
                for x in x0..x0 + 8 {
                    s += chan(&c, x, y, ch) as f32;
                }
            }
            s / 64.0
        };
        let mean = |x0: u32, y0: u32| -> (f32, f32, f32) {
            (
                patch(x0, y0, 2),
                patch(x0, y0, 1),
                patch(x0, y0, 0),
            )
        };
        for &(x0, y0) in &[(266u32, 60u32), (282, 62), (266, 70)] {
            let (r, g, b) = mean(x0, y0);
            assert!(
                (r - 35.0).abs() <= 1.5 && (g - 38.0).abs() <= 1.5 && (b - 37.0).abs() <= 1.5,
                "bg patch ({x0},{y0}) = rgb({r},{g},{b}), want ≈ rgb(35,38,37)"
            );
            // The palette's own cast: g highest, b = r + 2. Navy would be b > g
            // or b − r ≈ 10.
            assert!(g >= b && b - r <= 3.0, "green-gray cast violated: rgb({r},{g},{b})");
        }
        // Top-left sheen adds ~+10: still green-gray, never navy (b > g is the
        // navy tell — forbid it). Patch sits ~6px inside the grow-1 capsule's
        // left end cap (center (86,52), r 32).
        let (r, g, b) = mean(96, 28);
        assert!(g >= b && b - r <= 3.0, "sheen must stay green-gray: rgb({r},{g},{b})");
        assert!(r > 35.0, "top-left sheen missing: R {r} not above the 35 floor");
        assert!(r < 50.0, "sheen must stay subtle: R {r}");
    }

    #[test]
    fn bloom_adds_energy_around_tall_bars() {
        // Sum a neighborhood around the tall center bar with the full frame,
        // then re-render with the bloom contribution isolated by comparing
        // against a flat-wave frame whose cores occupy the same pixels: the
        // tall-bar frame must carry strictly more total energy nearby.
        let sum_green = |c: &[u8]| -> u64 {
            let mut s = 0u64;
            for y in 0..H {
                for x in (MID_X - 20)..=(MID_X + 20) {
                    s += chan(c, x, y, 1) as u64;
                }
            }
            s
        };
        // Tall bar (bloom strong, gw≈1) vs flat dim wave (cores everywhere but
        // bloom weak, gw≈0.15 at v=0).
        let tall = sum_green(&frame(&one_hot(N27, 13), 0.0, 1.0));
        let flat = sum_green(&frame(&[0.0; N27], 0.0, 1.0));
        assert!(
            tall > flat + 2000,
            "bloom around a tall bar must exceed the dim-dot field: {tall} vs {flat}"
        );
    }

    #[test]
    fn shadow_below_only_no_side_halos() {
        // Below the capsule midline: shadow present; at the same distance
        // above/beside the silhouette: nothing.
        let c = frame(&[], 0.0, 1.0);
        // Core bottom edge at y=84; 4px below → inside the shadow falloff.
        assert!(alpha(&c, MID_X, 88) > 0, "shadow missing below the capsule");
        // Same offset OUTSIDE the left silhouette (x=20 < 24 pad): no halo.
        assert_eq!(alpha(&c, 20, 88), 0, "side shadow halo leaked left");
        // Mirror point above the silhouette (y=88 is below cy=52; the mirrored
        // distance above the top edge is y=16): nothing there.
        assert_eq!(alpha(&c, MID_X, 16), 0, "shadow leaked above the capsule");
        // The left flank of the pill (x=18, mid-height): clean.
        assert_eq!(alpha(&c, 18, 52), 0, "side halo leaked at mid height");
    }

    #[test]
    fn master_alpha_half_halves_alpha() {
        let full = frame(&[0.6; N27], 0.0, 1.0);
        let half = frame(&[0.6; N27], 0.0, 0.5);
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
    fn idle_dots_stay_visible() {
        // At bars=0 the elements must still read as small dim gradient dots —
        // not vanish. Idle cores render at 50% brightness and the leader-zone
        // falloff dims the tails: center dot G≈78, tail dots G≈55–63 (the
        // right end is dimmest — the blue palette stop carries G=130) vs
        // body-only G≤44 at the spot. The visibility bar stays "clearly
        // brighter than bare body"; the contrast margin is re-anchored to the
        // measured dimmed tails (+12 G over body; was +20 pre-falloff).
        let dots = frame(&[0.0; N27], 0.0, 1.0);
        let body = frame(&[], 0.0, 1.0);
        for i in 0..N27 {
            // Exact element center the renderer uses: waveform spans the central
            // 68% of the pill core, slot = 2*0.68*160/27.
            let x = cx_of(i, N27).round() as u32;
            let gd = chan(&dots, x, H / 2, 1);
            let gb = chan(&body, x, H / 2, 1);
            assert!(
                gd > 50 && gd > gb + 12,
                "idle dot {i} at x={x} not visible: dot G={gd} vs body G={gb}"
            );
        }
    }

    #[test]
    fn master_alpha_zero_writes_all_zero() {
        let c = frame(&[0.8; N27], 3.0, 0.0);
        assert!(
            c.iter().all(|&b| b == 0),
            "master_alpha 0 must blank the frame"
        );
    }

    #[test]
    fn absurd_inputs_do_not_panic_and_stay_in_range() {
        // Non-finite bars / time / alpha, plus a too-small canvas: none may panic.
        let mut tiny = [0u8; 10];
        render_pill(
            &mut tiny,
            W,
            H,
            &[f32::NAN, 2.0, f32::NEG_INFINITY],
            0.0,
            1.0,
            f32::INFINITY,
        );
        let c = frame(
            &[f32::NAN, 2.0, -1.0, f32::INFINITY, 0.0],
            f32::NAN,
            f32::NAN,
        );
        assert!(
            c.iter().all(|&b| b == 0),
            "NaN master_alpha must blank the frame"
        );
        let c = frame(&[f32::NAN, 2.0, -1.0, f32::INFINITY, 0.0], 1e9, 1.0);
        // Bloom-saturated pixels must have been clamped, not wrapped: a core
        // pixel of the v=1.0 element (index 1: 5 elements over the central
        // 68% span; grow clamps to 1.25 → half-width 138, span 93.9, center
        // x = 184 - 93.9 + 37.55·1.5 ≈ 146.4) must be opaque and in range.
        let (b, g, r, a) = (
            chan(&c, 146, 52, 0),
            chan(&c, 146, 52, 1),
            chan(&c, 146, 52, 2),
            chan(&c, 146, 52, 3),
        );
        assert_eq!(a, 255);
        assert!(g > 140 && b > 140, "core pixel not bright: {r},{g},{b}");
        // Empty bars: body and shadow still render, corners stay clean.
        let c = frame(&[], 0.0, 1.0);
        assert!(alpha(&c, W / 2, H / 2) > 200);
        assert_eq!(alpha(&c, 0, 0), 0);
    }

    #[test]
    fn grow_zero_is_smaller_and_dimmer_than_grow_one() {
        let bright = |c: &[u8]| -> u32 {
            c.iter()
                .skip(3)
                .step_by(4)
                .filter(|&&a| a > 8)
                .count() as u32
        };
        let collapsed = bright(&frame_g(&[0.5; N27], 0.0, 1.0, 0.0));
        let full = bright(&frame_g(&[0.5; N27], 0.0, 1.0, 1.0));
        assert!(
            collapsed < full,
            "collapsed pill must have smaller bright footprint: {collapsed} vs {full}"
        );
        // grow=0 is the clock-pill twin: flat #1F1E1D, NO cyan.
        let c = frame_g(&[0.5; N27], 0.0, 1.0, 0.0);
        let mut max_g = 0u8;
        for y in 0..H {
            for x in 0..W {
                max_g = max_g.max(chan(&c, x, y, 1));
            }
        }
        assert!(
            max_g < 70,
            "collapsed twin must show no cyan (bars/dots/glow): max G {max_g}"
        );
    }

    #[test]
    fn grow_zero_twin_covers_the_real_clock_pill() {
        // The dynamite-rust clock pill occupies x 714..790, y 3..27 — half-
        // extents 38×12 around center (752,15). In surface coords that is
        // (752-368, 15) = (384, 15) at the pill's rest scale... the twin is
        // drawn at cy = 29/104·H with the same half-extents, so probe its
        // silhouette edges directly: half-width 38 → ±38 from cx, half-height
        // 12 → ±12 from cy. Tolerance ±0.6 logical px.
        let c = frame_g(&[0.5; N27], 0.0, 1.0, 0.0);
        let cy0 = H as f32 * 29.0 / 104.0; // 29.0
        let cy0i = cy0.round() as u32; // 29
        let half_w = {
            // Rightmost opaque column at the twin's mid-row.
            let mut right = 0i32;
            for x in 0..W as i32 {
                if alpha(&c, x as u32, cy0i) > 100 {
                    right = x;
                }
            }
            right as f32 + 0.5 - (W as f32 * 0.5)
        };
        let half_h = {
            let mut bottom = 0i32;
            for y in 0..H as i32 {
                if alpha(&c, MID_X, y as u32) > 100 {
                    bottom = y;
                }
            }
            bottom as f32 + 0.5 - cy0
        };
        assert!(
            (half_w - 38.0).abs() <= 0.6,
            "twin half-width {half_w} must be 38.0 ±0.6"
        );
        assert!(
            (half_h - 12.0).abs() <= 0.6,
            "twin half-height {half_h} must be 12.0 ±0.6"
        );
    }

    #[test]
    fn twin_blends_to_capsule_bg_as_grow_rises() {
        // At grow 0 the body is exactly the #1F1E1D clock-pill twin (the
        // smooth blend ease means grow 0.05 is already partially blended, so
        // the blend DIRECTION is asserted at grow 0.5 instead).
        let collapsed = frame_g(&[], 0.0, 1.0, 0.0);
        let mid = frame_g(&[], 0.0, 1.0, 0.5);
        // (160, 36): inside the twin (half-extents 38×12 around (184,29)) at
        // both grows, away from edges and the sheen corner.
        let cr = chan(&collapsed, 160, 36, 2) as i32;
        let cg = chan(&collapsed, 160, 36, 1) as i32;
        let cb = chan(&collapsed, 160, 36, 0) as i32;
        assert!(
            (cr - 31).abs() <= 2 && (cg - 30).abs() <= 2 && (cb - 29).abs() <= 2,
            "grow-0 body not #1F1E1D: rgb({cr},{cg},{cb})"
        );
        let mr = chan(&mid, 160, 36, 2) as i32;
        let mg = chan(&mid, 160, 36, 1) as i32;
        let mb = chan(&mid, 160, 36, 0) as i32;
        assert!(mg >= mr && mg >= mb && mb - mr <= 3, "blended body lost the green-gray cast: {mr},{mg},{mb}");
        assert!(mg > cg + 3, "body must blend toward the capsule bg as grow rises: G {mg} vs twin {cg}");
    }

    #[test]
    fn grow_overshoot_and_nan_do_not_panic_or_overflow() {
        for grow in [1.25f32, f32::NAN, f32::INFINITY, -0.5] {
            let c = frame_g(&[0.9; N27], 1.0, 1.0, grow);
            // Premultiplied invariant: no color channel above its alpha.
            for i in (0..c.len()).step_by(4) {
                assert!(
                    c[i] <= c[i + 3] && c[i + 1] <= c[i + 3] && c[i + 2] <= c[i + 3],
                    "grow {grow}: pixel not premultiplied: {:?}",
                    &c[i..i + 4]
                );
            }
        }
    }

    #[test]
    fn dotted_leader_lines_present_outside_bar_span_at_grow_one() {
        let c = frame_g(&[0.5; N27], 0.0, 1.0, 1.0);
        // Waveform occupies the central 68% of the 260-wide capsule
        // (x ≈ 95.6..272.4); dots live outside that range but inside the
        // capsule (hull edges ≈ 54/314): probe the flanks.
        let mut wave_dots = 0u32;
        for x in 56..90 {
            // Gradient-family dots: G ≥ R and B between them (teal→blue), or
            // plainly brighter than the body.
            let (b, g, r, a) = (
                chan(&c, x, 52, 0),
                chan(&c, x, 52, 1),
                chan(&c, x, 52, 2),
                alpha(&c, x, 52),
            );
            if a > 20 && g > r + 10 && b >= 37 {
                wave_dots += 1;
            }
        }
        assert!(
            wave_dots >= 2,
            "left leader line missing: only {wave_dots} wave-family px at row 52"
        );
        let mut wave_dots = 0u32;
        for x in 278..312 {
            let (b, g, r) = (chan(&c, x, 52, 0), chan(&c, x, 52, 1), chan(&c, x, 52, 2));
            if alpha(&c, x, 52) > 20 && g > r + 10 && b >= 37 {
                wave_dots += 1;
            }
        }
        assert!(
            wave_dots >= 2,
            "right leader line missing: only {wave_dots} wave-family px at row 52"
        );
        // ... and they are absent while collapsed.
        let c = frame_g(&[0.5; N27], 0.0, 1.0, 0.0);
        for x in 26..54 {
            assert!(
                chan(&c, x, 52, 1) < 70,
                "no dots may show at grow=0 (x={x}: G={})",
                chan(&c, x, 52, 1)
            );
        }
    }

    #[test]
    fn collapsed_pill_stays_inside_surface_with_margin() {
        // grow=0: capsule center y≈29, half-height 12, plus shadow — nothing
        // may touch the top rows (and only the shadow may sit low).
        let c = frame_g(&[0.5; N27], 0.0, 1.0, 0.0);
        for x in 0..W {
            assert_eq!(alpha(&c, x, 0), 0, "grow=0 bleeds at top (x={x})");
            assert_eq!(alpha(&c, x, 1), 0, "grow=0 bleeds at y=1 (x={x})");
        }
    }

    #[test]
    fn grow_one_capsule_aspect_matches_reference_4_05() {
        // Reference capsule 676×167 @1x = aspect 4.05, rescaled into the
        // 368×104 surface as 260×64 logical. Measure the α>100 hull at
        // scale 2 (736×208); the drop shadow (α ≤ 64) is excluded by the
        // threshold, so this is the capsule silhouette only.
        let (w, h) = (SURF_W * 2, SURF_H * 2);
        let mut big = vec![0u8; (w * h * 4) as usize];
        render_pill(&mut big, w, h, &[], 0.0, 1.0, 1.0);
        let a = |x: u32, y: u32| big[((y * w + x) * 4 + 3) as usize];
        let (mut left, mut right, mut top, mut bottom) = (w as i32, -1i32, h as i32, -1i32);
        for y in 0..h {
            for x in 0..w {
                if a(x, y) > 100 {
                    left = left.min(x as i32);
                    right = right.max(x as i32);
                    top = top.min(y as i32);
                    bottom = bottom.max(y as i32);
                }
            }
        }
        assert!(right >= 0, "grow=1 frame is empty");
        let fw = (right - left + 1) as f32;
        let fh = (bottom - top + 1) as f32;
        assert!(
            (fw / 2.0 - 260.0).abs() <= 3.0,
            "grown capsule width {fw} @2x, want ≈520 (260 logical)"
        );
        assert!(
            (fh / 2.0 - 64.0).abs() <= 3.0,
            "grown capsule height {fh} @2x, want ≈128 (64 logical)"
        );
        let aspect = fw / fh;
        assert!(
            (aspect - 4.05).abs() <= 0.10,
            "grow=1 aspect {aspect} must be 4.05 ±0.10 (hull {left}..{right} × {top}..{bottom})"
        );
    }

    #[test]
    fn grow_shapes_interpolate_monotonically_and_stay_inside_the_surface() {
        // Between the twin (38×12 half-extents) and the 4.05-aspect rest
        // shape (130×32) the silhouette must widen and tallen monotonically
        // (no S-curve weirdness), and the overshoot bulge (g=1.25) must stay
        // inside the 368×104 surface so nothing clips.
        let hull = |grow: f32| -> (i32, i32, i32, i32) {
            let c = frame_g(&[], 0.0, 1.0, grow);
            let (mut left, mut right, mut top, mut bottom) = (W as i32, -1i32, H as i32, -1i32);
            for y in 0..H {
                for x in 0..W {
                    // 150: body rows are ≥200, the ±1.25px AA skirt ≤106,
                    // the drop shadow ≤64 — only the silhouette crosses.
                    if alpha(&c, x, y) > 150 {
                        left = left.min(x as i32);
                        right = right.max(x as i32);
                        top = top.min(y as i32);
                        bottom = bottom.max(y as i32);
                    }
                }
            }
            (left, right, top, bottom)
        };
        let mut prev_w = 0i32;
        let mut prev_h = 0i32;
        for grow in [0.0f32, 0.25, 0.5, 0.75, 1.0, 1.25] {
            let (left, right, top, bottom) = hull(grow);
            let fw = right - left + 1;
            let fh = bottom - top + 1;
            assert!(
                fw > prev_w,
                "width must rise monotonically: grow {grow} width {fw} after {prev_w}"
            );
            assert!(
                fh >= prev_h,
                "height must not shrink: grow {grow} height {fh} after {prev_h}"
            );
            prev_w = fw;
            prev_h = fh;
            // Nothing may clip: the grown capsule stays inside the pill core
            // bounds (24px side pad; top hugs its edge, bottom well clear).
            assert!(
                left >= 26 && right <= W as i32 - 26,
                "grow {grow} clips horizontally: hull {left}..{right} in 0..{W}"
            );
            assert!(
                top >= 16 && bottom <= H as i32 - 2,
                "grow {grow} clips vertically: hull {top}..{bottom} in 0..{H}"
            );
        }
    }

    #[test]
    fn bloom_half_excess_radius_matches_reference_halo() {
        // Spec: bloom halo r≈15px at the 676px reference capsule width;
        // rescaled to our capsule that is ≈7px logical (≈14px @2x) of
        // half-excess past a tall bar's core edge (±30%). Render a one-hot
        // tall-bar frame at scale 2 and measure the horizontal
        // excess-over-bg profile rightward from the tallest bar, 8px @2x
        // above the centerline (clear of the idle-dot / leader-dot row).
        let n = crate::pill::vecmentor::NUM_BARS;
        let (w, h) = (SURF_W * 2, SURF_H * 2);
        let mut bars = vec![0.0; n];
        bars[n / 2] = 1.0;
        let mut big = vec![0u8; (w * h * 4) as usize];
        render_pill(&mut big, w, h, &bars, 0.0, 1.0, 1.0);
        let mut bg = vec![0u8; (w * h * 4) as usize];
        render_pill(&mut bg, w, h, &[], 0.0, 1.0, 1.0);
        let g = |c: &[u8], x: u32| c[((96 * w + x) * 4 + 1) as usize] as f32;
        // Tall bar core edge @2x: center 368 + radius 5 (r = 2.5 logical).
        let x_edge = (cx_of(n / 2, n) * 2.0 + 5.0).round() as u32;
        let excess = |d: u32| g(&big, x_edge + d) - g(&bg, x_edge + d);
        let v0 = excess(3); // first sample clear of the ±1.25px core AA band
        assert!(v0 > 40.0, "tall-bar bloom must be strong at the edge: {v0}");
        let mut r = 0.0f32;
        for d in 4..=48u32 {
            let v = excess(d);
            if v <= v0 * 0.5 {
                let vp = excess(d - 1);
                let frac = ((vp - v0 * 0.5) / (vp - v).max(1e-6)).clamp(0.0, 1.0);
                r = (d - 1) as f32 + frac;
                break;
            }
        }
        assert!(
            (10.0..=18.0).contains(&r),
            "bloom half-excess {r}px @2x, want ≈14 (spec 15px at the 676px reference width)"
        );
    }

    #[test]
    fn third_peak_colors_track_spec_stops_on_tall_bars() {
        // Third-peak sampling on the GROWN capsule: tall bars at u ≈ 1/6,
        // 1/2, 5/6 of the wave span must land within dRGB ≤ 60 of the
        // palette stop at their x (bloom is lifted under full cores, so the
        // core carries the exact gradient), with the teal→cyan→blue ordering
        // (g channel: L < C, C > R). Expectations come from the same
        // wave_color the renderer uses, so this tracks any NUM_BARS.
        let n = crate::pill::vecmentor::NUM_BARS;
        let idx = [n / 6, n / 2, 5 * n / 6];
        let mut bars = vec![0.0; n];
        for &i in &idx {
            bars[i] = 1.0;
        }
        let c = frame(&bars, 0.0, 1.0);
        let mut gs = [0.0f32; 3];
        for (k, &i) in idx.iter().enumerate() {
            let u = i as f32 / (n as f32 - 1.0).max(1.0);
            let (er, eg, eb) = wave_color(u);
            let x = cx_of(i, n).round() as u32;
            let (r, g, b) = (
                chan(&c, x, 52, 2) as f32,
                chan(&c, x, 52, 1) as f32,
                chan(&c, x, 52, 0) as f32,
            );
            let d = (r - er).abs() + (g - eg).abs() + (b - eb).abs();
            assert!(
                d <= 60.0,
                "third {k} (i={i}, u={u:.3}): rgb({r},{g},{b}) vs stop ({er},{eg},{eb}), dRGB {d}"
            );
            gs[k] = g;
        }
        assert!(
            gs[0] < gs[1] && gs[1] > gs[2],
            "gradient must read teal→cyan→blue: g at thirds = {:?}",
            gs
        );
    }

    #[test]
    fn scale_two_buffer_matches_logical_frame() {
        let mut small = vec![0u8; (SURF_W * SURF_H * 4) as usize];
        render_pill(&mut small, SURF_W, SURF_H, &[0.5; N27], 0.0, 1.0, 1.0);
        let mut big = vec![0u8; (SURF_W * 2 * SURF_H * 2 * 4) as usize];
        render_pill(&mut big, SURF_W * 2, SURF_H * 2, &[0.5; N27], 0.0, 1.0, 1.0);
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
