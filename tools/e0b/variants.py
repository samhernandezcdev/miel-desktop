#!/usr/bin/env python3
"""E0b: define las variantes CSS de PRUEBA (se añaden al final del CSS de producción en una copia servida
localmente; el producto y los SVG no se tocan) y el plan de ejecución intercalado."""
import json, random, sys
from pathlib import Path

OUT = Path(sys.argv[1])
NONE = "{animation:none!important}"
V = {
  "C0-base": "",
  "C1-todo-apagado": ".bee,.bee *{animation:none!important;transition:none!important}",
  "C2-sin-flotacion": ".bee .float" + NONE,
  "C3-sin-balanceo": ".bee .sway" + NONE,
  "C4-sin-aleteo": ".bee .flap" + NONE,
  "C5-sin-antenas": ".bee .wiggle" + NONE,
  "C6-sin-parpadeo": ".bee .eyes" + NONE,
  "C7-sin-zzz": ".bee .zzz .z-1,.bee .zzz .z-2" + NONE,
  # (a) compositor
  "Ha1-promover-capa": ".stage .sprite,.stage .sprite>svg{will-change:transform}",
  "Ha2-float-sway-en-raiz": (
    ".bee .float,.bee .sway" + NONE + "\n"
    ".stage .sprite>svg{transform-origin:50% 57.8125%;will-change:translate,rotate;"
    "animation:e0b-float var(--float-dur) ease-in-out infinite,e0b-sway 5.4s ease-in-out infinite}\n"
    "@keyframes e0b-float{0%,100%{translate:0 0}50%{translate:0 calc(var(--float-amp) * -1)}}\n"
    "@keyframes e0b-sway{0%,100%{rotate:-1.6deg}50%{rotate:1.6deg}}"),
  # (b) amplitud 0 en reposo
  "Hb1-reposo-sin-aleteo-0": '.bee[data-state="resting"] .flap' + NONE,
  "Hb2-reposo-sin-aleteo-ni-flot-0": '.bee[data-state="resting"] .flap,.bee[data-state="resting"] .float' + NONE,
  # (c) steps()
  "Hc1-steps8": ".bee .float,.bee .sway,.bee .wiggle,.bee .flap,.bee .zzz .z-1,.bee .zzz .z-2{animation-timing-function:steps(8,end)!important}",
  # (d) contain
  "Hd1-contain": ".stage .sprite{contain:strict}",
  # (e) filtros/recortes
  "He1-sin-clip-path": ".bee *{clip-path:none!important}",
  # caras ocultas (opacity 0) que siguen animando
  "Hh1-caras-inactivas-display-none": (
    ".bee .face{display:none!important}\n"
    '.bee[data-state="idle"] .face-idle,.bee[data-state="resting"] .face-resting{display:inline!important}'),
  "Hh2-caras-inactivas-sin-animacion": (
    '.bee[data-state="idle"] .face:not(.face-idle) *,.bee[data-state="resting"] .face:not(.face-resting) *{animation:none!important}'),
}
extra = Path(sys.argv[3]) if len(sys.argv) > 3 else None
if extra and extra.exists():
    V.update(json.loads(extra.read_text(encoding="utf-8")))
(OUT / "css").mkdir(parents=True, exist_ok=True)
for k, css in V.items():
    (OUT / "css" / f"{k}.css").write_text(css + "\n", encoding="utf-8")
reps = int(sys.argv[2])
names = list(V)
plan = []
for r in range(1, reps + 1):
    order = names[:]
    random.Random(2026_11 + r).shuffle(order)  # orden distinto en cada repetición (intercalado)
    plan += [{"rep": r, "cfg": n} for n in order]
(OUT / "plan.json").write_text(json.dumps(plan, indent=1), encoding="utf-8")
print(len(names), "variantes,", len(plan), "ejecuciones")
