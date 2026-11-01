#!/usr/bin/env python3
"""
Miel — generador de la mascota (abeja 2D original).

Fuente única de verdad del personaje: de aquí salen
  - assets/bee/bee-<estado>.svg   (8 estados, estáticos, sin dependencias)
  - assets/bee/bee-rig.svg        (maestro con los 8 rostros + CSS embebido)
  - assets/bee/bee.css            (animaciones y tabla de estados)
  - assets/bee/palette.json

Uso:  python3 tools/build_bee.py
Sólo usa la biblioteca estándar.
"""
from __future__ import annotations

import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "bee"

# ───────────────────────────── Paleta ─────────────────────────────
PALETTE = {
    "ink":        "#3B2A21",  # contorno, antenas, patas, rayas, ojos
    "honey":      "#F7B538",  # color base del cuerpo
    "honeyLight": "#FFD56B",  # brillo y puntas de antena
    "honeyShade": "#E8962A",  # sombra plana inferior derecha
    "cream":      "#FFF1CC",  # pelusa del cuello, burbujas
    "rim":        "#FFF7E0",  # halo claro (legible sobre fondos oscuros)
    "blush":      "#F28B72",  # mejillas
    "tongue":     "#EC6F5B",  # lengua
    "wingFill":   "#FFFFFF",  # alas (opacidad .78)
    "wingStroke": "#86AFCF",  # contorno de alas
    "wingVein":   "#B7D2E5",  # nervadura de alas
}
INK = PALETTE["ink"]
HONEY = PALETTE["honey"]
HONEY_LIGHT = PALETTE["honeyLight"]
HONEY_SHADE = PALETTE["honeyShade"]
CREAM = PALETTE["cream"]
RIM = PALETTE["rim"]
BLUSH = PALETTE["blush"]
TONGUE = PALETTE["tongue"]
WING_FILL = PALETTE["wingFill"]
WING_STROKE = PALETTE["wingStroke"]
WING_VEIN = PALETTE["wingVein"]

# ───────────────────────────── Geometría ─────────────────────────────
VIEWBOX = "0 0 128 128"
# Gota de miel: la punta superior se inclina a la izquierda, la panza a la derecha.
BODY = ("M 58,34 C 80,32 101,47 101,70 C 101,92 87,110 68,110 "
        "C 46,110 27,94 27,70 C 27,49 40,35 58,34 Z")

POSE_ORIGIN = (64, 74)
FIT = "translate(64 64) scale(.94) translate(-64 -64)"  # margen de seguridad dentro del viewBox
WING_L_ROOT = (54, 52)
WING_R_ROOT = (74, 52)
ANT_L_ROOT = (52, 40)
ANT_R_ROOT = (75, 40)

WING_L = "M 55,53 C 47,44 36,28 21,23 C 10,20 6,32 14,42 C 22,52 40,59 55,53 Z"
WING_R = "M 73,53 C 81,44 92,28 107,23 C 118,20 122,32 114,42 C 106,52 88,59 73,53 Z"
VEIN_L = "M 51,50 C 40,40 29,31 17,28"
VEIN_R = "M 77,50 C 88,40 99,31 111,28"

ANT_L = "M 52,41 C 49,30 47,23 42,16"
ANT_R = "M 75,41 C 77,30 82,24 89,19"
ANT_L_TIP = (42, 16)  # hexágono (firma del personaje)
ANT_R_TIP = (89, 19)  # círculo

LEGS = [
    "M 49,102 L 46,117",
    "M 63,104 L 63,119",
    "M 78,102 L 81,117",
]

EYE_L = (50, 57)
EYE_R = (78, 57)

# ───────────────────────────── Estados ─────────────────────────────
# dx/dy en unidades del viewBox, rot en grados, sx/sy escala del cuerpo.
# wing: ángulo base (+ = alas levantadas). amp/dur: aleteo.
# ant_l/ant_r: rotación (° CW) de cada antena. fl/fd: amplitud y duración de flotación.
STATES = {
    "idle":      dict(dx=1,  dy=0,  rot=-3, sx=1.0,  sy=1.0,  wing=0,   amp=7,   dur=0.34, ant_l=0,   ant_r=0,   fl=3.0, fd=3.8),
    "happy":     dict(dx=2,  dy=-2, rot=-5, sx=1.0,  sy=1.0,  wing=6,   amp=9,   dur=0.28, ant_l=-6,  ant_r=6,   fl=4.0, fd=3.2),
    "curious":   dict(dx=-1, dy=0,  rot=6,  sx=1.0,  sy=1.0,  wing=0,   amp=6,   dur=0.36, ant_l=12,  ant_r=10,  fl=2.5, fd=3.8),
    "thinking":  dict(dx=-1, dy=1,  rot=-5, sx=1.0,  sy=1.0,  wing=-6,  amp=5,   dur=0.42, ant_l=0,   ant_r=-14, fl=2.5, fd=4.2),
    "sleepy":    dict(dx=-1, dy=4,  rot=5,  sx=1.0,  sy=0.98, wing=-16, amp=2.5, dur=0.70, ant_l=-24, ant_r=24,  fl=1.5, fd=5.5),
    "surprised": dict(dx=0,  dy=-3, rot=0,  sx=1.03, sy=1.03, wing=14,  amp=12,  dur=0.22, ant_l=10,  ant_r=-10, fl=1.0, fd=3.0),
    "talking":   dict(dx=1,  dy=0,  rot=-2, sx=1.0,  sy=1.0,  wing=2,   amp=6,   dur=0.32, ant_l=0,   ant_r=0,   fl=2.5, fd=3.6),
    "resting":   dict(dx=0,  dy=4,  rot=0,  sx=1.05, sy=0.93, wing=-62, amp=0,   dur=1.00, ant_l=-34, ant_r=34,  fl=0.0, fd=4.0),
}
STATE_LABEL = {
    "idle": "flotando tranquila", "happy": "contenta", "curious": "curiosa",
    "thinking": "pensativa", "sleepy": "con sueño", "surprised": "sorprendida",
    "talking": "hablando", "resting": "descansando",
}


def n(v: float) -> str:
    """Número compacto para SVG."""
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def hexagon(cx: float, cy: float, r: float, rot: float = -90) -> str:
    pts = []
    for i in range(6):
        a = math.radians(rot + 60 * i)
        pts.append(f"{cx + r * math.cos(a):.1f},{cy + r * math.sin(a):.1f}")
    return " ".join(pts)


# ───────────────────────────── Partes ─────────────────────────────
def pose_attr(st: dict) -> str:
    ox, oy = POSE_ORIGIN
    return (f"translate({n(st['dx'])} {n(st['dy'])}) translate({ox} {oy}) "
            f"rotate({n(st['rot'])}) scale({n(st['sx'])} {n(st['sy'])}) "
            f"translate({-ox} {-oy})")


def wings(p: str, st: dict | None) -> str:
    wl = f' transform="rotate({n(st["wing"])} {WING_L_ROOT[0]} {WING_L_ROOT[1]})"' if st else ""
    wr = f' transform="rotate({n(-st["wing"])} {WING_R_ROOT[0]} {WING_R_ROOT[1]})"' if st else ""

    def wing(side: str, d: str, vein: str, attr: str) -> str:
        return (
            f'<g id="{p}wing-{side}" class="wing wing-{side[0]}"{attr}>'
            f'<g class="flap">'
            f'<path d="{d}" fill="{WING_FILL}" fill-opacity=".78" stroke="{WING_STROKE}" '
            f'stroke-width="1.6" stroke-linejoin="round"/>'
            f'<path d="{vein}" fill="none" stroke="{WING_VEIN}" stroke-width="1.4" stroke-linecap="round"/>'
            f'</g></g>'
        )

    return (f'<g id="{p}wings">' + wing("left", WING_L, VEIN_L, wl) +
            wing("right", WING_R, VEIN_R, wr) + "</g>")


def antennae(p: str, st: dict | None) -> str:
    al = f' transform="rotate({n(st["ant_l"])} {ANT_L_ROOT[0]} {ANT_L_ROOT[1]})"' if st else ""
    ar = f' transform="rotate({n(st["ant_r"])} {ANT_R_ROOT[0]} {ANT_R_ROOT[1]})"' if st else ""
    lx, ly = ANT_L_TIP
    rx, ry = ANT_R_TIP
    left = (
        f'<g id="{p}antenna-left" class="antenna antenna-l"{al}><g class="wiggle">'
        f'<path d="{ANT_L}" fill="none" stroke="{RIM}" stroke-width="6.2" stroke-linecap="round"/>'
        f'<polygon points="{hexagon(lx, ly, 4.2)}" fill="{RIM}" stroke="{RIM}" stroke-width="3.6" stroke-linejoin="round"/>'
        f'<path d="{ANT_L}" fill="none" stroke="{INK}" stroke-width="2.8" stroke-linecap="round"/>'
        f'<polygon points="{hexagon(lx, ly, 4.2)}" fill="{HONEY_LIGHT}" stroke="{INK}" stroke-width="2" stroke-linejoin="round"/>'
        f'</g></g>'
    )
    right = (
        f'<g id="{p}antenna-right" class="antenna antenna-r"{ar}><g class="wiggle">'
        f'<path d="{ANT_R}" fill="none" stroke="{RIM}" stroke-width="6.2" stroke-linecap="round"/>'
        f'<circle cx="{rx}" cy="{ry}" r="3.6" fill="{RIM}" stroke="{RIM}" stroke-width="3.2"/>'
        f'<path d="{ANT_R}" fill="none" stroke="{INK}" stroke-width="2.8" stroke-linecap="round"/>'
        f'<circle cx="{rx}" cy="{ry}" r="3.5" fill="{HONEY_LIGHT}" stroke="{INK}" stroke-width="2"/>'
        f'</g></g>'
    )
    return f'<g id="{p}antennae">{left}{right}</g>'


def legs(p: str) -> str:
    rim = "".join(f'<path d="{d}" stroke="{RIM}" stroke-width="6.4"/>' for d in LEGS)
    ink = "".join(f'<path d="{d}" stroke="{INK}" stroke-width="3.2"/>' for d in LEGS)
    return (f'<g id="{p}legs" fill="none" stroke-linecap="round">{rim}{ink}</g>')


def band(y_end: float, thick: float, bow: float, tilt: float = 0.0,
         x0: float = 16, x1: float = 112) -> str:
    """Franja curva (convexa hacia abajo) que sigue la redondez del cuerpo."""
    xm = (x0 + x1) / 2
    yl, yr = y_end, y_end + tilt
    return (f"M {x0},{n(yl)} Q {xm},{n((yl + yr) / 2 + 2 * bow)} {x1},{n(yr)} "
            f"L {x1},{n(yr + thick)} Q {xm},{n((yl + yr) / 2 + thick + 2 * bow)} {x0},{n(yl + thick)} Z")


def collar_path(y_end: float = 70, thick: float = 7, bow: float = 6,
                x0: float = 16, x1: float = 112, scallops: int = 8) -> str:
    """Pelusa del cuello: borde superior curvo y borde inferior festoneado."""
    xm = (x0 + x1) / 2
    half = (x1 - x0) / 2

    def y_base(x: float) -> float:
        return y_end + thick + bow * (1 - ((x - xm) / half) ** 2)

    d = f"M {x0},{n(y_end)} Q {xm},{n(y_end + 2 * bow)} {x1},{n(y_end)} L {x1},{n(y_base(x1))}"
    step = (x1 - x0) / scallops
    chord = step
    r = chord * 0.56
    for i in range(1, scallops + 1):
        x = x1 - step * i
        d += f" A {n(r)} {n(r)} 0 0 1 {n(x)},{n(y_base(x))}"
    return d + " Z"


def body(p: str) -> str:
    return (
        f'<g id="{p}body" class="body">'
        f'<path d="{BODY}" fill="{RIM}" stroke="{RIM}" stroke-width="7" stroke-linejoin="round"/>'
        f'<path d="{BODY}" fill="{HONEY_SHADE}"/>'
        f'<g clip-path="url(#{p}body-clip)">'
        f'<path d="{BODY}" transform="translate(-4 -5)" fill="{HONEY}"/>'
        f'<ellipse cx="44" cy="42" rx="9" ry="4.2" transform="rotate(-28 44 42)" fill="{HONEY_LIGHT}" opacity=".9"/>'
        f'<circle cx="57" cy="38.5" r="1.7" fill="{HONEY_LIGHT}" opacity=".9"/>'
        f'<g id="{p}stripes" fill="{INK}">'
        f'<path d="{band(79, 7, 6, tilt=1.5)}"/>'
        f'<path d="{band(92, 7, 6, tilt=1.5)}"/>'
        f'</g>'
        f'<path id="{p}collar" d="{collar_path()}" fill="{CREAM}"/>'
        f'</g>'
        f'<path d="{BODY}" fill="none" stroke="{INK}" stroke-width="3" stroke-linejoin="round"/>'
        f'</g>'
    )


# ───────────────────────────── Rostro ─────────────────────────────
def eye_open(p: str, side: str, sfx: str, c: tuple, rx=7.0, ry=8.6,
             look=(0.0, 0.0), hl=1.0) -> str:
    lx, ly = look
    return (
        f'<g id="{p}eye-{side}{sfx}" transform="translate({c[0]} {c[1]})">'
        f'<ellipse cx="{n(lx * .6)}" cy="{n(ly * .6)}" rx="{n(rx)}" ry="{n(ry)}" fill="{INK}"/>'
        f'<ellipse cx="{n(-2.3 + lx)}" cy="{n(-3.3 + ly)}" rx="{n(2.6 * hl)}" ry="{n(2.9 * hl)}" fill="#fff"/>'
        f'<circle cx="{n(2.5 + lx * .5)}" cy="{n(3.5 + ly * .5)}" r="{n(1.2 * hl)}" fill="#fff" opacity=".85"/>'
        f'</g>'
    )


def eye_arc(p: str, side: str, sfx: str, c: tuple, d: str) -> str:
    return (f'<g id="{p}eye-{side}{sfx}" transform="translate({c[0]} {c[1]})">'
            f'<path d="{d}" fill="none" stroke="{INK}" stroke-width="3.4" stroke-linecap="round"/></g>')


def eye_sleepy(p: str, side: str, sfx: str, c: tuple) -> str:
    # Ojo completo tapado por un párpado del color del rostro; la comisura externa cae.
    left = side == "left"
    y_out, y_in = 0.9, -2.6               # comisura externa más baja que la interna
    yl_end, yr_end = (y_out, y_in) if left else (y_in, y_out)
    yl_edge, yr_edge = yl_end + 0.3, yr_end + 0.3
    lid_line = f"M -8.4,{n(yl_end)} Q 0,-1.1 8.4,{n(yr_end)}"
    lid_fill = (f"M -10,-13 L 10,-13 L 10,{n(yr_edge)} L 8.4,{n(yr_end)} "
                f"Q 0,-1.1 -8.4,{n(yl_end)} L -10,{n(yl_edge)} Z")
    return (
        f'<g id="{p}eye-{side}{sfx}" transform="translate({c[0]} {c[1] + 3})">'
        f'<ellipse cx="0" cy="0" rx="7" ry="8.6" fill="{INK}"/>'
        f'<ellipse cx="{n(-2.2 if left else 2.2)}" cy="3.2" rx="2.1" ry="2.3" fill="#fff"/>'
        f'<circle cx="{n(2.4 if left else -2.4)}" cy="5.6" r="1" fill="#fff" opacity=".85"/>'
        f'<path d="{lid_fill}" fill="{HONEY}"/>'
        f'<path d="{lid_line}" fill="none" stroke="{INK}" stroke-width="3" stroke-linecap="round"/>'
        f'</g>'
    )


def brow(p: str, side: str, sfx: str, d: str) -> str:
    return (f'<path id="{p}brow-{side}{sfx}" d="{d}" fill="none" stroke="{INK}" '
            f'stroke-width="2.6" stroke-linecap="round"/>')


def cheeks(p: str, sfx: str, k: float) -> str:
    return (f'<g id="{p}cheeks{sfx}" opacity="{k}">'
            f'<ellipse cx="37" cy="68" rx="5.6" ry="3.4" fill="{BLUSH}"/>'
            f'<ellipse cx="91" cy="67" rx="5.2" ry="3.2" fill="{BLUSH}"/></g>')


def line_mouth(p: str, sfx: str, d: str, cls: str = "mouth") -> str:
    return (f'<path id="{p}mouth{sfx}" class="{cls}" d="{d}" fill="none" stroke="{INK}" '
            f'stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/>')


def open_mouth(p: str, sfx: str, path_d: str, pos: tuple, tongue: tuple | None,
               cls: str = "mouth", extra_attr: str = "") -> str:
    tongue_svg = ""
    if tongue:
        tcy, trx, try_ = tongue
        tongue_svg = (f'<ellipse cx="0" cy="{n(tcy)}" rx="{n(trx)}" ry="{n(try_)}" fill="{TONGUE}" '
                      f'clip-path="url(#{p}mclip{sfx}-{cls})"/>')
    return (
        f'<g id="{p}{cls}{sfx}" class="{cls}" transform="translate({pos[0]} {pos[1]})"{extra_attr}>'
        f'<clipPath id="{p}mclip{sfx}-{cls}"><path d="{path_d}"/></clipPath>'
        f'<path d="{path_d}" fill="{INK}" stroke="{INK}" stroke-width="2" stroke-linejoin="round"/>'
        f'{tongue_svg}</g>'
    )


def rim_stroke(d: str, w: float) -> str:
    return (f'<path d="{d}" fill="none" stroke="{RIM}" stroke-width="{n(w + 3.2)}" '
            f'stroke-linecap="round" stroke-linejoin="round"/>'
            f'<path d="{d}" fill="none" stroke="{INK}" stroke-width="{n(w)}" '
            f'stroke-linecap="round" stroke-linejoin="round"/>')


def face(state: str, p: str, sfx: str = "") -> str:
    """Devuelve el contenido del rostro (sin el <g> contenedor)."""
    eyes_cls = "eyes"
    parts: list[str] = []
    extras = ""

    if state == "idle":
        eyes = (eye_open(p, "left", sfx, EYE_L) +
                eye_open(p, "right", sfx, EYE_R, ry=8.9))
        parts += [cheeks(p, sfx, .55), "{EYES}",
                  line_mouth(p, sfx, "M 59,68 Q 64,73 69,68")]
    elif state == "happy":
        eyes_cls += " no-blink"
        d = "M -7,2.5 Q 0,-7.5 7,2.5"
        eyes = (eye_arc(p, "left", sfx, (EYE_L[0], 58), d) +
                eye_arc(p, "right", sfx, (EYE_R[0], 58), d))
        parts += [cheeks(p, sfx, .8), "{EYES}",
                  open_mouth(p, sfx, "M -6.5,0 Q 0,11 6.5,0 Z", (64, 66), (5.0, 3.8, 2.4))]
    elif state == "curious":
        eyes = (eye_open(p, "left", sfx, (51, 57), look=(1.8, -1.2)) +
                eye_open(p, "right", sfx, (79, 57), ry=9.4, rx=7.4, look=(1.8, -1.2)))
        parts += [cheeks(p, sfx, .5), "{EYES}",
                  brow(p, "right", sfx, "M 71,44 Q 79,38.5 87,43"),
                  open_mouth(p, sfx, "M 0,-3.2 C 2.6,-3.2 2.8,3.2 0,3.2 C -2.8,3.2 -2.6,-3.2 0,-3.2 Z", (69, 69.5), None)]
    elif state == "thinking":
        eyes = (eye_open(p, "left", sfx, EYE_L, look=(-2.2, -2.2)) +
                eye_open(p, "right", sfx, EYE_R, ry=7.4, look=(-2.2, -2.0)))
        parts += [cheeks(p, sfx, .45), "{EYES}",
                  brow(p, "left", sfx, "M 43,47 L 57,45.5"),
                  brow(p, "right", sfx, "M 71,45 Q 79,39.5 87,43.5"),
                  line_mouth(p, sfx, "M 59,69.5 Q 61.5,67 64,69.5 T 69,69.5")]
        extras = (
            f'<g id="{p}thought{sfx}" class="extras">'
            f'<circle cx="92" cy="28" r="1.8" fill="{CREAM}" stroke="{INK}" stroke-width="1.5"/>'
            f'<circle cx="97.5" cy="21.5" r="2.7" fill="{CREAM}" stroke="{INK}" stroke-width="1.7"/>'
            f'<ellipse cx="107" cy="11.5" rx="9" ry="7.3" fill="{CREAM}" stroke="{INK}" stroke-width="2"/>'
            f'<circle cx="102.8" cy="11.5" r="1.3" fill="{INK}"/>'
            f'<circle cx="107" cy="11.5" r="1.3" fill="{INK}"/>'
            f'<circle cx="111.2" cy="11.5" r="1.3" fill="{INK}"/></g>')
    elif state == "sleepy":
        eyes = (eye_sleepy(p, "left", sfx, EYE_L) + eye_sleepy(p, "right", sfx, EYE_R))
        parts += [cheeks(p, sfx, .6), "{EYES}",
                  open_mouth(p, sfx, "M 0,-3.4 C 2.8,-3.4 2.8,3.4 0,3.4 C -2.8,3.4 -2.8,-3.4 0,-3.4 Z", (64, 70.5), None)]
        z1 = "M 64,24 h 6 l -6,6 h 6"
        z2 = "M 74,9 h 8 l -8,8 h 8"
        extras = (f'<g id="{p}zzz{sfx}" class="extras zzz">'
                  f'<g class="z-1">{rim_stroke(z1, 2.2)}</g><g class="z-2">{rim_stroke(z2, 2.4)}</g></g>')
    elif state == "surprised":
        eyes = (eye_open(p, "left", sfx, (49, 57), rx=8, ry=10, hl=.9) +
                eye_open(p, "right", sfx, (79, 57), rx=8.2, ry=10.4, hl=.9))
        parts += [cheeks(p, sfx, .4), "{EYES}",
                  brow(p, "left", sfx, "M 41,42.5 Q 49,37 58,41.5"),
                  brow(p, "right", sfx, "M 70,41.5 Q 79,37 87,42.5"),
                  f'<circle id="{p}mouth{sfx}" class="mouth" cx="64" cy="70" r="3.7" fill="{INK}"/>']
        extras = (f'<g id="{p}burst{sfx}" class="extras">'
                  f'{rim_stroke("M 64,14 L 64,8", 2.2)}'
                  f'{rim_stroke("M 57,16 L 54,11", 2.2)}'
                  f'{rim_stroke("M 71,16 L 74,11", 2.2)}</g>')
    elif state == "talking":
        eyes = (eye_open(p, "left", sfx, EYE_L, look=(0.8, 0)) +
                eye_open(p, "right", sfx, EYE_R, ry=8.9, look=(0.8, 0)))
        mouth_a = open_mouth(p, sfx, "M 0,-5 C 4.8,-5 5,5.4 0,5.4 C -5,5.4 -4.8,-5 0,-5 Z",
                             (64, 69), (3.2, 3.4, 2.2), cls="mouth-a", extra_attr=' opacity="1"')
        mouth_b = line_mouth(p, sfx, "M 59,68.5 Q 64,72 69,68.5", cls="mouth-b")
        mouth_b = mouth_b.replace("<path ", '<path opacity="0" ', 1)
        parts += [cheeks(p, sfx, .6), "{EYES}", mouth_a, mouth_b]
    elif state == "resting":
        eyes_cls += " no-blink"
        d = "M -7,-1.5 Q 0,6 7,-1.5"
        eyes = (eye_arc(p, "left", sfx, (EYE_L[0], 59), d) +
                eye_arc(p, "right", sfx, (EYE_R[0], 59), d))
        parts += [cheeks(p, sfx, .75), "{EYES}",
                  line_mouth(p, sfx, "M 60,69 Q 64,72.5 68,69")]
    else:
        raise ValueError(state)

    eyes_g = f'<g id="{p}eyes{sfx}" class="{eyes_cls}">{eyes}</g>'
    inner = "".join(parts).replace("{EYES}", eyes_g)
    return inner + extras


# ───────────────────────────── Ensamblado ─────────────────────────────
DEFS = lambda p: f'<defs><clipPath id="{p}body-clip"><path d="{BODY}"/></clipPath></defs>'


def build_state_svg(state: str, p: str = "") -> str:
    st = STATES[state]
    label = STATE_LABEL[state]
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{VIEWBOX}" width="128" height="128" '
        f'class="bee" data-state="{state}" role="img" aria-labelledby="{p}title">\n'
        f'<title id="{p}title">Miel, la abeja — {label}</title>\n'
        f'{DEFS(p)}\n'
        f'<g id="{p}fit" transform="{FIT}"><g id="{p}bee" class="float"><g class="sway">'
        f'<g id="{p}pose" class="pose" transform="{pose_attr(st)}">\n'
        f'{wings(p, st)}\n{antennae(p, st)}\n{legs(p)}\n{body(p)}\n'
        f'<g id="{p}face" class="face face-{state}">{face(state, p)}</g>\n'
        f'</g></g></g></g>\n</svg>\n'
    )


def build_rig_svg(css: str, p: str = "") -> str:
    """Rig maestro. `p` antepone un prefijo a todos los ids (útil si montas varias abejas a la vez)."""
    faces = "\n".join(
        f'<g id="{p}face-{s}" class="face face-{s}">{face(s, p, "-" + s)}</g>' for s in STATES
    )
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{VIEWBOX}" width="128" height="128" '
        f'class="bee" data-state="idle" role="img" aria-labelledby="{p}title">\n'
        f'<title id="{p}title">Miel, la abeja</title>\n'
        f'<style>\n{css}\n</style>\n'
        f'{DEFS(p)}\n'
        f'<g id="{p}fit" transform="{FIT}"><g id="{p}bee" class="float"><g class="sway"><g id="{p}pose" class="pose">\n'
        f'{wings(p, None)}\n{antennae(p, None)}\n{legs(p)}\n{body(p)}\n'
        f'<g id="{p}faces">\n{faces}\n</g>\n'
        f'</g></g></g></g>\n</svg>\n'
    )


# ───────────────────────────── CSS ─────────────────────────────
def build_css() -> str:
    ox, oy = POSE_ORIGIN
    states_css = []
    for s, st in STATES.items():
        states_css.append(
            f'.bee[data-state="{s}"]{{--dx:{n(st["dx"])}px;--dy:{n(st["dy"])}px;--rot:{n(st["rot"])}deg;'
            f'--sx:{n(st["sx"])};--sy:{n(st["sy"])};--wing-base:{n(st["wing"])}deg;--wing-amp:{n(st["amp"])}deg;'
            f'--wing-dur:{n(st["dur"])}s;--ant-l:{n(st["ant_l"])}deg;--ant-r:{n(st["ant_r"])}deg;'
            f'--float-amp:{n(st["fl"])}px;--float-dur:{n(st["fd"])}s}}'
        )
    show_faces = ",\n".join(f'.bee[data-state="{s}"] .face-{s}' for s in STATES)
    return f"""/* Miel — animaciones base y tabla de estados (generado por tools/build_bee.py) */
.bee{{overflow:visible;--dx:0px;--dy:0px;--rot:0deg;--sx:1;--sy:1;--wing-base:0deg;--wing-amp:7deg;--wing-dur:.34s;--ant-l:0deg;--ant-r:0deg;--float-amp:3px;--float-dur:3.8s}}

/* Capas de movimiento: float → sway → pose (cada una anima una sola propiedad) */
.bee .float{{animation:bee-float var(--float-dur) ease-in-out infinite}}
.bee .sway{{transform-origin:{ox}px {oy}px;animation:bee-sway 5.4s ease-in-out infinite}}
.bee .pose{{transform-origin:{ox}px {oy}px;transform:translate(var(--dx),var(--dy)) rotate(var(--rot)) scale(var(--sx),var(--sy));transition:transform .7s cubic-bezier(.34,.9,.3,1)}}

/* Alas */
.bee .wing-l{{transform-origin:{WING_L_ROOT[0]}px {WING_L_ROOT[1]}px;transform:rotate(var(--wing-base));transition:transform .7s ease}}
.bee .wing-r{{transform-origin:{WING_R_ROOT[0]}px {WING_R_ROOT[1]}px;transform:rotate(calc(var(--wing-base) * -1));transition:transform .7s ease}}
.bee .wing-l .flap{{transform-origin:{WING_L_ROOT[0]}px {WING_L_ROOT[1]}px;animation:bee-flap-l var(--wing-dur) ease-in-out infinite alternate}}
.bee .wing-r .flap{{transform-origin:{WING_R_ROOT[0]}px {WING_R_ROOT[1]}px;animation:bee-flap-r var(--wing-dur) ease-in-out infinite alternate}}

/* Antenas */
.bee .antenna-l{{transform-origin:{ANT_L_ROOT[0]}px {ANT_L_ROOT[1]}px;transform:rotate(var(--ant-l));transition:transform .7s ease}}
.bee .antenna-r{{transform-origin:{ANT_R_ROOT[0]}px {ANT_R_ROOT[1]}px;transform:rotate(var(--ant-r));transition:transform .7s ease}}
.bee .antenna-l .wiggle{{transform-origin:{ANT_L_ROOT[0]}px {ANT_L_ROOT[1]}px;animation:bee-wig 4.6s ease-in-out infinite alternate}}
.bee .antenna-r .wiggle{{transform-origin:{ANT_R_ROOT[0]}px {ANT_R_ROOT[1]}px;animation:bee-wig 5.3s ease-in-out -1.7s infinite alternate-reverse}}

/* Rostro: un grupo por estado, fundido cruzado */
.bee .face{{opacity:0;transition:opacity .3s ease}}
{show_faces}{{opacity:1}}
.bee .eyes{{transform-origin:64px 58px;animation:bee-blink 6s ease-in-out infinite}}
.bee .eyes.no-blink{{animation:none}}
.bee[data-state="talking"] .mouth-a{{animation:bee-talk-a 1.1s step-end infinite}}
.bee[data-state="talking"] .mouth-b{{animation:bee-talk-b 1.1s step-end infinite}}
.bee .zzz .z-1{{animation:bee-zz 3.2s ease-in-out infinite}}
.bee .zzz .z-2{{animation:bee-zz 3.2s ease-in-out .9s infinite}}

@keyframes bee-float{{0%,100%{{transform:translateY(0)}}50%{{transform:translateY(calc(var(--float-amp) * -1))}}}}
@keyframes bee-sway{{0%,100%{{transform:rotate(-1.6deg)}}50%{{transform:rotate(1.6deg)}}}}
@keyframes bee-flap-l{{from{{transform:rotate(calc(var(--wing-amp) * -1))}}to{{transform:rotate(var(--wing-amp))}}}}
@keyframes bee-flap-r{{from{{transform:rotate(var(--wing-amp))}}to{{transform:rotate(calc(var(--wing-amp) * -1))}}}}
@keyframes bee-wig{{from{{transform:rotate(-3deg)}}to{{transform:rotate(3deg)}}}}
@keyframes bee-blink{{0%,92%,100%{{transform:scaleY(1)}}95%{{transform:scaleY(.08)}}}}
@keyframes bee-talk-a{{0%{{opacity:1}}22%{{opacity:0}}38%{{opacity:1}}60%{{opacity:0}}72%{{opacity:1}}88%{{opacity:0}}100%{{opacity:1}}}}
@keyframes bee-talk-b{{0%{{opacity:0}}22%{{opacity:1}}38%{{opacity:0}}60%{{opacity:1}}72%{{opacity:0}}88%{{opacity:1}}100%{{opacity:0}}}}
@keyframes bee-zz{{0%,100%{{opacity:.45;transform:translateY(0)}}50%{{opacity:1;transform:translateY(-2px)}}}}

/* Tabla de estados (variables CSS) */
{chr(10).join(states_css)}

/* Movimiento reducido: sin animaciones ni transiciones, pose estática */
@media (prefers-reduced-motion:reduce){{
  .bee,.bee *{{animation:none!important;transition:none!important}}
  .bee[data-state="talking"] .mouth-b{{opacity:0}}
}}
"""


# ───────────────────────────── Salida ─────────────────────────────
def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    css = build_css()
    for s in STATES:
        (OUT / f"bee-{s}.svg").write_text(build_state_svg(s), encoding="utf-8")
    (OUT / "bee-rig.svg").write_text(build_rig_svg(css), encoding="utf-8")
    (OUT / "bee.css").write_text(css, encoding="utf-8")
    (OUT / "palette.json").write_text(json.dumps(PALETTE, indent=2) + "\n", encoding="utf-8")
    print("OK ->", OUT)
    for f in sorted(OUT.iterdir()):
        print(f"  {f.name:20s} {f.stat().st_size:6d} B")


if __name__ == "__main__":
    main()
