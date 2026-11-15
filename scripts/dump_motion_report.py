#!/usr/bin/env python3
"""H2: motion report from a STATIC_DICTATION_DUMP_FRAMES dump dir.

Takes the PNG frame sequence (frame-%04d.png at fixed 100 ms cadence — see
src/dump.rs) and produces:
  - critique/round3-anim.gif           100 ms/frame animation of the dump
  - critique/round3-motion-sheet.md    the motion sheet

Motion metrics (method mirrors the motion critic's round-2 measurement):
  - mean |ΔH| per step: per consecutive frame pair, the mean absolute change
    of the per-column waveform height inside the pill region (px @scale 2);
    slow breathing should read ≈0.1–1 px, wobble/aliasing several px.
  - profile correlation: Pearson r of the column height profile between
    consecutive frames (silhouette stability; the critic gates ≥ 0.998 on
    the designed profile).
  - breathing period: autocorrelation of the center-cluster height series
    (detrended, 10 fps) → first strong peak = dominant period in seconds.

Usage: python3 scripts/dump_motion_report.py <dumpdir> [--thresh T]
Pill region is auto-detected per frame as the non-black bbox (dump PNGs are
composited on transparent/black).
"""
import glob
import os
import sys

import numpy as np
from PIL import Image

PROJ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GIF_PATH = os.path.join(PROJ, "critique", "round3-anim.gif")
SHEET_PATH = os.path.join(PROJ, "critique", "round3-motion-sheet.md")
FPS = 10.0  # dump cadence: one frame per 100 ms


def load_frames(dumpdir, thresh):
    paths = sorted(glob.glob(os.path.join(dumpdir, "frame-*.png")))
    if len(paths) < 2:
        sys.exit(f"need ≥2 frames in {dumpdir}, found {len(paths)}")
    profiles, centers, used = [], [], []
    for p in paths:
        im = np.asarray(Image.open(p).convert("RGB"), dtype=np.int16)
        # Waveform mask: teal/cyan G channel above threshold (body bg G≈20,
        # background black). thresh≈40 keeps whisper tails, drops body/halo.
        mask = im[:, :, 1] >= thresh
        if not mask.any():
            continue
        ys, xs = np.where(mask)
        x0, x1 = xs.min(), xs.max() + 1
        # Per-column waveform height (px) inside the pill bbox.
        prof = mask[:, x0:x1].sum(axis=0).astype(np.float64)
        profiles.append(prof)
        # Center-cluster height: mean height over the middle 20% of columns.
        c0, c1 = int(len(prof) * 0.40), int(len(prof) * 0.60)
        centers.append(prof[c0:c1].mean())
        used.append(os.path.basename(p))
    return used, profiles, np.array(centers)


def autocorr_period(x, fps):
    """Dominant period of a short series via normalized autocorrelation."""
    if len(x) < 4 or np.allclose(x, x[0]):
        return None, None
    x = x - x.mean()
    ac = np.correlate(x, x, mode="full")[len(x) - 1:]
    if ac[0] <= 0:
        return None, None
    ac /= ac[0]
    # First local maximum with ac ≥ 0.3 after the series' own zero crossing
    # of the mean (lag 0..1 is trivially high); needs 3 rising samples.
    for i in range(2, len(ac) - 1):
        if ac[i] >= 0.3 and ac[i] >= ac[i - 1] and ac[i] >= ac[i + 1]:
            return i / fps, ac[i]
    return None, None


def main():
    args = sys.argv[1:]
    thresh = 40
    if "--thresh" in args:
        i = args.index("--thresh")
        thresh = float(args[i + 1])
        del args[i:i + 2]
    dumpdir = args[0] if args else max(
        glob.glob(os.path.join(PROJ, "live-proof", "dump-*")), key=os.path.getmtime)

    used, profiles, centers = load_frames(dumpdir, thresh)
    n = len(profiles)
    width = max(len(p) for p in profiles)

    # Pad profiles to a common width (bbox jitters ±1 px at most) with zeros.
    padded = np.zeros((n, width))
    for i, p in enumerate(profiles):
        padded[i, :len(p)] = p

    deltas, corrs = [], []
    for i in range(1, n):
        deltas.append(np.abs(padded[i] - padded[i - 1]).mean())
        a, b = padded[i - 1], padded[i]
        if a.std() > 1e-9 and b.std() > 1e-9:
            corrs.append(float(np.corrcoef(a, b)[0, 1]))
        else:
            corrs.append(1.0 if np.allclose(a, b) else 0.0)

    period, ac_peak = autocorr_period(centers, FPS)
    deltas, corrs = np.array(deltas), np.array(corrs)
    # Per-column pop detector: the largest single-column height change in any
    # 100 ms step. Aggregate mean |ΔH| can hide one rim column twinkling.
    # Skip frame 0→1: the spring's intentional entrance run-in from zero
    # bars, not a steady-state pop (already gated by the entrance tests).
    step = np.abs(padded[2:] - padded[1:-1])
    max_col_step = float(step.max())
    pop_col = int(np.unravel_index(step.argmax(), step.shape)[1])

    lines = [
        "# Motion sheet (from in-process 100 ms frame dump)",
        "",
        f"- source: `{os.path.relpath(dumpdir, PROJ)}` — {n} frames used "
        f"of {len(glob.glob(os.path.join(dumpdir, 'frame-*.png')))} "
        f"({os.path.basename(used[0])} … {os.path.basename(used[-1])}), "
        f"{1000 / FPS:.0f} ms/frame, G-threshold {thresh:.0f}",
        f"- mean |ΔH| per step: {deltas.mean():.3f} px (min {deltas.min():.3f}, "
        f"max {deltas.max():.3f}) — slow breathing target ≲1 px/step",
        f"- consecutive profile correlation: mean {corrs.mean():.4f}, "
        f"min {corrs.min():.4f} (critic gate ≥ 0.998)",
        f"- center-cluster height series: {centers.min():.1f}–{centers.max():.1f} px "
        f"(±{100 * (centers.max() - centers.min()) / 2 / max(centers.mean(), 1e-9):.1f}% of mean)",
        f"- per-column max step: {max_col_step:.1f} px at column {pop_col} "
        f"(gate ≤ 3 px ⇒ {'PASS' if max_col_step <= 3 else 'FAIL'}: no single-column pop)",
        f"- breathing period: "
        + (f"{period:.2f} s (autocorr peak {ac_peak:.2f}, gate ≥ 1.5 s)"
           if period else "not detected (series too short/flat)"),
        "",
        "## per-step |ΔH| (px)",
        " ".join(f"{d:.2f}" for d in deltas),
    ]
    sheet = "\n".join(lines) + "\n"
    os.makedirs(os.path.dirname(SHEET_PATH), exist_ok=True)
    with open(SHEET_PATH, "w") as f:
        f.write(sheet)

    # 100 ms GIF over the used frames (re-crop to the union bbox for framing).
    frames = [Image.open(os.path.join(dumpdir, u)) for u in used]
    gif = frames[0].convert("RGB").quantize(colors=128, method=Image.MEDIANCUT)
    gif.save(GIF_PATH, save_all=True, append_images=[
        f.convert("RGB").quantize(colors=128, method=Image.MEDIANCUT) for f in frames[1:]],
        duration=int(1000 / FPS), loop=0)

    print("\n".join(lines[2:8]))
    print(f"\ngif: {GIF_PATH} ({os.path.getsize(GIF_PATH)} bytes)")
    print(f"sheet: {SHEET_PATH}")


if __name__ == "__main__":
    main()
