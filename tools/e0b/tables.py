import csv, statistics as st, collections, sys
M = r'C:\dev\miel-desktop\docs\measurements\v020-e0b-'

def load(name):
    rows = list(csv.DictReader(open(M + name + '-samples.csv', encoding='utf-8-sig')))
    d = collections.defaultdict(list)
    for r in rows:
        d[(r['cfg'], r['state'])].append(r)
    return d

def stat(d, cfg, state, f='cpu_total'):
    v = [float(r[f]) for r in d[(cfg, state)]]
    return st.median(v), min(v), max(v), len(v)

def line(d, cfg, ctrl='C0-base'):
    out = [f"`{cfg}`"]
    for s in ('idle', 'rest'):
        m, lo, hi, n = stat(d, cfg, s)
        m0 = stat(d, ctrl, s)[0]
        out.append(f"{m:.2f} [{lo:.2f}–{hi:.2f}]")
        out.append(f"{(m - m0):+.2f} ({(m - m0) / m0 * 100:+.0f} %)")
    g = [float(r['gpu_pct']) for s in ('idle',) for r in d[(cfg, s)]]
    out.append(f"{st.median(g):.2f}")
    mb = [float(r['private_mb']) for r in d[(cfg, 'idle')]]
    out.append(f"{st.median(mb):.0f}")
    return '| ' + ' | '.join(out) + ' |'

hdr = "| Variante | Idle: mediana [rango] | Δ vs base | Rest: mediana [rango] | Δ vs base | GPU % (Idle) | Private MB |\n|---|---|---|---|---|---|---|"
for name, cfgs in (('main', None), ('r2', None), ('r3', None)):
    d = load(name)
    order = []
    for (c, s) in d:
        if c not in order:
            order.append(c)
    print(f"\n### {name}\n{hdr}")
    for c in order:
        print(line(d, c))

# atribución por proceso (C0 y C1) en main
d = load('main')
print("\n### reparto por rol (medianas, % de un núcleo)")
print("| Variante | Estado | miel.exe | WebView2 browser | renderer | gpu-process | utility | otros | total |\n|---|---|---|---|---|---|---|---|---|")
for c in ('C0-base', 'C1-todo-apagado'):
    for s in ('idle', 'rest'):
        f = lambda k: st.median(float(r[k]) for r in d[(c, s)])
        print(f"| `{c}` | {s} | {f('cpu_miel'):.2f} | {f('cpu_wv2_browser'):.2f} | {f('cpu_renderer'):.2f} | {f('cpu_gpu_process'):.2f} | {f('cpu_utility'):.2f} | {f('cpu_other'):.2f} | {f('cpu_total'):.2f} |")
# C0 agrupado de las tres rondas
for s in ('idle', 'rest'):
    v = []
    for n in ('main', 'r2', 'r3'):
        v += [float(r['cpu_total']) for r in load(n)[('C0-base', s)]]
    print(f"C0 agrupado {s}: n={len(v)} mediana {st.median(v):.2f} rango {min(v):.2f}–{max(v):.2f}")
