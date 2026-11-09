import json, sys
from pathlib import Path

def steps_css(fps):
    # pasos por INTERVALO de keyframes (la función de temporización se aplica entre keyframes)
    def n(sec):
        return max(1, round(sec * fps))
    t = "animation-timing-function:steps(%d,end)!important"
    rules = []
    for st, fdur, flap in (("idle", 3.8, 0.34), ("resting", 4.0, 1.0)):
        s = f'.bee[data-state="{st}"]'
        rules.append(f"{s} .float{{{t % n(fdur / 2)}}}")
        rules.append(f"{s} .sway{{{t % n(5.4 / 2)}}}")
        rules.append(f"{s} .antenna-l .wiggle{{{t % n(4.6)}}}")
        rules.append(f"{s} .antenna-r .wiggle{{{t % n(5.3)}}}")
        rules.append(f"{s} .flap{{{t % n(flap)}}}")
        rules.append(f"{s} .zzz .z-1,{s} .zzz .z-2{{{t % n(3.2 / 2)}}}")
    return "\n".join(rules)

hh2 = ('.bee[data-state="idle"] .face:not(.face-idle) *,.bee[data-state="resting"] .face:not(.face-resting) *{animation:none!important}')
hb2 = ('.bee[data-state="resting"] .flap,.bee[data-state="resting"] .float{animation:none!important}')
V = {
    "M30-steps-30fps": steps_css(30),
    "M20-steps-20fps": steps_css(20),
    "M60-steps-60fps": steps_css(60),
    "M30c-30fps+caras+reposo-0": steps_css(30) + "\n" + hh2 + "\n" + hb2,
}
Path(sys.argv[1]).write_text(json.dumps(V, indent=1), encoding="utf-8")
print(list(V))
