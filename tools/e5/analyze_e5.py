#!/usr/bin/env python3
"""E5: agrega las mediciones de tools/e5/e5run.ps1 (miel.exe SOLO).

Uso: python -I -X utf8 tools/e5/analyze_e5.py <prefijo> [--md]

Cifra principal: DESPERTARES DEL PLANIFICADOR por minuto y estado (línea `[miel] t=<ms> despertar <estado>` del registro de `diag`:
una por cada vez que el hilo del motor despierta, por plazo, orden o aviso). Es mucho más fiable que el CPU para cifras tan bajas.
- `awake_on` / `awake_off`: estados Idle, Rest y Reaction normales; los despertares se asignan al estado en que estaba el motor y el
  tiempo en cada estado sale de las líneas `vista` (se descartan los primeros 8 s).
- `sleeping`, `suppressed_*`, `hidden_*`, `paused_*`: estado fijo tras el guion (a los 20 s): se cuenta desde 8 s después de que la
  vista confirme el estado hasta el final.
Se da la mediana de las repeticiones y el rango [mín–máx]. También el CPU medio del proceso raíz en la misma ventana (el contador de
Windows avanza en ticks de ~15,6 ms: en cifras tan bajas solo sirve como comprobación gruesa).
"""
import csv
import re
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path

VIEW = re.compile(r"t=(\d+) vista v\d+ (\w+)/(\w+) paused=(\w+) hidden=(\w+) suppressed=(\w+) sleeping=(\w+)")
WAKE = re.compile(r"t=(\d+) despertar (\w+)")
SETTLE_MS = 8_000
FIXED = {
    "sleeping": ("durmiendo", lambda v: v["sleeping"] and not v["hidden"] and not v["paused"] and not v["suppressed"]),
    "suppressed_on": ("suprimida", lambda v: v["suppressed"]),
    "suppressed_off": ("suprimida", lambda v: v["suppressed"]),
    "hidden_on": ("oculta", lambda v: v["hidden"]),
    "hidden_off": ("oculta", lambda v: v["hidden"]),
    "paused_on": ("pausada", lambda v: v["paused"] and not v["hidden"]),
    "paused_off": ("pausada", lambda v: v["paused"] and not v["hidden"]),
}


def parse(path: Path):
    views, wakes = [], []
    for l in path.read_text(encoding="utf-8", errors="replace").splitlines():
        m = VIEW.search(l)
        if m:
            views.append(dict(t=int(m.group(1)), beh=m.group(2), expr=m.group(3), paused=m.group(4) == "true", hidden=m.group(5) == "true",
                              suppressed=m.group(6) == "true", sleeping=m.group(7) == "true"))
            continue
        m = WAKE.search(l)
        if m:
            wakes.append((int(m.group(1)), m.group(2)))
    return views, wakes


def label_of(v):
    if v["hidden"]:
        return "oculta"
    if v["paused"]:
        return "pausada"
    if v["suppressed"]:
        return "suprimida"
    if v["sleeping"]:
        return "durmiendo"
    return {"Idle": "idle", "Follow": "follow", "Rest": "rest", "Reaction": "reaction"}[v["beh"]]


def main(prefix: str, md: bool) -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    rows = list(csv.DictReader(open(prefix + "-samples.csv", encoding="utf-8-sig")))
    ends, cpu = {}, defaultdict(list)
    for r in rows:
        key = (r["variant"], r["rep"])
        ends[key] = max(ends.get(key, 0), int(r["t_ms"]))
        cpu[key].append((int(r["t_ms"]), float(r["cpu_pct"])))
    rate = defaultdict(lambda: defaultdict(list))  # (variante, estado) -> rep -> [tasa]
    cpu_by = defaultdict(lambda: defaultdict(list))
    dur_by = defaultdict(float)
    for (v, rep), t_end in sorted(ends.items()):
        views, wakes = parse(Path(f"{prefix}-logs/{v}-r{rep}.log"))
        if not views:
            continue
        if v in FIXED:
            lab, pred = FIXED[v]
            start = next((x["t"] for x in views if pred(x)), None)
            if start is None:
                continue
            a = start + SETTLE_MS
            n = sum(1 for t, l in wakes if l == lab and a <= t <= t_end)
            minutes = (t_end - a) / 60_000
            if minutes > 0.3:
                rate[(v, lab)][rep].append(n / minutes)
                dur_by[(v, lab)] += minutes
                cpu_by[(v, lab)][rep] = [c for t, c in cpu[(v, rep)] if a <= t <= t_end]
        else:
            a = views[0]["t"] + SETTLE_MS
            span = defaultdict(float)
            for i, x in enumerate(views):
                t1 = views[i + 1]["t"] if i + 1 < len(views) else t_end
                lo, hi = max(x["t"], a), min(t1, t_end)
                if hi > lo:
                    span[label_of(x)] += (hi - lo) / 60_000
            counts = defaultdict(int)
            for t, l in wakes:
                if a <= t <= t_end:
                    counts[l] += 1
            for lab in ("idle", "rest", "reaction"):
                if span[lab] >= 0.25:  # al menos 15 s en ese estado en la repetición
                    rate[(v, lab)][rep].append(counts[lab] / span[lab])
                    dur_by[(v, lab)] += span[lab]
    out = []
    for (v, lab), d in sorted(rate.items()):
        per_rep = {k: st.mean(x) for k, x in d.items()}
        vals = sorted(per_rep.values())
        cm = [c for xs in cpu_by.get((v, lab), {}).values() for c in xs]
        out.append((v, lab, len(vals), st.median(vals), vals[0], vals[-1], dur_by[(v, lab)], st.mean(cm) if cm else None, per_rep))
    if md:
        print("| Variante | Estado | Rep. | Despertares/min (mediana) | Rango | Minutos medidos | CPU medio (% núcleo) |")
        print("|---|---|---|---|---|---|---|")
        for v, lab, n, med, lo, hi, mins, cm, _ in out:
            print(f"| `{v}` | {lab} | {n} | **{med:.2f}** | {lo:.2f}–{hi:.2f} | {mins:.1f} | {'' if cm is None else f'{cm:.3f}'} |")
    else:
        for v, lab, n, med, lo, hi, mins, cm, per in out:
            print(f"{v:15} {lab:10} reps={n} mediana={med:7.2f}/min rango={lo:.2f}-{hi:.2f} min={mins:.1f} cpu={'-' if cm is None else f'{cm:.3f}'}  " + "  ".join(f"r{k}={m:.2f}" for k, m in sorted(per.items())))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], "--md" in sys.argv))
