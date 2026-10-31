#!/usr/bin/env python3
"""Regenerate cached PNG masks from the pinned, unchanged Material SVG assets.

This maintainer-only script needs Inkscape and Pillow. BitchPixel's build and
runtime use only the checked-in PNG masks and do not require either tool.
"""
from pathlib import Path
import subprocess
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "apps/canvas-demo/assets/material-symbols-rounded"
SIZE = 96

for source in sorted(ASSETS.glob("*.svg")):
    target = source.with_suffix(".png")
    subprocess.run(
        [
            "inkscape",
            str(source),
            "--export-type=png",
            f"--export-filename={target}",
            f"--export-width={SIZE}",
            f"--export-height={SIZE}",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    rendered = Image.open(target).convert("RGBA")
    alpha = rendered.getchannel("A")
    mask = Image.new("RGBA", rendered.size, (255, 255, 255, 0))
    mask.putalpha(alpha)
    mask.save(target, optimize=True)
print(f"Rendered {len(list(ASSETS.glob('*.svg')))} icon masks at {SIZE}x{SIZE}")
