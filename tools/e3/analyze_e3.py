#!/usr/bin/env python3
"""Agrega las mediciones de tools/e3/e3run.ps1 (E3) con la MISMA metodologia que tools/e0b/analyze_prod.py.

Uso: python -I -X utf8 tools/e3/analyze_e3.py <prefijo> [--settle 1.0]

- Una muestra (~1 s) se asigna a un estado del motor (Idle, Rest...) solo si el motor permanecio en ese estado todo el
  intervalo y ya habian pasado `settle` s desde que entro (igual que analyze_prod.py).
- Fases (solo variantes con guion de interfaz): A = antes de abrir la ventana, B = con la ventana abierta (se descartan
  los primeros `--guard` s tras abrir), C = tras cerrarla (se descartan los primeros `--guard` s). Las marcas de apertura y
  cierre salen del registro del propio binario (`diag: OpenSettings` / `diag: CloseSettings`, con tiempo Unix en ms).
- Las variantes sin ventana se parten en las mismas fases por tiempo relativo al arranque (mismo reloj de guion), para
  comparar como con como: la «fase» de esas variantes es solo una franja de tiempo, no hubo ventana.
- Por variante y repeticion se toma la MEDIA de sus muestras; por variante se reporta la mediana de esas medias y el rango
  entre repeticiones. Procesos y memoria: mediana de las muestras de la fase.
"""
import csv
import re
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "e0b"))
from analyze_prod import classify, states_from_log  # noqa: E402  (misma clasificacion que E0b/E1a)

MARK = re.compile(r"t=(\d+) diag: (OpenSettings|CloseSettings)")
START = re.compile(r"t=(\d+) ")


def marks(path: Path):
    open_t = close_t = None
    if path.exists():
        for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
            m = MARK.search(raw)
            if m:
                if m.group(2) == "OpenSettings" and open_t is None:
                    open_t = int(m.group(1))
                if m.group(2) == "CloseSettings" and close_t is None:
                    close_t = int(m.group(1))
    return open_t, close_t


def main() -> None:
    prefix = sys.argv[1]
    settle = float(sys.argv[sys.argv.index("--settle") + 1]) if "--settle" in sys.argv else 1.0
    guard = float(sys.argv[sys.argv.index("--guard") + 1]) if "--guard" in sys.argv else 10.0
    rows = list(csv.DictReader(open(prefix + "-samples.csv", encoding="utf-8-sig")))
    runs = defaultdict(list)
    for r in rows:
        runs[(r["variant"], r["rep"])].append(r)
    cpu = defaultdict(lambda: defaultdict(list))  # (variante, fase, rep) -> estado -> [cpu]
    mem = defaultdict(list)  # (variante, fase) -> [private_mb]
    ws = defaultdict(list)
    procs = defaultdict(list)
    ref_marks = {}
    for (v, rep), rs in runs.items():
        log = Path(f"{prefix}-logs/{v}-r{rep}.log")
        states = states_from_log(log)
        o, c = marks(log)
        if o and c:
            ref_marks[(v, rep)] = (o, c)
    # para variantes sin guion de interfaz: usa los instantes relativos (media de las que si lo tienen, por repeticion)
    rel = [(c - o) for (o, c) in ref_marks.values()]
    for (v, rep), rs in runs.items():
        log = Path(f"{prefix}-logs/{v}-r{rep}.log")
        states = states_from_log(log)
        o, c = ref_marks.get((v, rep), (None, None))
        first_t = int(rs[0]["t_ms"]) - float(rs[0]["dt_ms"])
        if o is None:  # sin ventana: mismas franjas que las variantes con ventana, contadas desde el primer muestreo
            offs = [(oo - int(rows_first[0]["t_ms"]) + float(rows_first[0]["dt_ms"]), cc - int(rows_first[0]["t_ms"]) + float(rows_first[0]["dt_ms"]))
                    for (vv, rr), (oo, cc) in ref_marks.items() for rows_first in [runs[(vv, rr)]]]
            if not offs:
                continue
            o = first_t + st.median([a for a, _ in offs])
            c = first_t + st.median([b for _, b in offs])
        for r in rs:
            t1 = int(r["t_ms"])
            t0 = t1 - float(r["dt_ms"])
            if t1 <= o:
                ph = "A"
            elif t0 >= o + guard * 1000 and t1 <= c:
                ph = "B"
            elif t0 >= c + guard * 1000:
                ph = "C"
            else:
                continue
            lab = classify(states, t0, t1, settle * 1000)
            if lab and "+msg" not in lab:
                cpu[(v, ph, rep)][lab].append(float(r["cpu_pct"]))
            mem[(v, ph)].append(float(r["private_mb"]))
            ws[(v, ph)].append(float(r["ws_mb"]) if "ws_mb" in r else float("nan"))
            procs[(v, ph)].append(int(r["procs"]))
    variants = []
    for (v, _r) in runs:
        if v not in variants:
            variants.append(v)
    print("### CPU por estado (% de un nucleo; mediana entre repeticiones de la media por repeticion; [rango])")
    print("| Variante | Fase | Estado | Repeticiones | CPU | Muestras |")
    print("|---|---|---|---|---|---|")
    for v in variants:
        for ph in "ABC":
            for lab in ["Idle", "Rest", "Reaction"]:
                meds, n = [], 0
                for (vv, pp, rep), d in cpu.items():
                    if vv == v and pp == ph and d.get(lab):
                        meds.append(st.mean(d[lab]))
                        n += len(d[lab])
                if meds:
                    print(f"| `{v}` | {ph} | {lab} | {len(meds)} | **{st.median(meds):.2f}** [{min(meds):.2f}–{max(meds):.2f}] | {n} |")
    print()
    print("### Procesos y memoria (mediana de las muestras de la fase; MB privados / working set)")
    print("| Variante | Fase | Procesos (min–max) | Private MB | Working set MB |")
    print("|---|---|---|---|---|")
    for v in variants:
        for ph in "ABC":
            if procs[(v, ph)]:
                p = procs[(v, ph)]
                print(f"| `{v}` | {ph} | {st.median(p):.0f} ({min(p)}–{max(p)}) | {st.median(mem[(v, ph)]):.1f} | {st.median(ws[(v, ph)]):.1f} |")


if __name__ == "__main__":
    main()
