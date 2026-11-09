#!/usr/bin/env python3
"""Agrega las mediciones de tools/e0b/prodrun.ps1 por estado del motor (registro de stderr del propio binario).

Uso: python -I -X utf8 tools/e0b/analyze_prod.py <prefijo> [--settle 1.0] [--md]

Una muestra (intervalo de ~1 s) se asigna a un estado SOLO si el motor permaneció en ese estado durante TODO el
intervalo y ya habían pasado `settle` segundos desde que entró en él (se descartan los fundidos y transiciones de 0,3-0,7 s).
El contador de CPU de Windows avanza en ticks de ~15,6 ms, así que una muestra de 1 s toma valores discretos (1,67 %, 3,34 %...):
por eso, por variante y repetición se toma la MEDIA de sus muestras (no la mediana, que se queda en un escalón); por variante se
reporta la mediana de esas medias y el rango [mínimo-máximo] entre repeticiones.
"""
import csv
import re
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path

LINE = re.compile(r"t=(\d+) vista v\d+ (\w+)/(\w+) paused=(\w+) hidden=(\w+) follow=(\w+) msg=(\S+)")
ORDER = ["Idle", "Rest", "Sleepy", "Reaction", "Paused", "Hidden"]


def states_from_log(path: Path):
    out = []
    if not path.exists():
        return out
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        m = LINE.search(raw)
        if not m:
            continue
        t, beh, expr, paused, hidden, _f, msg = m.groups()
        if hidden == "true":
            lab = "Hidden"
        elif paused == "true":
            lab = "Paused"
        elif beh == "Rest":
            lab = "Rest" if expr == "Resting" else "Sleepy"
        elif beh == "Idle":
            lab = "Idle"
        else:
            lab = beh
        if msg.startswith("Some"):
            lab += "+msg"
        out.append((int(t), lab))
    return out


def classify(states, t0, t1, settle_ms):
    """Etiqueta del intervalo (t0, t1] si el estado fue único y ya estable; si no, None."""
    cur, since = None, None
    for ts, lab in states:
        if ts <= t0:
            cur, since = lab, ts
        elif ts < t1:
            return None  # hubo un cambio dentro del intervalo
    if cur is None or since is None:
        return None
    return cur if t0 - since >= settle_ms else None


def main() -> None:
    prefix = sys.argv[1]
    settle = 1.0
    if "--settle" in sys.argv:
        settle = float(sys.argv[sys.argv.index("--settle") + 1])
    rows = list(csv.DictReader(open(prefix + "-samples.csv", encoding="utf-8-sig")))
    per = defaultdict(lambda: defaultdict(list))  # (variante, rep) -> estado -> [cpu]
    mem = defaultdict(list)
    runs = defaultdict(list)
    for r in rows:
        runs[(r["variant"], r["rep"])].append(r)
    for (v, rep), rs in runs.items():
        states = states_from_log(Path(f"{prefix}-logs/{v}-r{rep}.log"))
        prev_t = None
        for r in rs:
            t1 = int(r["t_ms"])
            t0 = t1 - float(r["dt_ms"])
            lab = classify(states, t0, t1, settle * 1000)
            if lab and "+msg" not in lab:
                per[(v, rep)][lab].append(float(r["cpu_pct"]))
                mem[(v, lab)].append(float(r["private_mb"]))
    variants = []
    for (v, _rep) in runs:
        if v not in variants:
            variants.append(v)
    print(f"| Variante | Estado | Repeticiones (con datos) | Mediana de medias por repetición | Rango entre repeticiones | Muestras (total) | Private MB (mediana) |")
    print("|---|---|---|---|---|---|---|")
    for v in variants:
        for lab in ORDER:
            meds, n = [], 0
            for (vv, rep), d in per.items():
                if vv == v and d.get(lab):
                    meds.append(st.mean(d[lab]))
                    n += len(d[lab])
            if not meds:
                continue
            pm = st.median(mem[(v, lab)]) if mem[(v, lab)] else float("nan")
            print(f"| `{v}` | {lab} | {len(meds)} | **{st.median(meds):.2f}** | {min(meds):.2f}–{max(meds):.2f} | {n} | {pm:.0f} |")


if __name__ == "__main__":
    main()
