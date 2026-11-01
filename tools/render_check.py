#!/usr/bin/env python3
"""Rasteriza los SVG estáticos y arma hojas de contacto (fondo claro/oscuro; 64/96/128 px).
Requiere rsvg-convert y Pillow. Solo para revisión visual; no forma parte de la app."""
import subprocess, sys
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "assets" / "bee"
OUT = ROOT / "preview"
OUT.mkdir(exist_ok=True)
STATES = ["idle", "happy", "curious", "thinking", "sleepy", "surprised", "talking", "resting"]
BGS = {"light": (244, 241, 234, 255), "dark": (30, 31, 36, 255)}
SIZES = [128, 96, 64]
SCALE = int(sys.argv[1]) if len(sys.argv) > 1 else 1


def raster(state: str, size: int) -> Image.Image:
    png = OUT / f"_{state}_{size}.png"
    subprocess.run(["rsvg-convert", "-w", str(size * SCALE), "-h", str(size * SCALE),
                    str(SVG / f"bee-{state}.svg"), "-o", str(png)], check=True)
    return Image.open(png).convert("RGBA")


for bg_name, bg in BGS.items():
    pad = 8 * SCALE
    w = len(STATES) * (128 * SCALE + pad) + pad
    h = sum(s * SCALE + pad for s in SIZES) + pad
    sheet = Image.new("RGBA", (w, h), bg)
    y = pad
    for size in SIZES:
        x = pad
        for st in STATES:
            im = raster(st, size)
            sheet.alpha_composite(im, (x + (128 * SCALE - size * SCALE) // 2, y))
            x += 128 * SCALE + pad
        y += size * SCALE + pad
    sheet.convert("RGB").save(OUT / f"sheet-{bg_name}.png")
    print("saved", OUT / f"sheet-{bg_name}.png", sheet.size)

for f in OUT.glob("_*.png"):
    f.unlink()
