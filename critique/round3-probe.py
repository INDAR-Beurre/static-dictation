#!/usr/bin/env python3
"""Round-3 pill acceptance probe — self-contained PIL/numpy audit.

Usage:
    python3 critique/round3-probe.py [crop.png ...]

Given a pill crop PNG (880x190 = 2x upscale of 440x95 physical @ display
scale 1.5; scale is auto-detected from the capsule bbox), computes ALL
round-3 acceptance numbers (critique/round3-brief.md) in one run and
prints a PASS/FAIL table.

Region definitions (tuned on critique/round2-active.png, see caveats in
the validation report):
  * capsule bbox: autodetected — host navy is B-G>=28 & lum 28..90,
    outside is lum<8; capsule = the x-run of columns whose longest
    vertical non-host/non-black run is >= 40% of image height, then the
    y-band where >=60% of that x-span is capsule. Override with env
    PILL_BBOX="x0,y0,x1,y1" (inclusive, crop px) if autodetect misses.
  * body RGB  = median of px inside bbox with lum<40 & B-G<14.
  * rim       = 2px inset ring; interior = bbox shrunk 12px each side.
  * host navy = median of px with B-G>=30 & 30<=lum<=90.
  * waveform band = rows centered on capsule vertical middle, height 40%
    of capsule H, clamped to bbox; per-column max G builds the profile.
  * "runs"    = columns with profile G > 45 (round-2 probe threshold).
  * midgap baseline for item 4 = Q1 of clean-gap medians (gaps >70px
    away from the peak cluster and not adjacent to it) — reproduces
    round-2's 1.09x/1.78x on round2-active.png.
Prints numbers as CROP px (and logical px = crop/scale where relevant).
"""
import os
import statistics
import sys

import numpy as np
from PIL import Image

# ---------- targets (round3-brief.md) ----------
T = {
    "plateau_px_max": 40,        # px >= lum 230
    "plateau_span_max": 5,       # rows
    "core_rg_min": 0.70,
    "halo_rg_lo": 0.57, "halo_rg_hi": 0.65,
    "gap_med_min": 3.4, "gap_min_min": 2.5,
    "gap_haze_max": 20,          # gap G lift over body
    "midgap_ratio_max": 1.05,
    "span_pct": 80.0, "margin_L_min": 8.0, "margin_R_min": 13.0,
    "cluster_pct": 26.0, "tall_cols": 12,
    "peak_flank": 7.0,
    "enddot_bg_min": 30,
    "tail_lift_lo": 17, "tail_lift_hi": 25,
    "dropouts": 0,
    "pitch": 3.7,
}
DOT_RUN_THR = 45          # round-2 run threshold on profile G
CLUSTER_FUSE = 40         # an ON run wider than this many px = fused cluster
BODY = np.array([18.0, 22.0, 32.0])


def longest_run(mask):
    best = run = 0
    for t in mask:
        run = run + 1 if t else 0
        best = max(best, run)
    return best


def autodetect_bbox(im):
    """Capsule = longest x-run of mostly-capsule columns (dark body on navy host).

    Capsule classifier: B-G < 18 & lum >= 8 (navy host, sidebars, shadow
    rows and black workspace are all blue-leaning or black; everything
    inside the capsule -- body, bars, dots, glow -- is not).  Column must
    be >= 50% capsule over the scan rows; runs broken by > 6 px gaps
    split off other UI.  Calibrated on round2-active.png -> x18..453,
    y0..97.
    """
    B, G = im[:, :, 2], im[:, :, 1]
    lum = im.mean(axis=2)
    cap = ((B - G) < 18) & (lum >= 8)
    H = lum.shape[0]
    scan_h = min(H, 112)
    colfrac = cap[:scan_h].mean(axis=0)
    xs = np.where(colfrac >= 0.4)[0]
    if len(xs) == 0:
        return None
    runs, s = [], xs[0]
    for a, b in zip(xs, xs[1:]):
        if b - a > 6:
            runs.append((s, a))
            s = b
    runs.append((s, xs[-1]))
    x0, x1 = max(runs, key=lambda r: r[1] - r[0])
    rowfrac = cap[:scan_h, x0:x1 + 1].mean(axis=1)
    ys = np.where(rowfrac >= 0.5)[0]
    if len(ys) == 0:
        return None
    return int(x0), int(x1), int(ys[0]), int(ys[-1])


def runs_of(mask):
    """[(start, end_inclusive, ...)] of True runs in a 1-D bool array."""
    out = []
    x, n = 0, len(mask)
    while x < n:
        if mask[x]:
            s = x
            while x < n and mask[x]:
                x += 1
            out.append((s, x - 1))
        else:
            x += 1
    return out


def analyze(path):
    im = np.asarray(Image.open(path).convert("RGB")).astype(int)
    R, G, B = im[:, :, 0], im[:, :, 1], im[:, :, 2]
    lum = im.mean(axis=2)
    res = {"file": path}

    bb = os.environ.get("PILL_BBOX")
    if bb:
        x0, y0, x1, y1 = (int(v) for v in bb.split(","))
    else:
        d = autodetect_bbox(im)
        if d is None:
            print("FATAL: cannot autodetect capsule bbox; set PILL_BBOX=x0,y0,x1,y1")
            sys.exit(2)
        x0, x1, y0, y1 = d
    res["bbox"] = (x0, y0, x1, y1)
    bw, bh = x1 - x0 + 1, y1 - y0 + 1
    scale = bw / 440.0  # crop px per logical px (capsule W is 440 logical)
    res["scale"] = scale

    # ---- 10. body / rim / host ----
    sub = im[y0:y1 + 1, x0:x1 + 1]
    lsub = lum[y0:y1 + 1, x0:x1 + 1]
    bsub = (B - G)[y0:y1 + 1, x0:x1 + 1]
    bodymask = (lsub < 40) & (bsub < 14)
    body = np.median(sub[bodymask].reshape(-1, 3), axis=0) if bodymask.any() else BODY
    res["body_rgb"] = body
    res["body_G"] = float(body[1])
    res["body_lum"] = float(body.mean())
    hostmask_full = ((B - G) >= 30) & (lum >= 30) & (lum <= 90)
    host = (np.median(im[hostmask_full].reshape(-1, 3), axis=0)
            if hostmask_full.any() else np.array([18.0, 34.0, 67.0]))
    res["host_rgb"] = host
    # rim = top specular edge vs body (round-2's rim +12–16 = top specular
    # band); side rims measure +2–3 and are not the scored metric.
    Wsub = lsub.shape[1]
    colmask = np.ones(Wsub, bool)
    colmask[int(0.3 * Wsub):int(0.7 * Wsub)] = False   # keep peak glow out
    rim_lum = float(np.median(lsub[0:3][:, colmask]))
    res["rim_d"] = rim_lum - res["body_lum"]
    res["spec_top"] = sub[0:3].reshape(-1, 3)[lsub[0:3].reshape(-1).argmax()]

    # ---- waveform band + profile ----
    band_h = max(8, int(round(0.40 * bh)))
    cy = (y0 + y1) // 2
    by0 = max(y0, cy - band_h // 2)
    by1 = min(y1, by0 + band_h - 1)
    band = G[by0:by1 + 1, x0:x1 + 1]
    prof = band.max(axis=0).astype(float)          # per-column max G, bbox-relative
    res["band"] = (by0, by1)

    # ---- 1. plateau census (inside bbox) ----
    hot = lsub >= 230
    n_hot = int(hot.sum())
    span = 0
    if n_hot:
        ys_, xs_ = np.where(hot)
        for y in np.unique(ys_):
            r = runs_of(hot[y])
            span = max(span, max((b - a + 1) for a, b in r))
    hot_px = sub.reshape(-1, 3)[lsub.reshape(-1).argmax()]
    res.update(plateau_px=n_hot, plateau_span=span,
               hot_rgb=hot_px, hot_rg=float(hot_px[0] / max(1, hot_px[1])))

    # ---- element runs ----
    on = prof > DOT_RUN_THR
    orun = runs_of(on)
    # fused cluster = widest ON run (round-2: the 52px fused peak cluster)
    cluster = max(orun, key=lambda r: r[1] - r[0]) if orun else (0, -1)
    ci = orun.index(cluster) if cluster in orun else -1
    pc = (cluster[0] + cluster[1]) / 2.0           # peak cluster center
    res["peakx"] = pc

    # ---- 7. peak:flank height ratio ----
    def vext(x, thr):
        col = G[y0:y1 + 1, x0 + x]
        ys = np.where(col > thr)[0]
        return int(ys.max() - ys.min() + 1) if len(ys) else 0

    thr_hi = body[1] + 40
    pk_v = vext(int(round(pc)), thr_hi)
    flank_zone = [(x, vext(x, thr_hi))
                  for x in range(x0 + 12, x1 - 12)
                  if not (cluster[0] - 12 <= x <= cluster[1] + 12)]
    flanks = [v for _, v in flank_zone if v > 0]
    fl_med = statistics.median(flanks) if flanks else 1
    res["peak_vext"] = pk_v
    res["flank_vext_med"] = fl_med
    res["peak_flank"] = pk_v / max(1, fl_med)

    # ---- 4. cluster width + tall columns + profile ----
    cl_w = cluster[1] - cluster[0] + 1
    res["cluster_px"] = cl_w
    res["cluster_pct"] = 100.0 * cl_w / bw
    # Tall columns: the crop is a 2x upscale, so both halves of a logical
    # column repeat -- count columns >= 50% peak and report logical (crop/2)
    # so the count is comparable to ref's 12.  Note: a render with coarse
    # pitch packs fewer slots into the cluster, so this is render-dependent.
    tall = [x for x in range(x0 + 12, x1 - 12)
            if vext(x, thr_hi) >= 0.50 * pk_v]
    res["tall_cols"] = len(tall)
    res["tall_cols_logical"] = len(tall) / 2.0
    prof_cl = [vext(x, thr_hi) for x in range(cluster[0], cluster[1] + 1)]
    res["cluster_profile"] = prof_cl

    # ---- gaps (off-runs between ON runs) ----
    gaps = []
    for i in range(len(orun) - 1):
        g0, g1 = orun[i][1] + 1, orun[i + 1][0] - 1
        if g1 < g0:
            continue
        gv = prof[g0:g1 + 1]
        gaps.append({"i": i, "x0": g0, "x1": g1, "max": float(gv.max()),
                     "med": float(np.median(gv))})
    adj = {ci - 1, ci}                              # gaps touching cluster
    clean = [g for g in gaps
             if g["i"] not in adj and abs((g["x0"] + g["x1"]) / 2 - pc) > 70]
    base_med = statistics.median([g["med"] for g in clean]) if clean else 30.0
    q1 = sorted(g["med"] for g in clean)[len(clean) // 4] if clean else 30.0

    # ---- 3. gap contrast + haze (clean gaps, max-sample = round-2 method) ----
    core_gap = [min(prof[g["x0"] - 1], prof[g["x1"] + 1]) / max(1, g["max"])
                for g in clean]
    res["gap_n"] = len(clean)
    res["gap_med"] = statistics.median(core_gap) if core_gap else 0.0
    res["gap_min"] = min(core_gap) if core_gap else 0.0
    res["gap_haze_max"] = max((g["max"] - body[1] for g in gaps), default=0.0)

    # ---- 2. midgap vs single-bar glow ----
    r_med = [g["med"] / q1 for g in gaps]
    r_max = [g["max"] / q1 for g in gaps]
    res["midgap_med"] = statistics.median(r_med) if r_med else 0.0
    res["midgap_max"] = max(r_max) if r_max else 0.0

    # ---- 6. span % + margins ----
    # Span = extent of the emitting elements (the ON runs), not a loose
    # body+10 threshold -- the latter picks up halo bleed outside the
    # waveform and reproduces 99% instead of round-2's 96.3%.
    if orun:
        f, l = orun[0][0], orun[-1][1]
    else:
        f, l = 0, bw - 1
    res["span_first"], res["span_last"] = f, l
    res["span_pct"] = 100.0 * (l - f + 1) / bw
    # f/l are bbox-relative; margins must be too (round-2's 8.2%/13% are
    # ref targets, the live capture's margins are near-zero).
    res["margin_L"] = 100.0 * f / bw
    res["margin_R"] = 100.0 * (bw - 1 - l) / bw

    # ---- 9. dropout detection (interior off-runs > 6px, max G < 30) ----
    # An interior dropout = an emitting-line hole > 6 px away from the
    # capsule ends.  Runs that touch the margin (last dot -> edge) are the
    # span margin, not a hole -- report them separately so the round-2
    # "3 dropouts" (2 interior + 1 margin run in that capture) is legible.
    drop, margin = [], []
    for g0, g1 in runs_of(prof < 30):
        if g1 - g0 + 1 <= 6:
            continue
        rec = (g0 + x0, g1 + x0, g1 - g0 + 1, int(prof[g0:g1 + 1].max()))
        (drop if (g0 > x0 + 6 and g1 < x1 - 6) else margin).append(rec)
    res["dropouts"] = drop
    res["margin_runs"] = margin

    # ---- 11. pitch + dot count per side ----
    # Pitch: autocorrelation of the emitting line over the two flank
    # zones (symmetric about the peak) -- robust to the crop's 2x upscale
    # and to the fused cluster run, unlike run-center spacing (which the
    # 2x duplicate columns roughly double -> 11.75).  Logical px =
    # crop/2: brief target 3.7 logical = 7.4 crop px.  Round-2's headline
    # "4.84" used crop/2.27 (=11.0 crop); printed for cross-check.
    pk_x = int(round(pc))
    zone = np.concatenate([prof[max(0, pk_x - 260):max(0, pk_x - 70)],
                           prof[pk_x + 70:min(len(prof), pk_x + 260)]]).astype(float)
    if len(zone) >= 24:
        z = zone - zone.mean()
        ac = np.correlate(z, z, "full")[len(z) - 1:]
        ac = (ac / ac[0]) if ac[0] else ac
        cand = [k for k in range(5, min(24, len(ac) - 1))
                if ac[k] > ac[k - 1] and ac[k] >= ac[k + 1]]
        pitch = float(max(cand, key=lambda k: ac[k])) if cand else float("nan")
    else:
        pitch = float("nan")

    def pitch_side(rng):
        cs = [(a + b) / 2.0 for a, b in rng]
        d = [b - a for a, b in zip(cs, cs[1:])]
        d = [x for x in d if x < 40]          # drop cluster-fusion skips
        return (statistics.median(d) if d else float("nan"), len(rng))
    left = [r for r in orun if r[1] < cluster[0]]
    right = [r for r in orun if r[0] > cluster[1]]
    nl = pitch_side(left)[1]
    nr = pitch_side(right)[1]
    res["pitch_px"] = pitch
    res["pitch_ac"] = float(ac.max())
    res["pitch_logical"] = pitch / 2.0
    res["pitch_r2"] = pitch / 2.27
    res["dots_L"], res["dots_R"] = nl, nr

    # ---- 8. tail: end-dot B-G + tail glow lift ----
    def enddot(run):
        a, b = run
        seg = im[by0:by1 + 1, x0 + a:x0 + b + 1].reshape(-1, 3)
        core = seg[seg[:, 1].argmax()]
        return core, float(core[2] - core[1])
    tail_lifts = []
    ends = []
    for run in ([orun[0]] if orun else []) + ([orun[-1]] if len(orun) > 1 else []):
        core, dbg = enddot(run)
        ends.append((run, tuple(int(v) for v in core), dbg))
        tail_lifts.append(core[1] - body[1])   # G lift over body (round-2 method)
    res["enddots"] = ends
    res["enddot_bg_min"] = min((e[2] for e in ends), default=0.0)
    res["tail_lift_min"] = min(tail_lifts) if tail_lifts else 0.0
    res["tail_lift_max"] = max(tail_lifts) if tail_lifts else 0.0

    # ---- 2b. halo hue: band above the peak cluster + gap beside it ----
    pcrel = int(round(pc))
    colref = lsub[:, pcrel]
    top = int(np.where(colref >= 230)[0].min()) if (colref >= 230).any() else y0
    hb = im[max(y0, top - 8):top, max(x0, pcrel - 16):min(x1 + 1, pcrel + 17)]
    hm = hb.reshape(-1, 3).mean(axis=0) if hb.size else np.zeros(3)
    res["halo_above"] = (hm, float(hm[0] / max(1, hm[1])), float(hm[2] / max(1, hm[1])))
    adj_gaps = [g for g in gaps if g["i"] in adj]
    gside = adj_gaps[0] if adj_gaps else None
    if gside:
        gb = im[by0:by1 + 1, x0 + gside["x0"]:x0 + gside["x1"] + 1].reshape(-1, 3)
        glow = gb[gb[:, 1] >= body[1] + 15]    # halo bleed, not body floor
        gm = (glow if len(glow) >= 12 else gb).mean(axis=0)
        res["halo_gap"] = (gm, float(gm[0] / max(1, gm[1])), float(gm[2] / max(1, gm[1])))
    else:
        res["halo_gap"] = None
    return res


def rg_ok(v):
    return T["halo_rg_lo"] <= v <= T["halo_rg_hi"]


def report(res):
    rows = []
    add = lambda item, val, tgt, ok: rows.append((item, val, tgt, ok))
    add("1 plateau px >=lum230", res["plateau_px"], "<=%d" % T["plateau_px_max"],
        res["plateau_px"] <= T["plateau_px_max"])
    add("1 plateau max row-span", "%d rows" % res["plateau_span"],
        "<=%d rows" % T["plateau_span_max"], res["plateau_span"] <= T["plateau_span_max"])
    add("1 hottest px RGB / R/G",
        "%s / %.2f" % (tuple(int(v) for v in res["hot_rgb"]), res["hot_rg"]),
        "R/G>=%.2f" % T["core_rg_min"], res["hot_rg"] >= T["core_rg_min"])
    ha = res["halo_above"]
    add("2 halo above peak  mean/R/G/B-G",
        "(%d,%d,%d) R/G %.2f B/G %.2f" % (ha[0][0], ha[0][1], ha[0][2], ha[1], ha[2]),
        "R/G %.2f-%.2f" % (T["halo_rg_lo"], T["halo_rg_hi"]), rg_ok(ha[1]))
    hg = res["halo_gap"]
    if hg:
        add("2 halo gap beside cluster",
            "(%d,%d,%d) R/G %.2f" % (hg[0][0], hg[0][1], hg[0][2], hg[1]),
            "R/G %.2f-%.2f" % (T["halo_rg_lo"], T["halo_rg_hi"]), rg_ok(hg[1]))
    add("3 gap contrast core/gap", "med %.2f min %.2f (n=%d)" %
        (res["gap_med"], res["gap_min"], res["gap_n"]),
        ">=%.1f / >=%.1f" % (T["gap_med_min"], T["gap_min_min"]),
        res["gap_med"] >= T["gap_med_min"] and res["gap_min"] >= T["gap_min_min"])
    add("3 gap haze over body", "+%.0f G" % res["gap_haze_max"],
        "<=%d" % T["gap_haze_max"], res["gap_haze_max"] <= T["gap_haze_max"])
    add("4 midgap vs single-bar", "med %.2fx max %.2fx" %
        (res["midgap_med"], res["midgap_max"]),
        "<=%.2fx" % T["midgap_ratio_max"], res["midgap_med"] <= T["midgap_ratio_max"])
    add("5 span %% of W (margins L/R)",
        "%.1f%% (%.1f%%/%.1f%%)" % (res["span_pct"], res["margin_L"], res["margin_R"]),
        "~%.0f%%, >=%.0f%%/>=%.0f%%" % (T["span_pct"], T["margin_L_min"], T["margin_R_min"]),
        abs(res["span_pct"] - T["span_pct"]) <= 5
        and res["margin_L"] >= T["margin_L_min"] and res["margin_R"] >= T["margin_R_min"])
    add("6 cluster %% of W / tall cols", "%.1f%% / %d cols (%.0f logical)" %
        (res["cluster_pct"], res["tall_cols"], res["tall_cols_logical"]),
        "~%.0f%% / ~%d" % (T["cluster_pct"], T["tall_cols"]),
        abs(res["cluster_pct"] - T["cluster_pct"]) <= 6
        and abs(res["tall_cols_logical"] - T["tall_cols"]) <= 4)
    add("6 tall-col profile", str(res["cluster_profile"][::max(1, len(res["cluster_profile"]) // 12)]),
        "mound 54/74/84/74/54-like", None)
    add("7 peak:flank height", "%.1f:1 (%d vs %d px)" %
        (res["peak_flank"], res["peak_vext"], res["flank_vext_med"]),
        "~%.0f:1" % T["peak_flank"], abs(res["peak_flank"] - T["peak_flank"]) <= 2.5)
    add("8 end-dot B-G", " / ".join("B-G %+.0f %s" % (e[2], e[1]) for e in res["enddots"]),
        ">=%+d" % T["enddot_bg_min"], res["enddot_bg_min"] >= T["enddot_bg_min"])
    add("8 tail glow lift over body", "%.0f..%.0f lum" %
        (res["tail_lift_min"], res["tail_lift_max"]),
        "+%d..%d" % (T["tail_lift_lo"], T["tail_lift_hi"]),
        res["tail_lift_min"] >= T["tail_lift_lo"] - 3
        and res["tail_lift_max"] <= T["tail_lift_hi"] + 3)
    nd = len(res["dropouts"])
    add("9 interior dropouts >6px", "%d %s%s" % (nd, res["dropouts"] if nd else "",
        " (+%d margin run)" % len(res["margin_runs"]) if res["margin_runs"] else ""),
        "0", nd == T["dropouts"])
    add("11 pitch", "%.2f crop = %.2f br-log / %.2f r2-log" %
        (res["pitch_px"], res["pitch_logical"], res["pitch_r2"]),
        "~7.4 crop px", abs(res["pitch_px"] - 2 * T["pitch"]) <= 1.0)
    add("11 dots per side", "L %d / R %d" % (res["dots_L"], res["dots_R"]),
        "~25-30 per side", None)
    add("10 body RGB / lum", "%s / %.0f" %
        (tuple(int(v) for v in res["body_rgb"]), res["body_lum"]),
        "(16,20,30)+-6", all(abs(res["body_rgb"][i] - t) <= 8
                             for i, t in enumerate((16, 20, 30))))
    add("10 rim delta vs interior", "+%.0f lum" % res["rim_d"], "+10..+20",
        8 <= res["rim_d"] <= 24)
    add("10 host navy RGB", str(tuple(int(v) for v in res["host_rgb"])),
        "(17,33,66)-ish", None)

    print("=" * 78)
    print("ROUND-3 PROBE  %s" % res["file"])
    print("bbox x%d..%d y%d..%d  (W%d H%d, scale %.2f crop-px/logical; band rows %d-%d)"
          % (res["bbox"][0], res["bbox"][2], res["bbox"][1], res["bbox"][3],
             res["bbox"][2] - res["bbox"][0] + 1, res["bbox"][3] - res["bbox"][1] + 1,
             res["scale"], res["band"][0], res["band"][1]))
    print("=" * 78)
    print("%-38s %-34s %-16s %s" % ("metric", "measured", "target", "verdict"))
    print("-" * 78)
    for item, val, tgt, ok in rows:
        print("%-38s %-34s %-16s %s"
              % (item[:38], str(val)[:34], str(tgt)[:16],
                 "--" if ok is None else ("PASS" if ok else "FAIL")))
    print("-" * 78)
    n_fail = sum(1 for r in rows if r[3] is False)
    print("%d checks, %d FAIL" % (len(rows), n_fail))


if __name__ == "__main__":
    paths = sys.argv[1:] or ["critique/round2-active.png"]
    for p in paths:
        report(analyze(p))
        print()
