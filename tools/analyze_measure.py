"""Agrega las mediciones de tools/measure.ps1 por estado de Miel.

Uso:  python -I tools/analyze_measure.py <prefijo>        (lee <prefijo>-samples.csv y <prefijo>-engine.log)

El registro del motor (build con --features diag o de depuración) contiene líneas
  [miel] t=<unix_ms> vista vN <Behavior>/<Expression> paused=<b> hidden=<b> follow=<b> msg=<None|Some(..)>
que delimitan los estados; cada muestra se asigna al estado vigente en su instante.
Sin registro, todo se agrupa en 'sin-clasificar'.
"""
import csv
import re
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path

LINE = re.compile(r"t=(\d+) vista v\d+ (\w+)/(\w+) paused=(\w+) hidden=(\w+) follow=(\w+) msg=(\S+)")


def load_states(log: Path):
    states = []
    if not log.exists():
        return states
    for raw in log.read_text(encoding="utf-8", errors="replace").splitlines():
        m = LINE.search(raw)
        if m:
            t, beh, expr, paused, hidden, _follow, msg = m.groups()
            if hidden == "true":
                label = "Hidden" + ("+Paused" if paused == "true" else "")
            elif paused == "true":
                label = "Paused"
            else:
                label = beh + ("+mensaje" if msg.startswith("Some") else "")
            states.append((int(t), label))
    return states


def label_at(states, t):
    cur = "Idle (inicio)"
    for ts, lab in states:
        if ts <= t:
            cur = lab
        else:
            break
    return cur


def summarize(vals):
    vals = [v for v in vals if v is not None]
    if not vals:
        return "—"
    return f"media {st.mean(vals):.2f} · mediana {st.median(vals):.2f} · máx {max(vals):.2f}"


def main(prefix: str):
    samples = list(csv.DictReader(open(f"{prefix}-samples.csv", encoding="utf-8-sig")))
    states = load_states(Path(f"{prefix}-engine.log"))
    groups = defaultdict(list)
    for r in samples:
        t = int(r["t_ms"])
        groups[label_at(states, t) if states else "sin-clasificar"].append(r)

    def f(r, k):
        v = r[k]
        return float(v) if v not in ("", None) else None

    print(f"Muestras: {len(samples)}  Estados registrados: {len(states)}")
    print(f"{'estado':<22}{'n':>4}  {'CPU % de 1 núcleo (Miel+WebView2)':<52}{'Private Bytes (MB, suma)':<40}{'Working Set (MB, suma*)':<40}procs")
    for lab, rows in sorted(groups.items()):
        cpu = summarize([f(r, "cpu_pct_of_one_core") for r in rows])
        priv = summarize([f(r, "private_sum_mb") for r in rows])
        ws = summarize([f(r, "ws_sum_mb") for r in rows])
        procs = sorted({int(r["processes"]) for r in rows})
        print(f"{lab:<22}{len(rows):>4}  {cpu:<52}{priv:<40}{ws:<40}{procs}")
    # Desglose por rol (miel.exe frente a la suma de sus WebView2) por estado
    procs_path = Path(f"{prefix}-procs.csv")
    if procs_path.exists():
        by_t = defaultdict(list)
        for r in csv.DictReader(open(procs_path, encoding="utf-8-sig")):
            by_t[int(r["t_ms"])].append(r)
        print("")
        print("Desglose por rol (media por muestra):")
        print(f"{'estado':<22}{'miel.exe CPU%':>14}{'WebView2 CPU%':>15}{'miel.exe Priv MB':>18}{'WebView2 Priv MB':>18}")
        for lab, rows in sorted(groups.items()):
            cm, cw, pm, pw = [], [], [], []
            for r in rows:
                ps = by_t.get(int(r["t_ms"]), [])
                m = [x for x in ps if x["role"] == "miel"]
                w = [x for x in ps if x["role"] == "webview2"]
                if m and m[0]["cpu_pct_core"] != "":
                    cm.append(float(m[0]["cpu_pct_core"]))
                cw.append(sum(float(x["cpu_pct_core"]) for x in w if x["cpu_pct_core"] != ""))
                pm.append(sum(float(x["private_mb"]) for x in m))
                pw.append(sum(float(x["private_mb"]) for x in w))
            mean = lambda v: f"{st.mean(v):.2f}" if v else "-"
            print(f"{lab:<22}{mean(cm):>14}{mean(cw):>15}{mean(pm):>18}{mean(pw):>18}")
    first, last = samples[0], samples[-1]
    span = (int(last["t_ms"]) - int(first["t_ms"])) / 60000
    print("")
    print(f"Deriva en {span:.1f} min: Private {first['private_sum_mb']} -> {last['private_sum_mb']} MB; "
          f"WS {first['ws_sum_mb']} -> {last['ws_sum_mb']} MB; procesos {first['processes']} -> {last['processes']}")
    print("* La suma de Working Sets cuenta varias veces las paginas compartidas entre procesos WebView2.")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
