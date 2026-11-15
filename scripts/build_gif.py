#!/usr/bin/env python3
"""Crop pill region from cap-* shots, verify pill liveness, build animated GIF.
Usage: python3 scripts/build_gif.py [capdir]  (default: newest live-proof/cap-*)"""
import os, sys, glob
from PIL import Image, ImageChops

PROJ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# pill region in full screenshot coords (2256x1504, scale 1.5)
X0, Y0, X1, Y1 = 1010, 0, 1450, 95
W, H = 880, 190  # output crop size

capdir = sys.argv[1] if len(sys.argv) > 1 else max(
    glob.glob(os.path.join(PROJ, "live-proof", "cap-*")), key=os.path.getmtime)
shots = sorted(glob.glob(os.path.join(capdir, "shot-[0-9][0-9].png")))
released = os.path.join(capdir, "shot-released.png")

crops = []
for p in shots:
    im = Image.open(p).crop((X0, Y0, X1, Y1)).resize((W, H), Image.LANCZOS)
    im.save(os.path.join(capdir, f"crop-{os.path.basename(p)[5:7]}.png"))
    crops.append(im)
Image.open(released).crop((X0, Y0, X1, Y1)).resize((W, H), Image.LANCZOS).save(
    os.path.join(capdir, "crop-released.png"))

# liveness check: each held shot diffed against the released one in the pill region
rel = Image.open(released).crop((X0, Y0, X1, Y1)).convert("RGB")
scores = []
for p in shots:
    a = Image.open(p).crop((X0, Y0, X1, Y1)).convert("RGB")
    bbox = ImageChops.difference(a, rel).getbbox()
    scores.append((bbox[2] - bbox[0]) if bbox else 0)
print("pill-visible scores per shot:", scores)

# best consecutive window of 8-12 frames by total pill-visible score
best = (8, 0)
bestv = -1
for n in range(8, 13):
    for i in range(0, len(scores) - n + 1):
        v = sum(scores[i:i + n])
        if v > bestv:
            bestv, best = v, (n, i)
n, i0 = best
frames = crops[i0:i0 + n]
print(f"selected frames {i0+1}..{i0+n} of {len(crops)}")

# harness spacing: (HOLD-1)/(COUNT-1) s per frame = 4/9 s ≈ 444 ms for the default run
duration = int(4000 / 9)
gif = os.path.join(PROJ, "critique", "live-anim-30.gif")
frames[0].save(gif, save_all=True, append_images=frames[1:], duration=duration, loop=0)
print("gif:", gif, os.path.getsize(gif), "bytes,", n, "frames,", duration, "ms/frame")
