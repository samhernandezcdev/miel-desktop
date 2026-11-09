import json, random, sys, shutil
from pathlib import Path
W = Path(sys.argv[1]); SRC = Path(sys.argv[2])
if W.exists():
    shutil.rmtree(W)
W.mkdir(parents=True)
shutil.copytree(SRC / "dist", W / "dist"); shutil.copy2(SRC / "server.py", W / "server.py"); (W / "css").mkdir()
R = '.bee[data-state="resting"]'
only_zzz = f'{R} .float,{R} .sway,{R} .wiggle,{R} .flap,{R} .eyes{{animation:none!important}}'
zz_steps = f'{R} .zzz .z-1,{R} .zzz .z-2{{animation-timing-function:steps(16,end)!important}}'   # 10 fps
hh2 = f'{R} .face:not(.face-resting) *{{animation:none!important}}'
V = {
    "C0-base": "",
    "S1-reposo-solo-zzz": only_zzz,
    "S2-reposo-solo-zzz-10fps": only_zzz + "\n" + zz_steps,
    "S3-reposo-solo-zzz-10fps+caras": only_zzz + "\n" + zz_steps + "\n" + hh2,
}
for k, css in V.items():
    (W / "css" / f"{k}.css").write_text(css + "\n", encoding="utf-8")
names = list(V); plan = []
for r in range(1, 6):
    o = names[:]; random.Random(9100 + r).shuffle(o)
    plan += [{"rep": r, "cfg": c} for c in o]
(W / "plan.json").write_text(json.dumps(plan, indent=1), encoding="utf-8")
print(len(plan))
