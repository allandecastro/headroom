#!/usr/bin/env python3
"""
Generate the four tray icon PNGs from the Headroom design tokens.

Run from repo root:

    python3 scripts/build_tray_icons.py

Outputs to src-tauri/icons/.
"""

# NOTE: This script generates ONLY the tray state icons.
# App bundle icons (32x32.png, icon.icns, icon.ico) come from
# `npx tauri icon` — see README "Regenerating icons".

from pathlib import Path
from PIL import Image, ImageDraw

OUT_DIR = Path(__file__).parent.parent / "src-tauri" / "icons"
OUT_DIR.mkdir(parents=True, exist_ok=True)

# State -> (ceiling_rgba, bar_rgba, fill_pct)
STATES = {
    "ok": ((138, 179, 104, 140), (138, 179, 104, 255), 0.55),
    "warn": ((217, 156, 82, 140), (217, 156, 82, 255), 0.77),
    "crit": ((212, 98, 93, 165), (212, 98, 93, 255), 0.96),
    "unreachable": ((180, 180, 180, 180), (180, 180, 180, 140), None),
}

SIZES = [16, 22, 32, 64]


def draw_icon(state: str, size: int) -> Image.Image:
    """Render one tray icon to a square RGBA image."""
    # Work in 32x32 logical space, then resample to target size.
    canvas = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)
    ceiling_color, bar_color, fill_pct = STATES[state]

    if state == "unreachable":
        # Dashed ceiling line
        for x in range(6, 26, 4):
            draw.rectangle([x, 6, min(x + 2, 26), 7], fill=ceiling_color)
        # Dashed empty box outline
        for x in range(9, 23, 3):
            draw.rectangle([x, 13, min(x + 2, 23), 13], fill=bar_color)
            draw.rectangle([x, 27, min(x + 2, 23), 27], fill=bar_color)
        for y in range(13, 27, 3):
            draw.rectangle([9, y, 9, min(y + 2, 27)], fill=bar_color)
            draw.rectangle([23, y, 23, min(y + 2, 27)], fill=bar_color)
    else:
        # Solid ceiling
        draw.rectangle([6, 6, 25, 7], fill=ceiling_color)
        # Filled bar (height varies)
        available = 27 - 9  # 18 px range between ceiling+gap and bottom
        height = int(round(available * fill_pct))
        top = 27 - height
        draw.rounded_rectangle([9, top, 22, 27], radius=1.5, fill=bar_color)

    if size != 32:
        canvas = canvas.resize((size, size), Image.LANCZOS)
    return canvas


def main():
    for state in STATES:
        for size in SIZES:
            img = draw_icon(state, size)
            suffix = "" if size == 32 else f"@{size}"
            # Tauri expects tray-{state}.png (32 base) plus optional retina variants
            if size == 32:
                name = f"tray-{state}.png"
            elif size == 64:
                name = f"tray-{state}@2x.png"
            else:
                name = f"tray-{state}-{size}.png"
            out = OUT_DIR / name
            img.save(out, "PNG")
            print(f"wrote {out.relative_to(OUT_DIR.parent.parent)}")


if __name__ == "__main__":
    main()
