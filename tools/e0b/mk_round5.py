import json, random, sys, shutil
from pathlib import Path
W = Path(sys.argv[1]); SRC = Path(sys.argv[2])
if W.exists():
    shutil.rmtree(W)
W.mkdir(parents=True)
shutil.copytree(SRC / "dist", W / "dist"); shutil.copy2(SRC / "server.py", W / "server.py"); (W / "css").mkdir()
R = '.bee[data-state="resting"]'
base = f'{R} .float,{R} .flap,{R} .eyes{{animation:none!important}}\n{R} .face:not(.face-resting) *{{animation:none!important}}'
def st(a, b, c):
    return (f'{R} .sway{{animation-timing-function:steps({a},end)!important}}\n'
            f'{R} .antenna-l .wiggle{{animation-timing-function:steps({b},end)!important}}\n'
            f'{R} .antenna-r .wiggle{{animation-timing-function:steps({c},end)!important}}')
V = {"C0-base": "", "S4-reposo-visible-10fps": base + "\n" + st(27, 46, 53),
     "S5-reposo-visible-20fps": base + "\n" + st(54, 92, 106), "S6-reposo-visible-continuo": base}
for k, c in V.items():
    (W / "css" / f"{k}.css").write_text(c + "\n", encoding="utf-8")
names = list(V); plan = []
for r in range(1, 6):
    o = names[:]; random.Random(5500 + r).shuffle(o); plan += [{"rep": r, "cfg": k} for k in o]
(W / "plan.json").write_text(json.dumps(plan), encoding="utf-8")
