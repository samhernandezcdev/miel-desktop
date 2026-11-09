import json, random, sys, shutil
from pathlib import Path

W = Path(sys.argv[1])
SRC = Path(sys.argv[2])  # work2 (reutiliza dist y server)
if W.exists():
    shutil.rmtree(W)
W.mkdir(parents=True)
shutil.copytree(SRC / "dist", W / "dist")
shutil.copy2(SRC / "server.py", W / "server.py")
(W / "css").mkdir()

def aligned(fps):
    u = 1.0 / fps
    def n(sec):
        return max(1, round(sec / u))
    flap = n(0.34) * u        # duración del aleteo redondeada a un múltiplo del paso común
    t = "animation-timing-function:steps(%d,end)!important"
    out = []
    for st, fdur in (("idle", 3.8), ("resting", 4.0)):
        s = f'.bee[data-state="{st}"]'
        if st == "idle":
            out.append(f'{s}{{--wing-dur:{flap:.7f}s!important}}')
        out.append(f"{s} .float{{{t % n(fdur / 2)}}}")
        out.append(f"{s} .sway{{{t % n(5.4 / 2)}}}")
        out.append(f"{s} .antenna-l .wiggle{{{t % n(4.6)}}}")
        out.append(f"{s} .antenna-r .wiggle{{{t % n(5.3)}}}")
        out.append(f"{s} .flap{{{t % n(flap if st == 'idle' else 1.0)}}}")
        out.append(f"{s} .zzz .z-1,{s} .zzz .z-2{{{t % n(3.2 / 2)}}}")
    return "\n".join(out)

hh2 = ('.bee[data-state="idle"] .face:not(.face-idle) *,.bee[data-state="resting"] .face:not(.face-resting) *{animation:none!important}')
hb2 = '.bee[data-state="resting"] .flap,.bee[data-state="resting"] .float{animation:none!important}'
V = {
    "C0-base": "",
    "A30-alineado-30fps": aligned(30),
    "A20-alineado-20fps": aligned(20),
    "A30c-alineado-30fps+caras+reposo-0": aligned(30) + "\n" + hh2 + "\n" + hb2,
    "A20c-alineado-20fps+caras+reposo-0": aligned(20) + "\n" + hh2 + "\n" + hb2,
}
for k, css in V.items():
    (W / "css" / f"{k}.css").write_text(css + "\n", encoding="utf-8")
names = list(V)
plan = []
for r in range(1, 6):
    o = names[:]
    random.Random(4242 + r).shuffle(o)
    plan += [{"rep": r, "cfg": c} for c in o]
(W / "plan.json").write_text(json.dumps(plan, indent=1), encoding="utf-8")
print(len(plan), "ejecuciones")
print(V["A30-alineado-30fps"][:300])
