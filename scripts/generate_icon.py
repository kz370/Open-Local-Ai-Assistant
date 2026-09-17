"""Generates the application icon (a teal rounded square with a sound wave).

Usage:
    python scripts/generate_icon.py            # writes assets/icon.png
    npm run tauri icon assets/icon.png         # regenerates src-tauri/icons/*

Requires Pillow.
"""

from __future__ import annotations

import os

from PIL import Image, ImageDraw, ImageFilter

SIZE = 1024
RADIUS = int(SIZE * 0.22)
TOP = (52, 226, 205)  # teal 300
BOTTOM = (13, 106, 100)  # teal 800
BARS = [0.30, 0.52, 0.86, 0.66, 0.40]  # relative heights of the wave bars


def rounded_mask(size: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size * 4, size * 4), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, size * 4 - 1, size * 4 - 1), radius=radius * 4, fill=255)
    return mask.resize((size, size), Image.LANCZOS)


def gradient(size: int, top: tuple[int, int, int], bottom: tuple[int, int, int]) -> Image.Image:
    img = Image.new("RGB", (1, size))
    for y in range(size):
        t = y / max(1, size - 1)
        img.putpixel((0, y), tuple(round(top[i] + (bottom[i] - top[i]) * t) for i in range(3)))
    return img.resize((size, size), Image.BILINEAR)


def main() -> None:
    base = gradient(SIZE, TOP, BOTTOM).convert("RGBA")

    # Soft diagonal sheen for depth.
    sheen = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(sheen).ellipse((-SIZE * 0.45, -SIZE * 0.75, SIZE * 0.95, SIZE * 0.45), fill=70)
    base = Image.composite(Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 255)), base, sheen.filter(ImageFilter.GaussianBlur(SIZE * 0.08))).convert("RGBA")
    base = Image.blend(gradient(SIZE, TOP, BOTTOM).convert("RGBA"), base, 0.22)

    # Sound-wave glyph.
    draw = ImageDraw.Draw(base)
    bar_w = SIZE * 0.072
    gap = SIZE * 0.052
    total = len(BARS) * bar_w + (len(BARS) - 1) * gap
    x = (SIZE - total) / 2
    for h in BARS:
        height = SIZE * 0.58 * h
        y0 = SIZE / 2 - height / 2
        draw.rounded_rectangle((x, y0, x + bar_w, y0 + height), radius=bar_w / 2, fill=(255, 255, 255, 240))
        x += bar_w + gap

    icon = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    icon.paste(base, (0, 0), rounded_mask(SIZE, RADIUS))

    out_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets")
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, "icon.png")
    icon.save(out)
    print("wrote", out)


if __name__ == "__main__":
    main()
