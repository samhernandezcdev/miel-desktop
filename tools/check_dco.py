#!/usr/bin/env python3
"""
Miel: comprobación del DCO (Developer Certificate of Origin 1.1; ver DCO.md y CONTRIBUTING.md).

Cada commit de un pull request (sin contar los de fusión) debe llevar una línea
    Signed-off-by: Nombre <correo>
cuyo correo coincida con el del autor o el de quien confirma el commit. Sin acciones de terceros:
solo lee el historial con `git log` (no escribe nada).

Uso:
  python -I tools/check_dco.py --base <sha> --head <sha>    (en la CI: base y cabeza del PR)
  python -I tools/check_dco.py --self-test                  casos con y sin firma (deben comportarse como se espera)
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SIGNOFF = re.compile(r"^Signed-off-by:[ \t]+(\S.*?)[ \t]+<([^<>\s@]+@[^<>\s@]+)>[ \t]*$", re.MULTILINE)
SHA = re.compile(r"^[0-9a-fA-F]{7,64}$")
FS, RS = "\x1f", "\x1e"


def check_message(message: str, author_email: str, committer_email: str) -> str | None:
    """None si la firma es válida; si no, el motivo del rechazo."""
    signs = SIGNOFF.findall(message)
    if not signs:
        return "falta la línea 'Signed-off-by: Nombre <correo>' (usa `git commit -s`)"
    allowed = {author_email.strip().lower(), committer_email.strip().lower()}
    if not any(email.lower() in allowed for _, email in signs):
        return (f"ningún Signed-off-by coincide con el correo del autor ({author_email}) ni del committer "
                f"({committer_email}); firmas: {[e for _, e in signs]}")
    return None


def parse_log(raw: str) -> list[tuple[str, str, str, str]]:
    commits = []
    for rec in raw.split(RS):
        rec = rec.strip("\n")
        if not rec.strip():
            continue
        sha, ae, ce, body = (rec.split(FS, 3) + ["", "", "", ""])[:4]
        commits.append((sha.strip(), ae, ce, body))
    return commits


def check_range(base: str, head: str) -> list[str]:
    for ref in (base, head):
        if not SHA.match(ref):
            return [f"referencia no válida: {ref!r} (se esperaba un SHA)"]
    r = subprocess.run(
        ["git", "log", "--no-merges", f"--format=%H{FS}%ae{FS}%ce{FS}%B{RS}", f"{base}..{head}"],
        capture_output=True, text=True, encoding="utf-8", cwd=ROOT)
    if r.returncode != 0:
        return [f"git log falló: {r.stderr.strip()[-300:]}"]
    problems = []
    for sha, ae, ce, body in parse_log(r.stdout):
        why = check_message(body, ae, ce)
        if why:
            problems.append(f"{sha[:12]}: {why}")
    return problems


def self_test() -> int:
    good = "Arregla el cálculo\n\nPorque sí.\n\nSigned-off-by: Ana Pérez <ana@example.org>\n"
    cases = [
        ("con firma válida", good, "ana@example.org", "ana@example.org", True),
        ("sin firma", "Arregla el cálculo\n\nPorque sí.\n", "ana@example.org", "ana@example.org", False),
        ("firma sin correo", "x\n\nSigned-off-by: Ana Pérez\n", "ana@example.org", "ana@example.org", False),
        ("firma con formato roto", "x\n\nSigned-off-by Ana <ana@example.org>\n", "ana@example.org", "ana@example.org", False),
        ("firma de otra persona", "x\n\nSigned-off-by: Bob <bob@example.org>\n", "ana@example.org", "ana@example.org", False),
        ("mayúsculas en el correo", "x\n\nSigned-off-by: Ana <ANA@Example.org>\n", "ana@example.org", "ana@example.org", True),
        ("coincide con el committer", "x\n\nSigned-off-by: Bob <bob@example.org>\n", "ana@example.org", "bob@example.org", True),
        ("varias firmas, una válida", "x\n\nSigned-off-by: Bob <bob@example.org>\nSigned-off-by: Ana <ana@example.org>\n", "ana@example.org", "ana@example.org", True),
        ("firma solo en el asunto cuenta como línea", "Signed-off-by: Ana <ana@example.org>\n", "ana@example.org", "ana@example.org", True),
        ("texto que menciona la firma sin ser línea", "x\n\nOjo: olvidé Signed-off-by: Ana <ana@example.org> aquí\n", "ana@example.org", "ana@example.org", False),
        ("firma con noreply de GitHub", "x\n\nSigned-off-by: Ana <123+ana@users.noreply.github.com>\n", "123+ana@users.noreply.github.com", "noreply@github.com", True),
    ]
    failures = []
    for label, msg, ae, ce, should_pass in cases:
        ok = check_message(msg, ae, ce) is None
        if ok != should_pass:
            failures.append(f"{label}: se esperaba {'PASO' if should_pass else 'FALLO'}")
    raw = (f"aaaaaaa{FS}ana@example.org{FS}ana@example.org{FS}{good}{RS}\n"
           f"bbbbbbb{FS}ana@example.org{FS}ana@example.org{FS}sin firma\n{RS}\n")
    parsed = parse_log(raw)
    if [c[0] for c in parsed] != ["aaaaaaa", "bbbbbbb"]:
        failures.append(f"parse_log devolvió {[c[0] for c in parsed]}")
    else:
        flagged = [c[0] for c in parsed if check_message(c[3], c[1], c[2])]
        if flagged != ["bbbbbbb"]:
            failures.append(f"el análisis de un historial simulado marcó {flagged}")
    if check_range("--output=x", "HEAD") == []:
        failures.append("una referencia que no es un SHA no fue rechazada")
    if failures:
        print("AUTOTEST DCO FALLÓ:")
        for f in failures:
            print("  -", f)
        return 1
    print(f"AUTOTEST DCO OK ({len(cases)} casos con y sin firma + historial simulado).")
    return 0


def main() -> int:
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8", errors="replace")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base")
    ap.add_argument("--head")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    if not a.base or not a.head:
        ap.error("se necesitan --base y --head (o --self-test)")
    problems = check_range(a.base, a.head)
    if problems:
        print("DCO: faltan firmas válidas (Signed-off-by) en:")
        for p in problems:
            print("  -", p)
        print("Arréglalo con `git commit --amend -s` o `git rebase --signoff` y vuelve a subir. Ver DCO.md.")
        return 1
    print("DCO: OK (todos los commits del rango llevan Signed-off-by válido).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
