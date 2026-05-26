#!/usr/bin/env python3
"""
Rasterize the four tray state icons from their SVG sources into the PNG
sizes Tauri needs.

Run from repo root:

    python3 scripts/build_tray_icons.py

Sources:  assets/tray/tray-<state>.svg   (the geometry source of truth)
Outputs:  src-tauri/icons/tray-<state><suffix>.png

Uses rsvg-convert (librsvg) for clean, consistent rendering — the same
tool family used for the app bundle icons. Do not substitute PIL /
cairosvg / imagemagick; their output differs.

    brew install librsvg
"""

import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
SRC_DIR = ROOT / "assets" / "tray"
OUT_DIR = ROOT / "src-tauri" / "icons"

STATES = ["ok", "warn", "crit", "unreachable"]
# (size_px, filename_suffix)
SIZES = [(16, "-16"), (22, "-22"), (32, ""), (64, "@2x")]


def main() -> int:
    if shutil.which("rsvg-convert") is None:
        print(
            "error: rsvg-convert not found on PATH.\n"
            "Install librsvg first:  brew install librsvg",
            file=sys.stderr,
        )
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)

    for state in STATES:
        src = SRC_DIR / f"tray-{state}.svg"
        for size, suffix in SIZES:
            out = OUT_DIR / f"tray-{state}{suffix}.png"
            subprocess.run(
                ["rsvg-convert", "-w", str(size), "-h", str(size), str(src), "-o", str(out)],
                check=True,
            )
            print(f"wrote {out.relative_to(ROOT)}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
