#!/usr/bin/env python3
"""60-frame preview: the mangobar clock pill expands into the dictation waveform.

This is a DESIGN VIDEO, not the live surface. It draws the same palette and the
same 27-slot SHAPE_MASK envelope as src/pill/canvas.rs / src/pill/vecmentor.rs
over a crop of the real desktop top bar, so the motion can be reviewed before
anyone touches mangobar's clock widget.

Usage: python3 scripts/make_clock_morph.py [outputdir]
Outputs: clock-to-dictation-60.gif, clock-to-dictation-60.mp4,
         clock-to-dictation-contact.png, and 60 frame-*.png files.
"""
import math
import os
import sys
import subprocess
from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = sys.argv[1] if len(sys.argv) > 1 else "/tmp/clock_morph"
os.makedirs(OUT, exist_ok=True)

W, H = 880, 220
S = 2  # supersample, then downsample for clean edges
CX, TOP = W / 2, 2.0
W0, H0 = 140.0, 48.0   # clock pill
W1, H1 = 440.0, 96.0   # dictation pill
NFRAMES = 60

BG = os.path.join(ROOT, "critique", "clock-morph-bg.png")
FONT = "/usr/share/fonts/noto/NotoSans-Regular.ttf"

# Same 27-slot asymmetric envelope as src/pill/vecmentor.rs::SHAPE_MASK.
SHAPE = [
    0.03, 0.04, 0.05, 0.06, 0.07, 0.11, 0.16, 0.17, 0.12, 0.10,
    0.20, 0.30, 0.42, 0.58, 0.76, 1.00, 0.80, 0.56, 0.38, 0.25,
    0.12, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03,
]

BODY_TOP = (18, 23, 33)
BODY_BOTTOM = (12, 16, 26)
LEFT = (115, 153, 154)
CENTRE = (170, 235, 234)
RIGHT = (102, 132, 225)
GLOW = (171, 239, 238)


def clamp(v, a=0.0, b=1.0):
    return max(a, min(b, v))


def smooth(a, b, x):
    t = clamp((x - a) / (b - a))
    return t * t * (3 - 2 * t)


def ease(p):
    return 1 - (1 - p) ** 3


def mix(c1, c2, t):
    return tuple(int(round(a + (b - a) * t)) for a, b in zip(c1, c2))


def px(v):
    return int(round(v * S))


def prepare_bg():
    """Clean top-bar backdrop: wallpaper behind the pill, dark desktop below.

    The raw screenshot has the source clock pill (which would otherwise ghost
    through our own capsule) and a T3 window with readable text below the bar.
    """
    src = Image.open(BG).convert("RGB").resize((W * S, H * S), Image.LANCZOS)
    wall = src.crop((0, 0, W * S, px(55)))
    patch = wall.crop((px(20), 0, px(147), px(50))).transpose(
        Image.Transpose.FLIP_LEFT_RIGHT
    )
    wall.paste(patch, (px(383), 0))
    wall = wall.filter(ImageFilter.GaussianBlur(px(1.2)))
    out = Image.new("RGB", (W * S, H * S), (8, 10, 16))
    d = ImageDraw.Draw(out)
    y0 = px(55)
    for y in range(y0, H * S):
        c = mix((15, 18, 26), (7, 9, 14), (y - y0) / (H * S - y0))
        d.line((0, y, W * S, y), fill=c)
    out.paste(wall, (0, 0))
    ImageDraw.Draw(out).line((0, y0, W * S, y0), fill=(28, 42, 62), width=S)
    return out.convert("RGBA")


CLEAN_BG = None


def box_of(w, h):
    return (px(CX - w / 2), px(TOP), px(CX + w / 2), px(TOP + h))


def capsule(w, h):
    """Dark obsidian capsule with rim, crease and bottom specular."""
    layer = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    b = box_of(w, h)
    r = px(h / 2)
    d = ImageDraw.Draw(layer)
    # vertical body gradient
    strip = Image.new("RGBA", (1, max(1, b[3] - b[1])), (0, 0, 0, 0))
    sd = ImageDraw.Draw(strip)
    for y in range(strip.height):
        sd.point((0, y), fill=mix(BODY_TOP, BODY_BOTTOM, y / max(1, strip.height - 1)) + (255,))
    strip = strip.resize((b[2] - b[0], b[3] - b[1]))
    mask = Image.new("L", (W * S, H * S), 0)
    ImageDraw.Draw(mask).rounded_rectangle(b, radius=r, fill=255)
    layer.paste(strip, (b[0], b[1]), mask.crop((b[0], b[1], b[2], b[3])))
    # 1px rim
    d.rounded_rectangle(b, radius=r, outline=(*mix(BODY_TOP, (60, 70, 80), 0.35), 40), width=max(1, S))
    # dark crease along the top edge
    d.arc((b[0] + r, b[1] + px(1), b[2] - r, b[1] + px(12)), 180, 360,
          fill=(6, 9, 14, 90), width=max(1, S))
    # faint top specular
    d.arc((b[0] + r, b[1] + px(2), b[2] - r, b[1] + px(14)), 180, 360,
          fill=(150, 170, 185, 26), width=max(1, S))
    return layer


def shadow(w, h):
    b = box_of(w, h)
    layer = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    off = (px(-3), px(2))
    moved = (b[0] + off[0], b[1] + off[1], b[2] + off[0], b[3] + off[1])
    ImageDraw.Draw(layer).rounded_rectangle(moved, radius=px(h / 2), fill=(10, 20, 35, 92))
    return layer.filter(ImageFilter.GaussianBlur(px(5.5)))


def waveform(w, h, alpha, t, grow, p):
    """Fine dot leader line + 27-slot asymmetric mound, as in the live pill.

    During the morph the envelope rises from the centre outwards, so the pill
    reads as a wave spreading out of the clock rather than a static logo.
    """
    if alpha <= 0.001:
        return Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    cy = px(TOP + h / 2)
    span = w * 0.79
    x0 = CX - span / 2
    pitch = span / 60.0
    voice = 0.88 + 0.12 * math.sin(2 * math.pi * (t - 8) / 16.0)
    glow = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    layer = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    a = int(255 * clamp(alpha))

    # 61-slot leader line, unbroken
    dot_r = max(px(0.9), px(1.15 * w / W1))
    for j in range(61):
        u = j / 60.0
        x = px(x0 + j * pitch)
        rise = clamp((p - abs(u - 0.5)) / 0.4)
        lift = (0.34 + 0.66 * (1.0 - abs(2 * u - 1)) ** 0.8) * (0.2 + 0.8 * rise)
        c = mix(LEFT, RIGHT, u)
        c = mix((24, 34, 48), c, lift)
        d.ellipse((x - dot_r, cy - dot_r, x + dot_r, cy + dot_r), fill=(*c, a))

    # 27 bars centred on slot 30
    max_h = h * 0.46 * grow * voice
    bw = max(px(1.2), px(pitch * 0.62))
    for i, s in enumerate(SHAPE):
        j = 17 + i
        u = j / 60.0
        x = px(x0 + j * pitch)
        jitter = 1.0 + 0.05 * math.sin(2 * math.pi * (t - 8) / 11.0 + i * 0.65)
        rise = clamp((p - abs(u - 0.5)) / 0.4)
        bh = max_h * s * jitter * (0.15 + 0.85 * rise)
        if bh < px(1.0):
            continue
        base = mix(LEFT, CENTRE, clamp(u / 0.5)) if u < 0.5 else mix(CENTRE, RIGHT, clamp((u - 0.5) / 0.5))
        c = mix((30, 42, 55), base, 0.45 + 0.55 * s)
        y0, y1 = cy - px(bh / 2), cy + px(bh / 2)
        d.rounded_rectangle((x - bw / 2, y0, x + bw / 2, y1), radius=bw / 2, fill=(*c, a))
        if s > 0.35:
            gd.rounded_rectangle((x - bw, y0 - px(3), x + bw, y1 + px(3)),
                                 radius=px(2), fill=(*GLOW, int(60 * alpha * s)))
    glow = glow.filter(ImageFilter.GaussianBlur(px(6)))
    return Image.alpha_composite(glow, layer)


def clock_text(w, h, alpha, t):
    if alpha <= 0.001:
        return Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    layer = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    size = px(22 * (h / H0))
    font = ImageFont.truetype(FONT, max(8, size))
    cy = px(TOP + h / 2)
    # gentle horizontal smear as the clock dissolves into dots
    text = "10:09"
    bb = d.textbbox((0, 0), text, font=font)
    tw = bb[2] - bb[0]
    scale = 1.0 + 0.9 * (1 - alpha)
    tmp = Image.new("RGBA", (max(1, int(tw * scale * 1.4)), size * 2), (0, 0, 0, 0))
    ImageDraw.Draw(tmp).text((tmp.width / 2, tmp.height / 2), text, font=font,
                             fill=(236, 240, 246, int(255 * alpha)), anchor="mm")
    tmp = tmp.resize((int(tmp.width * scale), tmp.height), Image.LANCZOS)
    layer.alpha_composite(tmp, (int(px(CX) - tmp.width / 2), int(cy - tmp.height / 2)))
    return layer


def key_hint(t):
    a = smooth(7, 11, t) * (1 - smooth(18, 26, t))
    if a <= 0.001:
        return Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    layer = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    font = ImageFont.truetype(FONT, px(11))
    d.text((px(CX), px(H - 22)), "AltGr + L", font=font, anchor="mm",
           fill=(150, 165, 185, int(210 * a)))
    return layer


def render(t):
    p = ease(clamp((t - 8) / 22.0))
    w = W0 + (W1 - W0) * p
    h = H0 + (H1 - H0) * p
    alpha = smooth(0.04, 0.40, p)
    clock_a = 1 - smooth(0.0, 0.38, p)

    base = CLEAN_BG.copy()
    base = Image.alpha_composite(base, shadow(w, h))
    base = Image.alpha_composite(base, capsule(w, h))
    base = Image.alpha_composite(base, waveform(w, h, alpha, t, 0.6 + 0.4 * p, p))
    base = Image.alpha_composite(base, clock_text(w, h, clock_a, t))
    base = Image.alpha_composite(base, key_hint(t))
    return base.resize((W, H), Image.LANCZOS).convert("RGB")


CLEAN_BG = prepare_bg()

frames = []
for t in range(NFRAMES):
    im = render(t)
    path = os.path.join(OUT, f"frame-{t:02d}.png")
    im.save(path)
    frames.append(im)
    print(f"\rframe {t + 1}/{NFRAMES}", end="", flush=True)
print()

gif = os.path.join(ROOT, "critique", "clock-to-dictation-60.gif")
# ffmpeg keeps all 60 frames; PIL's GIF writer merges identical neighbours.
subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", "10",
                "-i", os.path.join(OUT, "frame-%02d.png"),
                "-filter_complex",
                "[0:v]split[s0][s1];[s0]palettegen=max_colors=128[p];"
                "[s1][p]paletteuse=dither=bayer:bayer_scale=3",
                "-loop", "0", gif], check=True)
mp4 = os.path.join(ROOT, "critique", "clock-to-dictation-60.mp4")
subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", "10",
                "-i", os.path.join(OUT, "frame-%02d.png"), "-c:v", "libx264",
                "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart", mp4],
               check=True)

# contact sheet: six key moments
picks = [0, 7, 14, 22, 30, 46]
# 2 columns x 3 rows, half-size frames
sheet = Image.new("RGB", (W, H * 3 // 2), (8, 10, 16))
for k, idx in enumerate(picks):
    thumb = frames[idx].resize((W // 2, H // 2), Image.LANCZOS)
    sheet.paste(thumb, ((k % 2) * (W // 2), (k // 2) * (H // 2)))
sheet.save(os.path.join(ROOT, "critique", "clock-to-dictation-contact.png"))
print("wrote", gif, mp4, "contact sheet; frames in", OUT)
