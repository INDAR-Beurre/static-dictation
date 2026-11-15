//! canvas: software rasterizer for the pill overlay — SDF anti-aliased
//! near-black pill with a teal→cyan→blue equalizer wave, soft bloom and a
//! below-only drop shadow.
//!
//! Style (measured from an iPhone Dynamic-Island reference): a near-black
//! stadium-shaped pill with a dark navy cast (rgb 16,20,30) in the logical
//! 220×64 surface (16px side pad, pill core 188×44); the grown pill hugs the
//! top of the surface (grow-1 pill 148×36 at y 3..39). A subtle +10 RGB sheen
//! crowns the body's top-left and fades darker rightward; a ~1px teal-white
//! rim (~9%) hugs the anti-aliased edge and a bright specular line rolls off
//! along the top edge — the capsule must read against the dark navy desktop.
//! The waveform is ONE element kind: a vertical capsule with fully rounded
//! caps (r = w/2) that reads as a ~1.1px dim dot at idle and grows into a
//! ~2px-wide bar; a center-weight envelope (floor 0.75) tapers element
//! heights toward the edges so silence reads as a centered pyramid of dots,//! and a broad gaussian peak emphasis (gain 1.0, σ² 19 elements²) spreads the
//! center cluster into a wide lit mound (~26% of the pill width). Peak
//! brightness follows a steep curve (0.35 + 0.65·v^0.6) and bars whose
//! peak-emphasized field reaches ~1.0 snap to SOLID hot teal rgb(171,239,238)
//! — the reference never bleaches white (max G 242, R/G 0.72), so there is no
//! white plateau. Bars dim toward the row ends under the whisper ramp (C9:
//! ref tails hold ≈25% of peak G with a hard stop — no tail fade).
//! Element color interpolates along x — teal
//! rgb(125,165,165) at the left, peak cyan rgb(140,220,222) at the center, blue
//! rgb(100,130,170) at the right. The dotted leader line CONTINUES the bar
//! grid (same ≈3.7px pitch, one slot past the outermost bar) out to
//! hw−15px and carries the reference whisper ramp: the left (teal) flank
//! climbs steeply (≈55%→18% of peak G), the right (blue) flank sags gently
//! (≈37%→18%) with B−G ≥ +30 throughout, and the line ENDS HARD at the last
//! slot (no fade-out tail); end dots sit ≈ body+20 lum. The grown (grow=1) capsule draws at
//! 148×36 logical — the compact island pill (the 188×44 core bounds the
//! overshoot bulge). A soft additive bloom in the strongest element's own
//! color (a per-pixel MAX over sources, so 27 bars at ~3.7px pitch cannot
//! fuse into one sheet — gaps read as gaps; ≈2.75px logical kernel scaled
//! 0.35..1.0 by bar height, faint to ~18px, pulsing slowly with `t_secs`)
//! halos the tall center cluster; only elements with v ≥ 0.35 emit (plus a
//! quiet "idle hum": at low energy the peak-emphasis cluster keeps ≈20–30%
//! of the active halo alive, breathing with v so the posed silence glows),
//! the field is capped at 1.0, and the bloom is masked to the pill interior
//! so the frame outside stays clean. Halos sample each element's PRE-hot teal
//! ramp color so the glow stays teal (halo R/G ≈ 0.6) instead of picking up
//! white. A soft dark crease runs just below the top edge (the reference has
//! NO bright keyline), and a 35% tinted drop shadow (offset −2.25/+1.5 px,
//! exponential λ≈7.2px falloff, tint rgb(10,20,35) — light from the
//! upper-right per the reference measure) pools under the capsule and
//! faintly along its lower-left flank.
//!
//! Entrance (iPhone Dynamic-Island style): the caller drives a spring via
//! `grow`. The capsule's width leads and its height lags (squash-stretch), its
//! center rides from the collapsed clock-pill position (y = 15 of 64)
//! up to its rest position (y = 21), and overshoot above
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
//!   `width`/`height` are the BUFFER pixel size (logical 220×64 × compositor
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

/// Logical surface width (pill core 188 + 16px pad each side).
pub const SURF_W: u32 = 220;
/// Logical surface height (pill core 44 + 10px pad top/bottom).
pub const SURF_H: u32 = 64;
const GROW_MAX: f32 = 1.25; // canvas-side overshoot clamp; live ceiling is tsar's GROW_CEIL_SHOW 1.12

const BODY_A: f32 = 255.0; // opaque: stored RGB == palette RGB
                           // Drop shadow + glass crease constants live below, next to SPEC.
const AA_BAND: f32 = 1.25;
// Element/dot AA band. Wave cores are ~1px radius (ref bar FWHM ≈2.7px, R5);
// through the 1.25px body band their deepest point (d = −1) only reaches
// coverage 0.97, so cores blend toward the body and cap ≈13% under the
// palette stop (measured G 225 vs the 239 hot teal). A tighter band lets the
// centre saturate without widening the bars past the reference.
const ELEM_AA: f32 = 0.45;
const BAR_SPAN_FRAC: f32 = 0.68; // waveform occupies the central span of the pill width
const DOT_R: f32 = 0.55; // ≈2.2px FWHM at the reference's 2× render scale
const DOT_STEP: f32 = 2.05; // ≈4.1px pitch at the reference's 2× render scale
const DOT_A: f32 = 255.0; // leader-dot peak alpha — the reference dots are
                          // opaque marks; the whisper ramp carries their dimness.
const VIS_FLOOR: f32 = 30.0; // C9 visibility floor: a ramped element's excess
                             // over body never drops below this per channel, so
                             // tail and idle dots stay ≥ +10 above the body
                             // even where the whisper fraction is tiny. 30
                             // matches the reference tail dots (end-dot G ≈60
                             // over body ≈32 → +28 excess, ≈13% of the 219
                             // peak excess — the R2 hard-stop fraction).
const BLUE_BIAS: f32 = 0.3; // right-flank blue weight per unit fpos (bars)
const DOT_BLUE_BIAS: f32 = 2.0; // keeps both endpoint dots blue (B−G ≥ +30)
const _DOT_A_OLD: f32 = 100.0; // leader-dot peak alpha (≈39% — tails read like the ref)
                               // Leader-zone dimming: brightness falls off toward both ends of the row so the
                               // island reads as a lit center cluster with dim tail dots (reference: tail ramp
                               // ≈35/255 ≈ bright-step of the peak). Measured effect at idle: outermost leader
                               // dots land at ≈25% of the center dot's bg-excess (raw-channel ratio ≈0.36,
                               // spec tail 0.10 relative to cluster 1.00 + body ≈42/255) — the multiplier is
                               // per-channel color, never below zero.
/// Reference whisper ramp (R2): core brightness fraction by position within
/// each flank — LEFT flank climbs steeply (8%→100% of peak excess over the
/// half-flank), RIGHT flank sags harder (47%→14%). Indexed by pos across the
/// flank (t: L 0=outer edge→1=center; R 0=center→1=outer edge). Sampled with
/// lerp. Rebuilt from critique/ref-deep-measure.md R2 (G%peak → bg-excess
/// fraction of the peak's +199 excess) — steeper than the first pass so the
/// lit cluster reads as a ~26%-of-W mound instead of a narrow spike.
const WHISPER_L: [f32; 8] = [0.08, 0.22, 0.31, 0.54, 0.75, 0.86, 0.94, 1.0];
const WHISPER_R: [f32; 8] = [1.0, 0.85, 0.72, 0.52, 0.46, 0.26, 0.17, 0.14];

/// Sample a whisper ramp table at t (0..1) with linear interpolation.
#[inline]
fn whisper(tbl: &[f32; 8], t: f32) -> f32 {
    let x = clamp01(t) * 7.0;
    let i = x as usize;
    let f = x - i as f32;
    tbl[i] + (tbl[(i + 1).min(7)] - tbl[i]) * f
}
const GLOW_K: f32 = 2.5; // bloom exp kernel at v=1, logical px — two-scaled per bar
                         // (k = GLOW_K·(0.35 + 0.65·v)) so tall bars get wide hot halos and dim
                         // elements stay tight; at the 27-bar pitch (≈3.7px logical) the mid-gap
                         // field is ~0.7× the at-edge value instead of the old sum's ≈1.3×, so
                         // gaps read as gaps.
const GLOW_REACH: f32 = 18.0; // faint bloom halo cut-off (~18px); at K=2.75 the
                              // cutoff step is ~0.15% of the edge value — dithered away.
const BLOOM: f32 = 0.28; // keep gap haze below the measured +20 G limit
const GLOW_TEAL: f32 = 1.05; // restrained halo lift; preserve dark inter-bar gaps
                             // Bloom alpha gain: must stay ≥ max glow channel (255·GLOW_TEAL ≈ 293)
                             // × BLOOM (≈146.5) so the additive step can never push a color channel
                             // above alpha (premultiplied).
const GLOW_A: f32 = 197.0;
/// Idle halo hum (C11): at near-zero bar energy the peak-emphasis cluster
/// keeps this fraction of its would-be halo alive, scaled by its own peak
/// weight — the posed silence glows dimly (ref halo α ≈ 25% of active) and
/// breathes with the signal via v.
const GLOW_IDLE: f32 = 0.30;
// 27 bars at pitch 2·0.68·74/27 ≈ 3.73 logical px (0.68 of the 148px grown
// pill width) → ~2.7px bar width max.
const ELEM_R_MAX: f32 = 1.2; // element half-width max → ~2.4px bars (ref FWHM 2.7px)
const DOT_R_MIN: f32 = 0.75; // idle element radius floor → ~1.5px dots. The critique asked
                             // 0.55, but 0.55 starves END dots of sub-pixel coverage (G-delta ≈10
                             // vs body < the +12 visibility bar); 0.75 keeps the ~1.5px dot while the
                             // env+peak-emphasis ramp carries edge→mid in height.
/// Center emphasis gaussian σ² in elements² (C4): ref cluster ≈ 26–28% of W
/// over 27 bars ⇒ σ ≈ 4.35 elements ⇒ σ² ≈ 19. Gain 1.0: the bump adds up to
/// +1.0 to the field (broad mound), not a thin spike.
const PEAK_SIGMA2: f32 = 19.0;
const SHEEN: f32 = 10.0; // top-left body sheen, +RGB
const RIM: (f32, f32, f32) = (20.0, 24.0, 27.0); // ~1px teal-white edge rim (~9%)
const SPEC: f32 = 26.0; // top-edge specular line strength, added per channel
/// Glass crease (C12, ref-measured replacement for the old keyline): the
/// reference top edge is a soft DARK line — lum dips to ≈0.6× of the
/// linear AA blend just inside the edge. Implemented as a subtractive band
/// weighted by the same rim_w edge distance, fading in with grow.
const CREASE: f32 = 14.0;
/// Drop shadow (C12, ref-measured R1): tint rgb(10,20,35) at 35% peak alpha,
/// offset (−2.25, +1.5) logical px (light from upper-right), exponential
/// falloff λ = 7.2px (half-decay ≈ 5px, visible ≈ 18px below).
const SHADOW2_A: f32 = 92.0;
const SHADOW2_OFF: (f32, f32) = (-2.25, 1.5);
const SHADOW2_LAMBDA: f32 = 7.2;
const SHADOW2_TINT: (f32, f32, f32) = (10.0, 20.0, 35.0);

/// Measured palette (verbatim from the reference spec).
const BG: (f32, f32, f32) = (16.0, 20.0, 30.0); // near-black navy cast (dark desktop)
const TWIN: (f32, f32, f32) = (31.0, 30.0, 29.0); // #1F1E1D clock-pill twin
const GRAD_L: (f32, f32, f32) = (125.0, 165.0, 165.0); // teal, left end
const GRAD_C: (f32, f32, f32) = (140.0, 220.0, 222.0); // peak cyan, center
const GRAD_R: (f32, f32, f32) = (100.0, 130.0, 170.0); // blue, right end

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

/// Coverage for a wave element / leader dot (tight band — see ELEM_AA).
#[inline]
fn elem_cov(d: f32) -> f32 {
    smooth(clamp01(0.5 - d / (2.0 * ELEM_AA)))
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
    gw: f32,  // bloom weight: 0 below v = 0.35, smoothstep up to 1 (idle hum at low v)
    gk: f32,  // bloom kernel width (GLOW_K scaled by height), device px
    cr: f32,  // gradient color at this element's x position,
    cg: f32,  // pre-scaled by height brightness and shimmer
    cb: f32,
    u: f32,   // element position along the span (0..1) — pre-hot halo color sampling
    dim: f32, // whisper-ramp dim factor at this element (scales halo too)
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

    // Entrance spring: 0.0 collapsed clock-pill twin, 1.0
    // full pill, up to 1.25 overshoot. Width leads (quadratic ease), height
    // lags (squared ease): liquid squash-stretch, both bulge on overshoot.
    let gr = if grow.is_finite() {
        grow.clamp(0.0, GROW_MAX)
    } else {
        0.0
    };
    let width_ease = if gr <= 1.0 {
        1.0 - (1.0 - gr) * (1.0 - gr)
    } else {
        1.0 + (gr - 1.0) * 0.45
    };
    let height_ease = if gr <= 1.0 {
        gr * gr
    } else {
        1.0 + (gr - 1.0) * 0.30
    };
    // Waveform fades in mid-bounce: 0 below grow 0.25, 1 from grow 0.7.
    let wave_op = smooth(clamp01((gr - 0.25) / 0.45));

    let cx = width as f32 * 0.5;
    // grow=0 must BE the clock pill: 76×24 logical (half-extents 38/12) at
    // world y=15 (surface y 3..27). grow=1 lands on the compact 148×36 island
    // pill (half-extents 74/18) at y=21. Overshoot g=1.25 → half-width ~82,
    // half-height ~19.4, cy 22.5 — footprint ≤ 164 wide ≤ 188 core, bottom
    // + shadow ≤ 59 < 64: nothing clips in the 220×64 surface.
    let hw_c = (38.0 + 36.0 * width_ease) * s;
    // Height lags width (squared ease) — wider arrives before taller.
    let hh_c = (12.0 + 6.0 * height_ease) * s;
    // The capsule grows out of the collapsed clock pill (y=15) and lands
    // hugging the top edge (y=21); overshoot dips below rest first.
    let cy = (15.0 + 6.0 * gr) * s;
    let rad = hh_c; // stadium: radius = core height / 2
    let reach = GLOW_REACH * s;

    // Precompute bar capsules + leader dots once per frame — nothing allocates
    // in the pixel loop.
    let mut caps: Vec<Bar> = Vec::with_capacity(bars.len());
    let mut dots: Vec<(f32, f32)> = Vec::new(); // (center x, radius) per leader dot
    let mut flank_lo = 0.0f32; // fast-path x-range (distance from cx) holding dots
    let mut flank_hi = 0.0f32;
    let mut hot_g = 0.0f32; // hot-core G (scaled) — caps the leader-dot blue bias
    if !bars.is_empty() && wave_op > 0.0 {
        let n = bars.len() as f32;
        // The waveform occupies the central span; leader dots flank it out
        // toward the capsule ends.
        let bar_span = BAR_SPAN_FRAC * hw_c;
        let slot = (2.0 * bar_span) / n;
        // Bars keep a little breathing room from the pill's top/bottom edge:
        // 24px bar height inside the 36px grown pill.
        let inner_h = (2.0 * hh_c - 12.0 * s).max(ELEM_R_MAX * 2.0 * s);
        for (i, &v) in bars.iter().enumerate() {
            let v = if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Idle elements stay visible as small dim dots (never zero height).
            // Center-weight envelope with a 0.75 floor (C5, ref-measured):
            // heights taper from the middle to the edges, peak:flank ≈ 7:1
            // comes from the peak gaussian, not a deep env valley.
            let env = 0.75 + 0.25 * (PI * ((i as f32 + 0.5) / n)).sin();
            // Peak emphasis (C4, ref-measured): broad gaussian over the
            // center (σ² ≈ 19 elements², gain 1.0) lifts a ~26%-of-W mound,
            // matching the reference's fused lit cluster. Capped to 1.0 so
            // the hot-snap threshold sees a clean field.
            let dc = (i as f32 + 0.5) - n * 0.5;
            let pk = 1.0 + (-(dc * dc) / PEAK_SIGMA2).exp();
            let raw_field = env * pk;
            // Soft knee retains field ordering above 1.0 instead of flattening
            // every central slot into one equal-height plateau.
            let field = if raw_field <= 1.0 {
                raw_field
            } else {
                1.0 + (raw_field - 1.0) * 0.20
            };
            let half_h = ((v * inner_h * 0.5).max(DOT_R_MIN * s) * field).min(inner_h * 0.5);
            // ONE element kind: width scales with height, 0.55→1.0 logical px —
            // a ~1.1px dot at idle, a ~2px bar when tall; caps fully rounded.
            let r = (ELEM_R_MAX * s).min(half_h).max(DOT_R_MIN * s);
            let bcx = cx - bar_span + slot * (i as f32 + 0.5);
            // Brightness: steep curve — dots stay dim, peaks reach the top of
            // the palette (capped at 1.0 so cores never overdrive the stops).
            let bright = (0.45 + 0.65 * v.powf(0.6)).min(1.0);
            let u = i as f32 / (n - 1.0).max(1.0);
            let (cr, cg, cb) = wave_color(u);
            // Hot snap (C1, ref-measured): bars whose field reaches ≈1.0
            // are SOLID hot teal rgb(171,239,238) — the reference's hottest
            // pixel is teal with R/G 0.72 and never bleaches white, so there
            // is no white plateau. The snap ramps in over field 0.90..1.0.
            let wmix = smooth(clamp01((field - 0.90) / 0.099));
            let hr = GRAD_C.0 + (171.0 - GRAD_C.0) * wmix;
            let hg = GRAD_C.1 + (234.0 - GRAD_C.1) * wmix;
            let hb = GRAD_C.2 + (233.0 - GRAD_C.2) * wmix;
            // Leader-zone dimming: the bars carry the SAME mound envelope
            // as their height (ref R3: bar core G tracks the cluster mound,
            // 52–100% of peak, not the 25% tail of the dot line). The
            // whisper ramp (C9/R2) is the LEADER-DOT curve only (measured on
            // the 36-dot line), so applying it here would double-dim the
            // flanks below the reference. `field` is already the height
            // factor, so colour brightness stays proportional to height.
            // The soft knee is geometry-only: colour must stay at/below the
            // palette stop, otherwise a >1 field overdrives the core to white.
            let dim = field.min(1.0);
            // C8 tail blue: the right flank's blue channel is weighted up so
            // the tail reads steel-blue (ref mid-height B−G ≈ +8, end dots
            // ≈ +30), while the left flank stays teal.
            let fpos = if u <= 0.5 {
                1.0 - u * 2.0
            } else {
                (u - 0.5) * 2.0
            };
            let bluew = if u > 0.5 { 1.0 + BLUE_BIAS * fpos } else { 1.0 };
            // Slow per-element shimmer (brighten-only, 1.0..1.10) keeps a
            // constant input alive without distorting the palette stops.
            let shimmer = 1.0 + 0.01 * (1.0 - (t * 3.2 + i as f32 * 1.05).cos());
            // Bloom gate: dots emit nothing; emission ramps in from v = 0.35.
            // Idle hum (C11): the peak-emphasis cluster keeps a fraction of
            // its halo alive at near-zero v (dim teal hum on the posed
            // silence; the breathing signal arrives through v).
            let gw = smooth(clamp01((v - 0.35) / 0.65))
                .max(GLOW_IDLE * smooth(clamp01((field - 0.75) / 0.249)) * (1.0 - v));
            // Two-scale kernel: tall bars get wide hot halos, dim elements
            // stay tight.
            let gk = GLOW_K * (0.35 + 0.65 * v) * s;
            caps.push(Bar {
                cx: bcx,
                r,
                seg: (half_h - r).max(0.0),
                gw,
                gk,
                u,
                dim,
                // Clamp to 255: hot teal × shimmer could exceed the channel,
                // breaking the premultiplied invariant at master_alpha < 1.
                // The whisper ramp scales the EXCESS over the body, never the
                // body itself, and the excess is floored (C9) — otherwise a
                // 0.14 tail fraction multiplies the element below the body
                // colour and tail/idle dots vanish. The right flank's blue
                // channel is weighted up (C8) before the ramp.
                cr: (BG.0
                    + (((cr * (1.0 - wmix) + hr * wmix) * bright * shimmer - BG.0) * dim)
                        .max(VIS_FLOOR))
                .min(255.0),
                cg: (BG.1
                    + (((cg * (1.0 - wmix) + hg * wmix) * bright * shimmer - BG.1) * dim)
                        .max(VIS_FLOOR))
                .min(255.0),
                cb: (BG.2
                    + (((cb * (1.0 - wmix) + hb * wmix) * bright * shimmer * bluew - BG.2) * dim)
                        .max(VIS_FLOOR))
                .min(255.0),
                x0: ((bcx - r - reach).floor() as i32).max(0),
                x1: ((bcx + r + reach).ceil() as i32).min(width as i32),
                y0: ((cy - half_h - reach).floor() as i32).max(0),
                y1: ((cy + half_h + reach).ceil() as i32).min(height as i32),
            });
        }
        // Dotted leader line: the CONTINUATION of the bar grid at the same
        // ≈3.7px pitch (C7) — one slot pitch past the outermost bar, running
        // out toward each capsule end to hw−15px (C3: ref line span ≈ 79–80%
        // of W with margins ≈ 8% / 12.6%). Every slot renders (C6: no
        // dropouts); the whisper ramp (C9) carries the brightness.
        let dr = (DOT_R * s).max(0.45);
        let edge = bar_span - slot * 0.5 + ELEM_R_MAX * s; // outermost element's outer edge
        flank_lo = edge + 0.5 * s;
        flank_hi = hw_c - 0.5 * s;
        let mut off = edge + slot * 0.5 + DOT_STEP * s; // one slot pitch past
                                                        // the outermost bar center: the line literally continues the grid.
        let end = hw_c - 15.0 * s;
        while off + dr < end {
            dots.push((cx - off, dr));
            dots.push((cx + off, dr));
            off += DOT_STEP * s;
        }
        // C8 reference: end-dot blue bias is measured against the hot core
        // (end dot B ≤ 0.72·peak G); carried to the pixel loop via hot_g.
        hot_g = 239.0 * s;
    }

    for y in 0..h {
        let py = y as f32 + 0.5;
        let dy_top = py - (cy - hh_c); // 0 at the pill's top edge
        for x in 0..w {
            let px = x as f32 + 0.5;
            let d = sd_rounded_rect(px, py, cx, cy, hw_c, hh_c, rad);
            // Drop shadow (C12, ref-measured R1): silhouette offset (−2.25,
            // +1.5)px (light from upper-right), exponential λ=7.2px falloff,
            // tinted dark rgba(10,20,35) at 35% — pools below and toward the
            // lower-left. Fades in with grow like the body.
            let ds = sd_rounded_rect(
                px - SHADOW2_OFF.0 * s,
                py - SHADOW2_OFF.1 * s,
                cx,
                cy,
                hw_c,
                hh_c,
                rad,
            );
            let st = (-ds.max(0.0) / (SHADOW2_LAMBDA * s)).exp();
            let sh_in = smooth(clamp01(gr / 0.15)); // entrance fade-in
                                                    // R1 masks: the shadow pools BELOW the capsule (nothing above the
                                                    // mid-line) and reaches ~10px left / ~6px right of the silhouette
                                                    // (light from upper-right), so tile corners and side flanks stay
                                                    // clean instead of washing out with the isotropic falloff.
            let below = clamp01((py - cy - 2.0 * s) / hh_c);
            let sdx = px - cx;
            let side = (hw_c + (if sdx < 0.0 { 10.0 } else { 6.0 }) * s - sdx.abs()) / (6.0 * s);
            // Shadow color = measured dark-navy tint (non-premultiplied; the
            // tail premultiplies at write time).
            let mut r = SHADOW2_TINT.0;
            let mut g = SHADOW2_TINT.1;
            let mut b = SHADOW2_TINT.2;
            let mut a = st * SHADOW2_A * sh_in * below * clamp01(side);

            // Pill body: near-black with a dark navy cast (rgb 16,20,30) so
            // the capsule separates from the dark navy desktop, a subtle +10
            // sheen at the top-left fading darker rightward, a ~1px teal-white
            // rim hugging the AA edge (~9%), and a soft DARK crease just
            // inside the top edge (C12: the reference has no bright keyline —
            // its top edge is a soft dark line, lum ≈0.6× the AA blend). While
            // collapsed (grow < 0.15) it relaxes to a flat #1F1E1D twin of the
            // clock pill — no rim, no crease — blending out of the flat zone
            // as grow rises.
            let cov = sdf_cov(d);
            if cov > 0.0 {
                let twin = smooth(clamp01((0.15 - gr) / 0.15)); // 1 when collapsed
                let flat = 1.0 - twin;
                let sheen = SHEEN
                    * (1.0 - clamp01((px - (cx - hw_c)) / (2.0 * hw_c)))
                    * (1.0 - clamp01(dy_top / (2.0 * hh_c)));
                // Rim: brightest at the silhouette, gone ~1px inward.
                let rim_w = clamp01(1.0 + d / s);
                // Specular roll-off: ~2px bright line at the top, fading
                // over ~6px.
                let spec_w = smooth(clamp01(1.0 - dy_top / (6.0 * s)));
                // Dark crease: a few px inside the top edge, subtractive,
                // weighted by edge proximity like the rim.
                let crease_w = rim_w * smooth(clamp01(1.0 - dy_top / (5.0 * s)));
                let br = TWIN.0 * twin
                    + (BG.0 + sheen + RIM.0 * rim_w + SPEC * spec_w - CREASE * crease_w).max(0.0)
                        * flat;
                let bg = TWIN.1 * twin
                    + (BG.1 + sheen + RIM.1 * rim_w + SPEC * spec_w - CREASE * crease_w).max(0.0)
                        * flat;
                let bb = TWIN.2 * twin
                    + (BG.2 + sheen + RIM.2 * rim_w + SPEC * spec_w - CREASE * crease_w).max(0.0)
                        * flat;
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
                    let bcov = elem_cov(db);
                    if bcov > bcov_max {
                        bcov_max = bcov;
                    }
                    if bcov > 0.0 {
                        (r, g, b, a) =
                            over(r, g, b, a, bar.cr, bar.cg, bar.cb, 255.0 * bcov * wave_op);
                    }
                    if db < reach {
                        // Max-field bloom: keep only the strongest source at
                        // this pixel, so 27 bars at ~3.7px pitch cannot sum
                        // into one fused sheet — gaps read as gaps.
                        let w = bar.gw * (-db.max(0.0) / bar.gk).exp();
                        if w > glow {
                            glow = w;
                            // C2: sample the PRE-hot teal ramp (with the
                            // element's own dim factor), NOT the snapped hot
                            // core — the halo must stay teal (R/G ≈ 0.6),
                            // never pick up the hot tip color.
                            let (hr, hg, hb) = wave_color(bar.u);
                            glow_r = hr * bar.dim;
                            glow_g = hg * bar.dim * GLOW_TEAL;
                            glow_b = hb * bar.dim * GLOW_TEAL;
                        }
                    }
                }
            }

            // Additive bloom in the strongest source's own color, masked to
            // the pill interior and lifted from under the cores. The field is
            // capped at 1.0 before the gains so stacked sources cannot
            // saturate a pixel.
            if glow > 0.0 {
                let gi = glow.min(1.0) * pulse * cov * (1.0 - bcov_max) * wave_op;
                r += glow_r * BLOOM * gi;
                g += glow_g * BLOOM * gi;
                b += glow_b * BLOOM * gi;
                a += GLOW_A * gi;
            }

            // Dotted leader line (C6/C8/C9): dim dots CONTINUING the bar
            // grid, masked to the capsule interior. Color = same gradient
            // family at the dot's x, dimmed by the SAME whisper ramp the bars
            // use (so the ramp is one continuous curve), with a blue bias on
            // the right flank that grows toward the end (C8: end-dot B−G ≥
            // +30; ref end dot rgb(51,60,72)).
            if !dots.is_empty() {
                let adx = (px - cx).abs();
                if adx > flank_lo && adx < flank_hi {
                    for &(dx, dr) in &dots {
                        if (px - dx).abs() > dr + ELEM_AA {
                            continue; // fast path: outside this dot's AA reach
                        }
                        let dcov = elem_cov(sd_capsule(px, py, dx, cy, 0.0, dr));
                        if dcov > 0.0 {
                            let u = 0.5 + 0.5 * (px - cx) / cx;
                            let (or_, og, ob) = wave_color(u);
                            let fpos = if u <= 0.5 { u * 2.0 } else { (u - 0.5) * 2.0 };
                            let d = if u <= 0.5 {
                                whisper(&WHISPER_L, fpos)
                            } else {
                                whisper(&WHISPER_R, fpos)
                            };
                            // Tail blue (C8): right-flank B climbs toward the
                            // end so B−G ≥ +30 even at the last dot; left
                            // flank keeps the teal ramp (B ≈ G).
                            let blue = if u > 0.5 {
                                1.0 + DOT_BLUE_BIAS * fpos
                            } else {
                                1.0
                            };
                            // Same excess-ramp + visibility floor as the bars
                            // (C9): the whisper fraction dims the leader dot
                            // toward the body but never below it.
                            let or_ = BG.0 + ((or_ - BG.0) * d).max(VIS_FLOOR);
                            let og = BG.1 + ((og - BG.1) * d).max(VIS_FLOOR);
                            let ob =
                                (BG.2 + ((ob * blue - BG.2) * d).max(VIS_FLOOR)).min(hot_g * 0.72);
                            (r, g, b, a) =
                                over(r, g, b, a, or_, og, ob, DOT_A * dcov * cov * wave_op);
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

    // 27 bars: middle bar (index 13) is centered exactly at x = 110 at scale 1.
    const MID_X: u32 = 110;
    /// Element count used by the generic wave tests (the pipeline's NUM_BARS;
    /// spelled locally so the canvas layer stays len-agnostic).
    const N27: usize = 27;
    /// grow=1 capsule half-width: 74 logical px (148×36 compact pill).
    const HW1: f32 = 74.0;
    /// Flat bar array with a single tall element at `idx` (mirrored center
    /// index 13 for 27 bars).
    fn one_hot(n: usize, idx: usize) -> Vec<f32> {
        let mut v = vec![0.0; n];
        v[idx] = 1.0;
        v
    }
    /// Element-center x at scale 1 for n bars: central span 0.68·74 around
    /// x=110, slot = 2*span/n, centers at slot*(i+0.5) from the left end.
    fn cx_of(i: usize, n: usize) -> f32 {
        let span = 0.68f32 * HW1;
        let slot = 2.0 * span / n as f32;
        110.0 - span + slot * (i as f32 + 0.5)
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
            (MID_X, 0),
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
        // ~4px below the pill core bottom edge (core bottom at y = 39).
        let near = alpha(&c, MID_X, 43);
        let far = alpha(&c, MID_X, 48);
        assert!(near > 0, "shadow missing just below the pill");
        assert!(near < 235, "shadow {near} must be softer than interior 235");
        assert!(
            far < near,
            "shadow must fall off with distance: {far} >= {near}"
        );
    }

    #[test]
    fn glow_falls_off_with_distance() {
        // Center bar (13) at value 0.5: core half-height 6 logical px (scale
        // 1), so cy=21 and the core top is y=15. Reference R5 puts the halo
        // reach at ~3–4px above the tip, so the measurable band is y=9..14:
        // y=14 ≈ +12 G over body, y=9 ≈ body. Sample inside that band.
        let c = frame(&[0.5; N27], 0.0, 1.0);
        let g_near = chan(&c, MID_X, 14, 1); // ~1px above the capsule end
        let g_far = chan(&c, MID_X, 9, 1); // ~6px above the capsule end
        assert!(
            g_near > g_far + 8,
            "bloom must decay: near {g_near} vs far {g_far}"
        );
    }

    #[test]
    fn taller_bar_covers_more_pixels_at_same_x() {
        // Count rows at the center bar's column that are clearly emissive
        // (G > 150: body+sheen tops out ≈37, white-hot cores are near-neutral
        // so the old G > R+25 hue predicate cannot see them).
        let count = |c: &[u8]| -> u32 {
            let mut n = 0;
            for y in 0..H {
                if chan(c, MID_X, y, 1) > 150 {
                    n += 1;
                }
            }
            n
        };
        let low = count(&frame(&[0.3; N27], 0.0, 1.0));
        let high = count(&frame(&[0.9; N27], 0.0, 1.0));
        assert!(
            high > low + 7,
            "taller bar must cover more rows: {high} vs {low}"
        );
    }

    #[test]
    fn capsule_peaks_reach_bright_teal() {
        // Argmax of the green channel inside the body: mid-range bars must
        // already show clearly bright peaks (>= 180) ...
        let scan = |c: &[u8]| -> u8 {
            let mut m = 0u8;
            for y in 4..40 {
                for x in 36..(W - 36) {
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
        // Slot layout at g=1 (hw_c = 74): span 0.68·74 ≈ 50.3, slot ≈ 3.7,
        // element centers at x = 110 - 50.3 + slot·(i+0.5); sampled at y = 21
        // (wave centerline).
        let c = frame(&[1.0; N27], 0.0, 1.0);
        let cx_at = |i: u32| -> u32 { cx_of(i as usize, N27).round() as u32 };
        // At the compact pitch (≈3.7px) the ±1.25px AA band and bloom from
        // neighbors widen the tolerance; the sheen lifts the left end.
        let tol = 30i32;
        // Probed values at the compact pitch (220-wide pill, v=1). Re-anchored
        // to the hot-teal macro (no white-hot tip per ref-deep-measure): left
        // end (86,116,122), center (173,241,240), right end (83,107,182).
        let (r, g, b) = (
            chan(&c, cx_at(0), 21, 2) as i32,
            chan(&c, cx_at(0), 21, 1) as i32,
            chan(&c, cx_at(0), 21, 0) as i32,
        );
        assert!((r - 86).abs() <= tol, "left dimmed teal R {r} vs ~86");
        assert!((g - 116).abs() <= tol, "left dimmed teal G {g} vs ~116");
        assert!((b - 122).abs() <= tol, "left dimmed teal B {b} vs ~122");
        // Center (i=13, x=110): hot teal peak rgb(173,241,240); ref max G 242.
        let (r, g, b) = (
            chan(&c, MID_X, 21, 2) as i32,
            chan(&c, MID_X, 21, 1) as i32,
            chan(&c, MID_X, 21, 0) as i32,
        );
        assert!((r - 172).abs() <= tol, "center teal R {r} vs 172");
        assert!((g - 236).abs() <= tol, "center teal G {g} vs 236");
        assert!((b - 235).abs() <= tol, "center teal B {b} vs 235");
        // Right end (i=26): dimmed blue-steel, lifted by edge bloom.
        let (r, g, b) = (
            chan(&c, cx_at(26), 21, 2) as i32,
            chan(&c, cx_at(26), 21, 1) as i32,
            chan(&c, cx_at(26), 21, 0) as i32,
        );
        assert!((r - 83).abs() <= tol, "right dimmed blue R {r} vs 83");
        assert!((g - 107).abs() <= tol, "right dimmed blue G {g} vs 107");
        assert!((b - 182).abs() <= tol, "right dimmed blue B {b} vs 182");
    }

    #[test]
    fn body_bg_is_navy_near_black_not_green_gray() {
        // Dither jitters single pixels ±1 per channel independently, so the
        // navy cast is asserted on patch MEANS (dither is zero-mean).
        // Interior body away from the sheen (right half) and away from wave
        // rows' verticals: must average ≈ rgb(16,20,30), b highest (navy).
        // Patches sit inside the compact capsule (220×64; hull x≈72..147).
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
            (patch(x0, y0, 2), patch(x0, y0, 1), patch(x0, y0, 0))
        };
        for &(x0, y0) in &[(150u32, 10u32), (168, 14), (52, 30)] {
            let (r, g, b) = mean(x0, y0);
            assert!(
                (r - 16.0).abs() <= 1.5 && (g - 20.0).abs() <= 1.5 && (b - 30.0).abs() <= 1.5,
                "bg patch ({x0},{y0}) = rgb({r},{g},{b}), want ≈ rgb(16,20,30)"
            );
            // The palette's own cast: b ≥ g ≥ r (navy). A green-gray body
            // would show g ≥ b.
            assert!(b >= g && g >= r, "navy cast violated: rgb({r},{g},{b})");
        }
        // Top-left sheen adds ~+10: still navy, never green-gray (g > b is
        // the green-gray tell — forbid it). Patch sits ~6px inside the
        // compact capsule's upper-left interior (probed ≈ 22.7,26.7,36.7:
        // sheen + a little rim/specular bleed at the top edge).
        let (r, g, b) = mean(48, 8);
        assert!(b > g, "sheen must stay navy: rgb({r},{g},{b})");
        assert!(
            r > 20.0,
            "top-left sheen missing: R {r} not above the 20 floor"
        );
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
        // Bottom of the compact capsule: core spans y≈3..26; shadow falls off
        // below y≈35. Probe 4px under the fresh hull bottom (~y=26).
        assert!(alpha(&c, MID_X, 43) > 0, "shadow missing below the capsule");
        // Same offset OUTSIDE the left silhouette (x=20 < 72): no halo.
        assert_eq!(alpha(&c, 20, 43), 0, "side shadow halo leaked left");
        // Mirror point above the silhouette: y=1, nothing there.
        assert_eq!(alpha(&c, MID_X, 1), 0, "shadow leaked above the capsule");
        // The left flank of the compact pill (x=20, mid-height): clean.
        assert_eq!(alpha(&c, 20, 21), 0, "side halo leaked at mid height");
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
        // not vanish. Idle cores render at 45% brightness and the leader-zone
        // falloff dims the tails: center dot G≈85, tail dots G≈36–43 vs
        // body-only G≤24 at the spot. The visibility bar stays "clearly
        // brighter than bare body" (≥ +12 G; re-anchored: sub-pixel coverage
        // of the ~1.5px dots limits the end-dot delta to ≈ +13).
        let dots = frame(&[0.0; N27], 0.0, 1.0);
        let body = frame(&[], 0.0, 1.0);
        for i in 0..N27 {
            // Exact element center the renderer uses: waveform spans the central
            // 68% of the pill core, slot = 2*0.68*160/27. The ~1.5px dots sit
            // up to 0.5px off the rounded center, so sample a ±1px max window
            // (a point sample below the AA peak is not evidence of absence).
            let x = cx_of(i, N27).round() as u32;
            let wmax = |c: &[u8]| -> u8 {
                let lo = x.saturating_sub(1);
                let hi = (x + 1).min(W - 1);
                (lo..=hi).map(|xx| chan(c, xx, 21, 1)).max().unwrap()
            };
            let gd = wmax(&dots);
            let gb = wmax(&body);
            assert!(
                gd >= 30 && gd > gb + 12,
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
        // pixel of the v=1.0 element (index 1 of 5 over the compact span;
        // center x = cx_of(1,5) ≈ 90) must be opaque and in range.
        let (b, g, r, a) = (
            chan(&c, 90, 21, 0),
            chan(&c, 90, 21, 1),
            chan(&c, 90, 21, 2),
            chan(&c, 90, 21, 3),
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
        let bright =
            |c: &[u8]| -> u32 { c.iter().skip(3).step_by(4).filter(|&&a| a > 8).count() as u32 };
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
        // The collapsed twin is the compact clock pill at rest: center
        // (MID_X, 15), half-extents 38×12 (hull y≈3..26, center 14.5 → 15).
        // Probe its silhouette edges directly: half-width 38 → ±38 from cx,
        // half-height 12 → ±12 from cy. Tolerance ±0.6 logical px.
        let c = frame_g(&[0.5; N27], 0.0, 1.0, 0.0);
        let cy0 = 15.0; // twin mid-row
        let cy0i = 15u32; // twin mid-row
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
        // (130, 21): inside the twin (half-extents 38×12 around (110,15)) at
        // both grows, away from edges and the sheen corner.
        let cr = chan(&collapsed, 130, 21, 2) as i32;
        let cg = chan(&collapsed, 130, 21, 1) as i32;
        let cb = chan(&collapsed, 130, 21, 0) as i32;
        assert!(
            (cr - 31).abs() <= 2 && (cg - 30).abs() <= 2 && (cb - 29).abs() <= 2,
            "grow-0 body not #1F1E1D: rgb({cr},{cg},{cb})"
        );
        let mr = chan(&mid, 130, 21, 2) as i32;
        let mg = chan(&mid, 130, 21, 1) as i32;
        let mb = chan(&mid, 130, 21, 0) as i32;
        // Blend mid-point must have moved to the capsule navy bg rgb(16,20,30)
        // (probed rgb(17,21,31)): b highest, and G has DROPPED from the twin's
        // warmer 30 — the old green-gray capsule bg is gone.
        assert!(
            (mr - 17).abs() <= 2 && (mg - 21).abs() <= 2 && (mb - 31).abs() <= 2,
            "blended body not navy: rgb({mr},{mg},{mb})"
        );
        assert!(
            mb > mg && mg < cg,
            "body must blend toward the navy capsule bg as grow rises: {mr},{mg},{mb}"
        );
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
        // Waveform occupies the central 68% of the compact capsule
        // (x ≈ 60..160); dots live outside that span but inside the capsule
        // (hull edges ≈ 72/147): probe the flanks at the wave centerline.
        let mut wave_dots = 0u32;
        for x in 34..60 {
            // Gradient-family dots: G ≥ R and B between them (teal→blue), or
            // plainly brighter than the body.
            let (b, g, r, a) = (
                chan(&c, x, 21, 0),
                chan(&c, x, 21, 1),
                chan(&c, x, 21, 2),
                alpha(&c, x, 21),
            );
            if a > 20 && g > r + 10 && b >= 37 {
                wave_dots += 1;
            }
        }
        assert!(
            wave_dots >= 2,
            "left leader line missing: only {wave_dots} wave-family px at row 21"
        );
        let mut wave_dots = 0u32;
        for x in 158..186 {
            let (b, g, r) = (chan(&c, x, 21, 0), chan(&c, x, 21, 1), chan(&c, x, 21, 2));
            if alpha(&c, x, 21) > 20 && g > r + 10 && b >= 37 {
                wave_dots += 1;
            }
        }
        assert!(
            wave_dots >= 2,
            "right leader line missing: only {wave_dots} wave-family px at row 21"
        );
        // ... and they are absent while collapsed.
        let c = frame_g(&[0.5; N27], 0.0, 1.0, 0.0);
        for x in 74..146 {
            assert!(
                chan(&c, x, 21, 1) < 70,
                "no dots may show at grow=0 (x={x}: G={})",
                chan(&c, x, 21, 1)
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
        // 220×64 surface. Measure the α>100 hull at scale 2 (440×128); the
        // drop shadow (α ≤ 64) is excluded by the threshold, so this is
        // the capsule silhouette only (probed: x 72..367, y 6..78).
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
            (fw - 296.0).abs() <= 6.0,
            "grown capsule width {fw} @2x, want ≈296 (148 logical)"
        );
        assert!(
            (fh - 73.0).abs() <= 4.0,
            "grown capsule height {fh} @2x, want ≈73 (36.5 logical)"
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
            // Nothing may clip: the grown capsule stays inside the compact
            // surface (26px side pads; top may hug the shadow band, bottom
            // stays clear).
            assert!(
                left >= 26 && right <= W as i32 - 26,
                "grow {grow} clips horizontally: hull {left}..{right} in 0..{W}"
            );
            assert!(
                top >= 2 && bottom <= H as i32 - 2,
                "grow {grow} clips vertically: hull {top}..{bottom} in 0..{H}"
            );
        }
    }

    #[test]
    fn bloom_half_excess_radius_matches_reference_halo() {
        // Spec: bloom halo r≈15px at the 676px reference capsule width;
        // rescaled to our capsule that is ≈3px logical (≈6.5px @2x) of
        // half-excess below a tall bar's core bottom (±30%). Render a one-hot
        // tall-bar frame at scale 2 and measure the VERTICAL excess-over-bg
        // profile downward from the tallest bar's core, at the bar's center
        // column (clear of the idle-dot / leader-dot row). Baseline is a
        // zero-bars frame (empty bars would add idle dots at that column).
        let n = crate::pill::vecmentor::NUM_BARS;
        let (w, h) = (SURF_W * 2, SURF_H * 2);
        let mut bars = vec![0.0; n];
        bars[n / 2] = 1.0;
        let mut big = vec![0u8; (w * h * 4) as usize];
        render_pill(&mut big, w, h, &bars, 0.0, 1.0, 1.0);
        let mut bg = vec![0u8; (w * h * 4) as usize];
        render_pill(&mut bg, w, h, &vec![0.0; n], 0.0, 1.0, 1.0);
        let g = |c: &[u8], y: u32| c[((y * w + 220) * 4 + 1) as usize] as f32;
        // Tall bar core bottom @2x: last row where the bar's own saturated
        // body holds G>200 (probed: row 65). The column argmax would instead
        // land on the bloom-saturated upper segment near the top of the bar.
        let mut core_bottom = 0u32;
        for y in 0..h {
            if g(&big, y) > 200.0 {
                core_bottom = y;
            }
        }
        let excess = |y: u32| g(&big, y) - g(&bg, y);
        let v0 = excess(core_bottom + 2); // first sample clear of the core AA band
        assert!(
            v0 > 40.0,
            "tall-bar bloom must be strong below the core: {v0}"
        );
        let mut r = 0.0f32;
        for y in (core_bottom + 3)..h {
            let v = excess(y);
            if v <= v0 * 0.5 {
                let vp = excess(y - 1);
                let frac = ((vp - v0 * 0.5) / (vp - v0).max(1e-6)).clamp(0.0, 1.0);
                r = (y - 1 - core_bottom) as f32 + frac;
                break;
            }
        }
        assert!(
            (4.0..=14.0).contains(&r),
            "bloom half-excess {r}px @2x, want ≈6.5 (spec 15px at the 676px reference width)"
        );
    }

    #[test]
    fn third_peak_colors_track_spec_stops_on_tall_bars() {
        // Third-peak sampling on the GROWN capsule: tall bars at u ≈ 1/6 and
        // 5/6 must land within dRGB ≤ 60 of the palette stop at their x
        // (bloom is lifted under full cores, so the core carries the exact
        // gradient); the CENTER third is the hot-teal peak (rgb ≈ 173,241,240
        // vs stop (140,220,222)) — no white tip per ref-deep-measure, so it
        // asserts the hot-teal signature rather than the stop.
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
                chan(&c, x, 21, 2) as f32,
                chan(&c, x, 21, 1) as f32,
                chan(&c, x, 21, 0) as f32,
            );
            // The CENTER third is the hot-teal peak by design; outer thirds
            // carry the C4/C5 mound dim plus, on the right, the C8 blue bias,
            // so their cores sit a little off the raw stop (measured dRGB 36 at
            // u=0.154 and 65 at u=0.846). 75 keeps that headroom.
            if k == 1 {
                // Center: hot teal — G-dominant, R/G ≈ 0.72, B at/below G.
                assert!(
                    g >= 225.0 && r < g && b <= g + 3.0 && r / g >= 0.70,
                    "center third not hot teal: rgb({r},{g},{b})"
                );
            } else {
                let d = (r - er).abs() + (g - eg).abs() + (b - eb).abs();
                assert!(
                    d <= 75.0,
                    "third {k} (i={i}, u={u:.3}): rgb({r},{g},{b}) vs stop ({er},{eg},{eb}), dRGB {d}"
                );
            }
            gs[k] = g;
        }
        assert!(
            gs[0] < gs[1] && gs[2] < gs[1],
            "gradient must read teal→hot-teal→blue: g at thirds = {:?}",
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
        let probes = [(110, 21, 220, 42), (48, 21, 96, 42), (110, 47, 220, 94)];
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
