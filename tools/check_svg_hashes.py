#!/usr/bin/env python3
"""Comprueba que los SVG de la mascota no cambian: SHA-256 por archivo contra una línea base versionada.

    python -I tools/check_svg_hashes.py              # compara con tools/svg_hashes.json (sale con 1 si hay diferencias)
    python -I tools/check_svg_hashes.py --self-test  # prueba negativa: debe fallar ante cambios, faltantes y sobrantes
    python -I tools/check_svg_hashes.py --update     # reescribe la línea base (SOLO cuando se aprueba un cambio de arte)

- Los archivos son todos los `*.svg` versionados (o, sin Git, los que haya bajo el repositorio salvo `node_modules`,
  `target*`, `dist`, `.git`).
- El hash se calcula sobre el contenido con los saltos de línea normalizados (CRLF → LF) para que no dependa de
  `core.autocrlf` en cada máquina. Cualquier otro cambio de bytes cambia el hash.
- Una línea base distinta de la del repositorio nunca se escribe sin `--update`.
"""
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINE = Path(__file__).resolve().parent / "svg_hashes.json"
SKIP = {"node_modules", ".git", "dist", "tools-cache"}


def sha256_normalized(path: Path) -> str:
    data = path.read_bytes().replace(b"\r\n", b"\n")
    return hashlib.sha256(data).hexdigest()


def list_svgs(root: Path) -> list[str]:
    """Rutas relativas (con `/`) de los SVG del proyecto, ordenadas."""
    try:
        out = subprocess.run(
            ["git", "ls-files", "*.svg"], cwd=root, capture_output=True, text=True, check=True, timeout=30
        ).stdout.split()
        found = [p for p in out if (root / p).is_file()]
        if found:
            return sorted(found)
    except (OSError, subprocess.SubprocessError):
        pass
    found = []
    for p in root.rglob("*.svg"):
        rel = p.relative_to(root)
        if any(part in SKIP or part.startswith("target") for part in rel.parts):
            continue
        found.append(rel.as_posix())
    return sorted(found)


def current(root: Path, files: list[str] | None = None) -> dict[str, str]:
    return {rel: sha256_normalized(root / rel) for rel in (files if files is not None else list_svgs(root))}


def compare(baseline: dict[str, str], now: dict[str, str]) -> list[str]:
    problems = []
    for rel in sorted(baseline.keys() - now.keys()):
        problems.append(f"FALTA   {rel}")
    for rel in sorted(now.keys() - baseline.keys()):
        problems.append(f"SOBRA   {rel} (no está en la línea base)")
    for rel in sorted(baseline.keys() & now.keys()):
        if baseline[rel] != now[rel]:
            problems.append(f"CAMBIÓ  {rel}: {baseline[rel][:16]}… → {now[rel][:16]}…")
    return problems


def load_baseline(path: Path = BASELINE) -> dict[str, str]:
    return json.loads(path.read_text(encoding="utf-8"))["sha256"]


def self_test() -> int:
    """Copia los SVG a una carpeta temporal y comprueba que cada tipo de cambio se detecta."""
    base = load_baseline()
    fails = []
    with tempfile.TemporaryDirectory() as tmp:
        t = Path(tmp)
        for rel in base:
            dst = t / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes((ROOT / rel).read_bytes())
        names = sorted(base)

        def check(label: str, expect_fail: bool) -> None:
            probs = compare(base, current(t, sorted(p.relative_to(t).as_posix() for p in t.rglob("*.svg"))))
            if bool(probs) != expect_fail:
                fails.append(f"{label}: esperaba {'fallo' if expect_fail else 'OK'}, obtuvo {probs}")

        check("copia idéntica", False)
        # CRLF en lugar de LF: no debe contar como cambio
        p0 = t / names[0]
        original = p0.read_bytes()
        p0.write_bytes(original.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
        check("saltos de línea CRLF", False)
        p0.write_bytes(original)
        # un byte distinto
        p0.write_bytes(original + b" ")
        check("un espacio añadido", True)
        p0.write_bytes(original.replace(b"<", b"<!-- x -->\n<", 1))
        check("comentario insertado", True)
        p0.write_bytes(original)
        # faltante
        p1 = t / names[1]
        saved = p1.read_bytes()
        p1.unlink()
        check("archivo eliminado", True)
        p1.write_bytes(saved)
        # sobrante
        extra = t / "src" / "assets" / "bee" / "nuevo.svg"
        extra.parent.mkdir(parents=True, exist_ok=True)
        extra.write_bytes(b"<svg/>")
        check("archivo nuevo", True)
        extra.unlink()
        check("restaurado", False)
    if fails:
        print("AUTOTEST FALLÓ:\n  " + "\n  ".join(fails))
        return 1
    print(f"autotest OK: detecta cambio, comentario, faltante y sobrante; ignora CRLF ({len(base)} archivos)")
    return 0


def main(argv: list[str]) -> int:
    if "--self-test" in argv:
        return self_test()
    now = current(ROOT)
    if "--update" in argv:
        BASELINE.write_text(
            json.dumps(
                {
                    "nota": "SHA-256 por archivo de los SVG (saltos de línea normalizados a LF). Se actualiza SOLO al "
                    "aprobar un cambio de arte: python -I tools/check_svg_hashes.py --update",
                    "sha256": now,
                },
                indent=2,
                ensure_ascii=False,
            )
            + "\n",
            encoding="utf-8",
        )
        print(f"línea base escrita: {len(now)} archivos")
        return 0
    problems = compare(load_baseline(), now)
    if problems:
        print("SVG distintos de la línea base:\n  " + "\n  ".join(problems))
        return 1
    print(f"OK: {len(now)} SVG idénticos a la línea base")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
