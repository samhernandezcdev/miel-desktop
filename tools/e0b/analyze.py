import csv, statistics as st, sys, collections
P = sys.argv[1]
rows = list(csv.DictReader(open(P + '-samples.csv', encoding='utf-8-sig')))
by = collections.defaultdict(list)
for r in rows:
    by[(r['cfg'], r['state'])].append(r)

def med(k, f):
    v = [float(r[f]) for r in by[k]]
    return st.median(v), min(v), max(v), len(v)

cfgs = []
for r in rows:
    if r['cfg'] not in cfgs:
        cfgs.append(r['cfg'])
print(f"{'variante':38} {'estado':5} {'n':>2} {'CPU tot med [min-max]':>24} {'renderer':>8} {'gpu-proc':>8} {'GPU%':>6} {'MB':>6}")
for c in cfgs:
    for s in ('idle', 'rest'):
        if (c, s) not in by:
            continue
        m, lo, hi, n = med((c, s), 'cpu_total')
        rr = med((c, s), 'cpu_renderer')[0]
        gp = med((c, s), 'cpu_gpu_process')[0] if 'cpu_gpu_process' in rows[0] else 0
        g = med((c, s), 'gpu_pct')[0]
        mb = med((c, s), 'private_mb')[0]
        print(f"{c:38} {s:5} {n:>2} {m:8.2f} [{lo:5.2f}-{hi:5.2f}] {rr:8.2f} {gp:8.2f} {g:6.2f} {mb:6.1f}")
