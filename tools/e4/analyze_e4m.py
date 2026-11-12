#!/usr/bin/env python3
"""E4b: agrega las mediciones de tools/e4/e4run.ps1 (CPU de miel.exe SOLO, % de un núcleo).

Uso: python -I -X utf8 tools/e4/analyze_e4m.py <prefijo> [--md]

- `dnd_on` / `dnd_off`: Idle y Rest con el MISMO criterio de tools/e0b/analyze_prod.py (una muestra cuenta solo si el motor
  permaneció en ese estado todo el intervalo y llevaba ≥ 1 s en él). Por repetición se toma la MEDIA; se reporta la mediana de
  esas medias y el rango [mín–máx] entre repeticiones.
- `suppressed` / `hidden` / `paused`: estado fijo tras el guion (a los 20 s): media de las muestras posteriores a los 30 s.
- El contador de CPU de Windows avanza en ticks de ~15,6 ms: una muestra de 1 s toma valores discretos; por eso se usan medias.
"""
import csv
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "e0b"))
from analyze_prod import classify, states_from_log  # noqa: E402


def main(prefix: str, md: bool) -> int:
    rows = list(csv.DictReader(open(prefix + "-samples.csv", encoding="utf-8-sig")))
    runs = defaultdict(list)
    for r in rows:
        runs[(r["variant"], r["rep"])].append(r)
    per = defaultdict(lambda: defaultdict(list))  # (variante, estado) -> rep -> [cpu]
    for (v, rep), rs in runs.items():
        t_first = int(rs[0]["t_ms"]) - float(rs[0]["dt_ms"])
        if v in ("dnd_on", "dnd_off"):
            states = states_from_log(Path(f"{prefix}-logs/{v}-r{rep}.log"))
            for r in rs:
                t1 = int(r["t_ms"])
                t0 = t1 - float(r["dt_ms"])
                lab = classify(states, t0, t1, 1000)
                if lab in ("Idle", "Rest"):
                    per[(v, lab)][rep].append(float(r["cpu_pct"]))
        else:
            # el arranque tarda unos segundos: el guion actúa a los 20 s desde el inicio del proceso; las muestras empiezan
            # a los ~12 s, así que se descartan las 30 s primeras (relativas a la primera muestra: 12 s + 18 s)
            for r in rs:
                if (int(r["t_ms"]) - t_first) / 1000.0 >= 30 - 12:
                    per[(v, "fijo")][rep].append(float(r["cpu_pct"]))
    out = []
    for (v, lab), d in sorted(per.items()):
        means = {rep: st.mean(x) for rep, x in d.items()}
        vals = sorted(means.values())
        out.append((v, lab, len(means), st.median(vals), vals[0], vals[-1], sum(len(x) for x in d.values()), means))
    if md:
        print("| Variante | Estado | Repeticiones | Mediana de medias (% núcleo) | Rango | Muestras |")
        print("|---|---|---|---|---|---|")
        for v, lab, n, med, lo, hi, ns, _ in out:
            print(f"| `{v}` | {lab} | {n} | **{med:.3f}** | {lo:.3f}–{hi:.3f} | {ns} |")
    else:
        for v, lab, n, med, lo, hi, ns, means in out:
            print(f"{v:11} {lab:5} reps={n} mediana={med:.3f} rango={lo:.3f}-{hi:.3f} n={ns}  por rep: " + "  ".join(f"r{k}={m:.3f}" for k, m in sorted(means.items())))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], "--md" in sys.argv))
