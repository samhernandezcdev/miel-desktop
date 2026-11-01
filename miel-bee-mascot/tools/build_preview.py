#!/usr/bin/env python3
"""Genera preview/index.html: página autónoma con demo animada, estados, paleta y guía breve.
Lee assets/bee/*.svg (ejecuta antes tools/build_bee.py). Solo librería estándar."""
import base64, json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_bee as B  # noqa: E402

ASSETS = ROOT / "assets" / "bee"

ROLES = {
    "ink": "Contorno, rayas, ojos, antenas",
    "honey": "Cuerpo (color base)",
    "honeyLight": "Brillo y puntas de antena",
    "honeyShade": "Sombra plana inferior",
    "cream": "Pelusa del cuello, burbuja",
    "rim": "Halo para fondos oscuros",
    "blush": "Mejillas",
    "tongue": "Lengua",
    "wingFill": "Alas (opacidad .78)",
    "wingStroke": "Contorno de alas",
    "wingVein": "Nervadura de alas",
}

states = {
    s: {
        "label": B.STATE_LABEL[s],
        "amp": B.STATES[s]["amp"], "dur": B.STATES[s]["dur"],
        "fl": B.STATES[s]["fl"], "fd": B.STATES[s]["fd"], "rot": B.STATES[s]["rot"],
    }
    for s in B.STATES
}
uris = {
    s: "data:image/svg+xml;base64," + base64.b64encode((ASSETS / f"bee-{s}.svg").read_bytes()).decode()
    for s in B.STATES
}
rig = (ASSETS / "bee-rig.svg").read_text(encoding="utf-8")
palette = [{"name": k, "hex": v, "role": ROLES[k]} for k, v in B.PALETTE.items()]

TEMPLATE = (ROOT / "tools" / "preview_template.html").read_text(encoding="utf-8")
html = (TEMPLATE
        .replace("/*__STATES__*/null", json.dumps(states, ensure_ascii=False))
        .replace("/*__URIS__*/null", json.dumps(uris))
        .replace("/*__PALETTE__*/null", json.dumps(palette, ensure_ascii=False))
        .replace("<!--__RIG__-->", rig))

out = ROOT / "preview" / "index.html"
out.parent.mkdir(exist_ok=True)
out.write_text(html, encoding="utf-8")
print("OK ->", out, f"({out.stat().st_size/1024:.0f} KB)")
