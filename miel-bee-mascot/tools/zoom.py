#!/usr/bin/env python3
"""Hoja ampliada de estados concretos (revisión visual). Uso: zoom.py out.png size estado1 estado2 ..."""
import subprocess, sys
from pathlib import Path
from PIL import Image
ROOT = Path(__file__).resolve().parent.parent
out, size, states = sys.argv[1], int(sys.argv[2]), sys.argv[3:]
bg = (244, 241, 234, 255)
sheet = Image.new("RGBA", (size * len(states), size), bg)
for i, s in enumerate(states):
    p = ROOT / "preview" / f"_z_{s}.png"
    subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), str(ROOT / "assets/bee" / f"bee-{s}.svg"), "-o", str(p)], check=True)
    sheet.alpha_composite(Image.open(p).convert("RGBA"), (i * size, 0))
    p.unlink()
sheet.convert("RGB").save(ROOT / "preview" / out)
