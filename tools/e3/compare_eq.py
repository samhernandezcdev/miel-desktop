"""E4a: compara la secuencia de vistas y eventos de diag entre variantes (mismo guion, misma semilla).

Una "linea de evento" es una linea `[miel] ... vista v<N> ...` o `[miel] diag: ...`; se quita la marca de tiempo
`t=<ms>`. Se comparan: (1) cada par de ejecuciones de variantes distintas y (2) las de la misma variante (control:
cuanta variacion natural hay por el reloj de pared). Uso: python -I compare_eq.py <carpeta de registros> [nombre ...]
"""
import itertools
import re
import sys
from pathlib import Path

T = re.compile(r"\bt=\d+ ")


def events(path: Path) -> list[str]:
    out = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("[miel]") and ("vista v" in line or " diag: " in line or line.startswith("[miel] diag:")):
            out.append(T.sub("", line))
    return out


def main() -> None:
    d = Path(sys.argv[1])
    logs = sorted(d.glob("*.log"))
    seqs = {p.stem: events(p) for p in logs}
    for k, v in seqs.items():
        print(f"{k:8} eventos={len(v):3}  vistas={sum('vista v' in e for e in v):3}")
    print()
    same = diff = 0
    for a, b in itertools.combinations(seqs, 2):
        kind = "misma variante" if a.split("-")[0] == b.split("-")[0] else "distinta variante"
        eq = seqs[a] == seqs[b]
        same += eq
        diff += not eq
        msg = "IDENTICA" if eq else "DIFIERE"
        extra = ""
        if not eq:
            n = next((i for i, (x, y) in enumerate(zip(seqs[a], seqs[b])) if x != y), min(len(seqs[a]), len(seqs[b])))
            extra = f"  (primera diferencia en el evento {n}: {seqs[a][n:n+1]} vs {seqs[b][n:n+1]})"
        print(f"{a:8} vs {b:8} [{kind}] {msg}{extra}")
    print(f"\nidenticas={same} distintas={diff}")


main()
