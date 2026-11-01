"""Genera docs/THIRD_PARTY.md con las licencias de las dependencias de EJECUCIÓN.

Uso:  python -I tools/third_party.py
Rust: `cargo metadata` (solo dependencias normales del objetivo x86_64-pc-windows-msvc).
npm:  lee package.json de node_modules para las dependencias directas de ejecución.
No requiere red. Las licencias se leen de los metadatos de cada paquete: revisar antes de publicar.
"""
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def rust():
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc",
         "--manifest-path", str(ROOT / "src-tauri" / "Cargo.toml")],
        capture_output=True, text=True, encoding="utf-8", check=True).stdout
    meta = json.loads(out)
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    pkgs = {p["id"]: p for p in meta["packages"]}
    root = meta["resolve"]["root"]
    seen, stack = set(), [root]
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        for d in nodes[i]["deps"]:
            if any(k["kind"] is None for k in d["dep_kinds"]):  # solo dependencias normales (no dev/build)
                stack.append(d["pkg"])
    rows = sorted({(pkgs[i]["name"], pkgs[i]["version"], pkgs[i].get("license") or "(sin campo license)") for i in seen if i != root})
    return rows


def npm():
    pj = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
    rows = []
    for name in sorted(pj["dependencies"]):
        meta = json.loads((ROOT / "node_modules" / name / "package.json").read_text(encoding="utf-8"))
        rows.append((name, meta["version"], meta.get("license", "(sin campo license)")))
    return rows


def main():
    r, n = rust(), npm()
    lic = {}
    for _, _, l in r:
        lic[l] = lic.get(l, 0) + 1
    lines = ["# Licencias de terceros (dependencias de ejecución)", "",
             "Generado con `python -I tools/third_party.py` a partir de los metadatos de cada paquete.",
             "**No sustituye a una revisión legal** ni a incluir los textos de licencia/avisos que exija cada una",
             "al redistribuir el instalador (ver `docs/LICENSING.md` y `docs/THIRD_PARTY_NOTES.md`).", "",
             f"## Resumen Rust ({len(r)} crates)", ""]
    lines += [f"- `{l}`: {c}" for l, c in sorted(lic.items(), key=lambda x: -x[1])]
    lines += ["", "## npm (dependencias directas de ejecución, empaquetadas en el frontend)", "",
              "| Paquete | Versión | Licencia |", "|---|---|---|"]
    lines += [f"| {a} | {b} | {c} |" for a, b, c in n]
    lines += ["", f"## Rust — detalle ({len(r)} crates)", "", "| Crate | Versión | Licencia |", "|---|---|---|"]
    lines += [f"| {a} | {b} | {c} |" for a, b, c in r]
    (ROOT / "docs" / "THIRD_PARTY.md").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"docs/THIRD_PARTY.md: {len(r)} crates Rust, {len(n)} paquetes npm")


if __name__ == "__main__":
    main()
