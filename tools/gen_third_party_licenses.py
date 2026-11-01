#!/usr/bin/env python3
"""Genera THIRD_PARTY_LICENSES.txt: textos completos de licencia y avisos de copyright de lo que
se redistribuye en el instalador de Miel.

Qué cubre
  * Rust: las crates enlazadas en miel.exe para Windows x64 (sin dependencias de desarrollo ni de
    compilación), con `cargo-about` (versión fijada abajo; configuración en tools/about.toml y
    plantilla en tools/about.hbs).
  * npm: el cierre de dependencias de PRODUCCIÓN (`npm ls --omit=dev --all`), que es lo que Vite
    empaqueta en el frontend (react, react-dom, scheduler, @tauri-apps/api). Se leen los archivos
    LICENSE/NOTICE de cada paquete instalado en node_modules.
  * Crates MPL-2.0: enlace a su código fuente y comprobación de que NO están modificadas (el .crate
    descargado coincide con el checksum de Cargo.lock y el código extraído coincide con el .crate).

Modos
  python -I tools/gen_third_party_licenses.py            genera/actualiza THIRD_PARTY_LICENSES.txt
  python -I tools/gen_third_party_licenses.py --check    (CI) falla si Cargo.lock o package-lock.json
                                                         cambiaron desde la última generación
  python -I tools/gen_third_party_licenses.py --verify   regenera en memoria y compara byte a byte
                                                         con el archivo del repositorio

Requisitos para generar: Rust, `cargo install cargo-about --locked --version <ABOUT_VERSION>`,
`npm ci` hecho (node_modules). --check no necesita ninguno de los dos.
No toca Git, ni el identificador de la aplicación, ni la red salvo `cargo fetch` (descarga de crates).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "THIRD_PARTY_LICENSES.txt"
CARGO_LOCK = ROOT / "src-tauri" / "Cargo.lock"
CARGO_TOML = ROOT / "src-tauri" / "Cargo.toml"
NPM_LOCK = ROOT / "package-lock.json"
ABOUT_VERSION = "0.9.3"  # fijada para que la salida sea reproducible; al subirla, regenerar y revisar el diff
TARGET = "x86_64-pc-windows-msvc"
LICENSE_NAMES = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)([-_. ].*)?$", re.IGNORECASE)
HASH_RE = {
    "Cargo.lock": re.compile(r"^Cargo\.lock-sha256: ([0-9a-f]{64})$", re.MULTILINE),
    "package-lock.json": re.compile(r"^package-lock\.json-sha256: ([0-9a-f]{64})$", re.MULTILINE),
}
RULE = "=" * 80


def norm_hash(path: Path) -> str:
    """SHA-256 con saltos de línea normalizados (CRLF -> LF): estable entre Windows y Git."""
    return hashlib.sha256(path.read_bytes().replace(b"\r\n", b"\n")).hexdigest()


def current_hashes() -> dict[str, str]:
    return {"Cargo.lock": norm_hash(CARGO_LOCK), "package-lock.json": norm_hash(NPM_LOCK)}


def run(cmd: list[str], **kw) -> str:
    r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", cwd=ROOT, **kw)
    if r.returncode != 0:
        sys.stderr.write(r.stderr)
        raise SystemExit(f"falló: {' '.join(cmd)}")
    return r.stdout


def lf(text: str) -> str:
    return text.replace("\r\n", "\n").replace("\r", "\n")


# ---- --check -------------------------------------------------------------------------------------
def check() -> int:
    if not OUT.exists():
        print(f"FALTA {OUT.name}. Genera con: python -I tools/gen_third_party_licenses.py")
        return 1
    text = OUT.read_text(encoding="utf-8")
    bad = []
    for name, h in current_hashes().items():
        m = HASH_RE[name].search(text)
        if not m:
            bad.append(f"{name}: el archivo no registra su hash")
        elif m.group(1) != h:
            bad.append(f"{name}: cambió desde la última generación (registrado {m.group(1)[:12]}…, actual {h[:12]}…)")
    if bad:
        print("THIRD_PARTY_LICENSES.txt está DESACTUALIZADO:")
        for b in bad:
            print("  -", b)
        print("Regenéralo con:  python -I tools/gen_third_party_licenses.py   (y súbelo junto al cambio de dependencias)")
        return 1
    print("THIRD_PARTY_LICENSES.txt coincide con Cargo.lock y package-lock.json")
    return 0


# ---- Rust (cargo-about) --------------------------------------------------------------------------
def rust_licenses() -> str:
    ver = run(["cargo", "about", "--version"]).strip()
    if ver != f"cargo-about {ABOUT_VERSION}":
        raise SystemExit(f"se necesita cargo-about {ABOUT_VERSION} (hay '{ver}'): cargo install cargo-about --locked --version {ABOUT_VERSION}")
    run(["cargo", "fetch", "--locked", "--manifest-path", str(CARGO_TOML), "--target", TARGET])
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td) / "rust-licenses.txt"
        # cargo-about exige -o (no redirigir stdout) para evitar problemas de codificación en PowerShell
        run(["cargo", "about", "generate", "--offline", "--fail", "-o", str(tmp),
             "-m", str(CARGO_TOML), "-c", str(ROOT / "tools" / "about.toml"), str(ROOT / "tools" / "about.hbs")])
        out = tmp.read_text(encoding="utf-8")
    return lf(out).rstrip() + "\n"


def linked_crates() -> set[str]:
    """Crates realmente enlazadas en miel.exe para Windows x64 (`cargo tree`, sin macros de compilación)."""
    out = run(["cargo", "tree", "--manifest-path", str(CARGO_TOML), "--target", TARGET,
               "-e", "normal,no-proc-macro", "--prefix", "none", "--locked"])
    found = set()
    for line in out.splitlines():
        m = re.match(r"^(\S+) v(\S+)", line)
        if m and m.group(1) != "miel":
            found.add(f"{m.group(1)} {m.group(2)}")
    return found


def assert_rust_coverage(rust_text: str) -> None:
    covered = {f"{a} {b}" for a, b in re.findall(r"^  - (\S+) (\S+)", rust_text, re.M)}
    missing = sorted(linked_crates() - covered)
    if missing:
        raise SystemExit("crates enlazadas SIN aviso de licencia en la salida de cargo-about:\n  " + "\n  ".join(missing))


def cargo_metadata() -> dict:
    return json.loads(run(["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET,
                           "--manifest-path", str(CARGO_TOML)]))


def runtime_crates(meta: dict) -> list[dict]:
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
            if any(k["kind"] is None for k in d["dep_kinds"]):
                stack.append(d["pkg"])
    return sorted((pkgs[i] for i in seen if i != root), key=lambda p: (p["name"], p["version"]))


def lock_checksums() -> dict[tuple[str, str], str]:
    text = lf(CARGO_LOCK.read_text(encoding="utf-8"))
    out = {}
    for m in re.finditer(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"\n(?:source = "[^"]+"\n)?(?:checksum = "([0-9a-f]+)")?', text):
        if m.group(3):
            out[(m.group(1), m.group(2))] = m.group(3)
    return out


def verify_unmodified(pkg: dict, checksum: str) -> list[str]:
    """Devuelve la lista de problemas (vacía = sin modificar)."""
    name, ver = pkg["name"], pkg["version"]
    cache = Path.home() / ".cargo" / "registry" / "cache"
    crate_files = sorted(cache.glob(f"*/{name}-{ver}.crate"))
    if not crate_files:
        return [f"no se encontró {name}-{ver}.crate en la caché de Cargo"]
    data = crate_files[0].read_bytes()
    problems = []
    if hashlib.sha256(data).hexdigest() != checksum:
        problems.append(".crate NO coincide con el checksum de Cargo.lock")
    src = Path(pkg["manifest_path"]).parent
    import io
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tf:
        in_tar = set()
        for m in tf.getmembers():
            if not m.isfile():
                continue
            rel = m.name.split("/", 1)[1] if "/" in m.name else m.name
            in_tar.add(rel)
            disk = src / rel
            if not disk.is_file():
                problems.append(f"falta en disco: {rel}")
            elif disk.read_bytes() != tf.extractfile(m).read():
                problems.append(f"difiere del .crate: {rel}")
    on_disk = {p.relative_to(src).as_posix() for p in src.rglob("*") if p.is_file()} - {".cargo-ok"}
    for extra in sorted(on_disk - in_tar):
        problems.append(f"archivo extra en disco: {extra}")
    return problems


def mpl_section(meta: dict) -> str:
    crates = [p for p in runtime_crates(meta) if "MPL-2.0" in (p.get("license") or "")]
    sums = lock_checksums()
    lines = [RULE, "COMPONENTES BAJO MPL-2.0: CÓDIGO FUENTE / MPL-2.0 COMPONENTS: SOURCE CODE", RULE, "",
             "Estas crates se distribuyen dentro de miel.exe en forma ejecutable. Su código fuente está disponible",
             "en los enlaces siguientes (MPL-2.0, sección 3.2). Miel NO las modifica: se usan tal como se publican en",
             "crates.io (el .crate coincide con el checksum de Cargo.lock y el código extraído coincide con el .crate;",
             "no hay [patch], [replace] ni dependencias por ruta en src-tauri/Cargo.toml).",
             "These crates are shipped inside miel.exe in Executable Form. Their Source Code Form is available at the",
             "links below (MPL-2.0 section 3.2). Miel does NOT modify them.", ""]
    problems_total = []
    for p in crates:
        n, v = p["name"], p["version"]
        ck = sums.get((n, v), "")
        probs = verify_unmodified(p, ck) if ck else ["sin checksum en Cargo.lock"]
        problems_total += [f"{n} {v}: {x}" for x in probs]
        lines += [f"* {n} {v}  ({p['license']})",
                  f"    Código fuente (.crate): https://static.crates.io/crates/{n}/{n}-{v}.crate",
                  f"    Página en crates.io:    https://crates.io/crates/{n}/{v}",
                  f"    Repositorio:            {p.get('repository') or '(no declarado)'}",
                  f"    SHA-256 del .crate:     {ck}",
                  f"    Sin modificar:          {'SÍ (verificado al generar este archivo)' if not probs else 'NO: ' + '; '.join(probs)}", ""]
    if problems_total:
        raise SystemExit("crates MPL-2.0 con diferencias:\n  " + "\n  ".join(problems_total))
    if not crates:
        lines.append("(ninguna)")
    return "\n".join(lines).rstrip() + "\n"


# ---- npm ----------------------------------------------------------------------------------------
def npm_section() -> str:
    npm = "npm.cmd" if sys.platform == "win32" else "npm"
    tree = json.loads(run([npm, "ls", "--omit=dev", "--all", "--json"]))
    found: dict[str, str] = {}

    def walk(node: dict) -> None:
        for k, v in (node.get("dependencies") or {}).items():
            found[k] = v.get("version", "")
            walk(v)

    walk(tree)
    lines = [RULE, "PAQUETES npm EN EL FRONTEND DE PRODUCCIÓN / npm PACKAGES IN THE PRODUCTION FRONTEND", RULE, "",
             "Cierre de dependencias de producción (`npm ls --omit=dev --all`); es lo que Vite empaqueta en el frontend.",
             "Las dependencias de desarrollo (TypeScript, Vite, Vitest, etc.) no se redistribuyen.", ""]
    for name in sorted(found):
        d = ROOT / "node_modules" / name
        pj = json.loads((d / "package.json").read_text(encoding="utf-8"))
        if pj["version"] != found[name]:
            raise SystemExit(f"{name}: node_modules tiene {pj['version']} y el árbol dice {found[name]}; ejecuta npm ci")
        lic = pj.get("license") or (pj.get("licenses") or [{}])[0].get("type") or "(sin campo license)"
        repo = pj.get("repository")
        repo = repo.get("url") if isinstance(repo, dict) else repo
        files = sorted(f for f in d.iterdir() if f.is_file() and LICENSE_NAMES.match(f.name))
        lines += [RULE, f"{name} {pj['version']}  [{lic}]", f"Repositorio: {repo or '(no declarado)'}", ""]
        if not files:
            raise SystemExit(f"{name}: no se encontró ningún archivo de licencia en el paquete")
        for f in files:
            lines += [f"--- {f.name} " + "-" * max(3, 70 - len(f.name)), lf(f.read_text(encoding="utf-8", errors="replace")).strip(), ""]
    return "\n".join(lines).rstrip() + "\n"


# ---- ensamblado ---------------------------------------------------------------------------------
HEADER = """Miel Desktop: avisos y licencias de software de terceros
Miel Desktop: third-party software notices and licenses

Este archivo se distribuye con el instalador de Miel y reproduce los avisos de copyright y los textos de
licencia exigidos por los componentes de terceros que Miel redistribuye. No incluye la licencia del propio
Miel (ver el repositorio del proyecto). / This file ships with the Miel installer and reproduces the
copyright notices and license texts required by the third-party components Miel redistributes. It does not
include Miel's own license (see the project repository).

NO EDITAR A MANO: lo genera tools/gen_third_party_licenses.py (cargo-about {about}).
DO NOT EDIT BY HAND: generated by tools/gen_third_party_licenses.py.

Generado a partir de / Generated from (SHA-256, saltos de línea normalizados a LF):
Cargo.lock-sha256: {cargo}
package-lock.json-sha256: {npm}

Contenido / Contents:
  1. Componentes MPL-2.0: código fuente / MPL-2.0 components: source code
  2. Paquetes npm del frontend de producción / npm packages in the production frontend
  3. Crates de Rust enlazadas en miel.exe (Windows x64) / Rust crates linked into miel.exe
  4. Otros componentes / Other components

"""

OTHER = """
================================================================================
OTROS COMPONENTES / OTHER COMPONENTS
================================================================================

* Microsoft Edge WebView2 Runtime: NO se incluye en el instalador. Lo aporta Windows o el bootstrapper de
  Microsoft, bajo sus propios términos. / NOT included in the installer; provided by Windows or Microsoft's
  bootstrapper under Microsoft's own terms.
* NSIS (Nullsoft Scriptable Install System): el instalador se genera con NSIS y contiene componentes suyos.
  Licencia: https://nsis.sourceforge.io/License (zlib/libpng; los módulos de compresión tienen licencias
  propias indicadas allí). / The installer is built with NSIS and contains NSIS components.
* Fuentes tipográficas: no se incluyen; la burbuja usa fuentes del sistema. / No fonts are bundled.
"""


def build() -> str:
    meta = cargo_metadata()
    h = current_hashes()
    rust = rust_licenses()
    assert_rust_coverage(rust)
    parts = [
        HEADER.format(about=ABOUT_VERSION, cargo=h["Cargo.lock"], npm=h["package-lock.json"]),
        mpl_section(meta),
        "\n",
        npm_section(),
        "\n" + RULE + "\nCRATES DE RUST / RUST CRATES (cargo-about)\n" + RULE + "\n\n",
        "Crates compiladas para Windows x64 y enlazadas en miel.exe, más las macros procedurales que se ejecutan\n"
        "durante la compilación (se incluyen por prudencia aunque no formen parte del binario). Sin dependencias de\n"
        "desarrollo ni de compilación (build scripts). / Crates compiled for Windows x64 and linked into miel.exe,\n"
        "plus procedural macros that run at build time (included as a precaution).\n\n",
        rust,
        OTHER,
    ]
    return lf("".join(parts))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--check", action="store_true", help="solo comprueba que los hashes de los lockfiles coinciden")
    g.add_argument("--verify", action="store_true", help="regenera en memoria y compara con el archivo")
    a = ap.parse_args()
    for stream in (sys.stdout, sys.stderr):  # consolas de Windows con otra codificación (CI)
        stream.reconfigure(encoding="utf-8", errors="replace")
    if a.check:
        return check()
    text = build()
    if a.verify:
        same = OUT.exists() and OUT.read_bytes() == text.encode("utf-8")
        print("regenerado == archivo del repositorio" if same else "DIFIERE del archivo del repositorio")
        return 0 if same else 1
    OUT.write_bytes(text.encode("utf-8"))
    print(f"escrito {OUT.name}: {len(text):,} caracteres")
    return 0


if __name__ == "__main__":
    sys.exit(main())
