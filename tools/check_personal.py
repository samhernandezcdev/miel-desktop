#!/usr/bin/env python3
"""Busca datos personales en lo que está a punto de versionarse.

    python -I tools/check_personal.py                    # cambios en staging (líneas añadidas) + archivos sin seguimiento
    python -I tools/check_personal.py --include-unstaged # además, los cambios del árbol de trabajo (git diff HEAD)
    python -I tools/check_personal.py --self-test        # prueba negativa con una identidad inventada

Qué busca (en las líneas AÑADIDAS de `git diff --cached` y en el contenido completo de los archivos sin seguimiento
que Git no ignora):
  1. El nombre de usuario y el nombre de equipo DE ESTA MÁQUINA, leídos del entorno en ejecución (nunca están escritos en
     el repositorio) y el nombre de la carpeta de perfil.
  2. Rutas `C:\\Users\\<nombre>` / `C:/Users/<nombre>` (cualquier nombre salvo los marcadores `%…%`, `<…>`, `Default`, `Public`…).
  3. Direcciones de correo.
  4. IPv4 que no sea de bucle local (127.0.0.0/8) ni `0.0.0.0`.
  5. IPv6 locales (enlace local fe80::/10 y únicas locales fc00::/7).
Las coincidencias legítimas (por ejemplo, un correo público de contacto) se declaran en `tools/personal_allowlist.json`
como cadenas exactas con su motivo. Las coincidencias se muestran enmascaradas (no se vuelve a imprimir el dato).
Código de salida: 0 limpio · 1 hay coincidencias.
"""
from __future__ import annotations

import getpass
import ipaddress
import json
import os
import platform
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ALLOWLIST = Path(__file__).resolve().parent / "personal_allowlist.json"
MAX_BYTES = 5 * 1024 * 1024

EMAIL = re.compile(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(?:\.[A-Za-z0-9\-]+)*\.[A-Za-z]{2,}")
IPV4 = re.compile(r"(?<![\w.])(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)(?![\w.])")
IPV6 = re.compile(r"(?<![\w:])(?:[0-9A-Fa-f]{0,4}:){2,7}[0-9A-Fa-f]{0,4}(?![\w:])")
USER_PATH = re.compile(r"(?i)[A-Za-z]:(?:\\\\|\\|/)Users(?:\\\\|\\|/)([^\\/\s\"'`<>|*?:%]+)")
RESERVED_DOMAINS = (".invalid", ".example", ".test", ".localhost")
RESERVED_EXACT = {"example.com", "example.org", "example.net"}
# Carpetas de sistema de C:\Users que no son una persona.
SYSTEM_USER_DIRS = {"default", "public", "all users", "default user", "defaultuser0", "<usuario>", "<user>", "username", "usuario", "user"}


@dataclass(frozen=True)
class Identity:
    """Valores de la máquina actual. Solo existen en memoria durante la ejecución."""

    names: tuple[str, ...]

    @staticmethod
    def from_environment() -> "Identity":
        raw = [
            os.environ.get("USERNAME") or os.environ.get("USER") or _safe(getpass.getuser),
            os.environ.get("COMPUTERNAME") or platform.node(),
            Path.home().name,
        ]
        names = {n.strip() for n in raw if n and len(n.strip()) >= 3}
        return Identity(tuple(sorted(names, key=str.lower)))


def _safe(fn):  # noqa: ANN001
    try:
        return fn()
    except Exception:  # noqa: BLE001
        return ""


def load_allowlist(path: Path = ALLOWLIST) -> set[str]:
    if not path.exists():
        return set()
    data = json.loads(path.read_text(encoding="utf-8"))
    return {item["valor"] for item in data.get("permitidos", [])}


def mask(s: str) -> str:
    return (s[:2] + "…" + f"({len(s)} car.)") if len(s) > 3 else "…"


@dataclass(frozen=True)
class Finding:
    where: str
    line: int
    kind: str
    sample: str

    def __str__(self) -> str:
        return f"{self.where}:{self.line}: {self.kind}: {mask(self.sample)}"


def scan_text(text: str, where: str, ident: Identity, allow: set[str], first_line: int = 1) -> list[Finding]:
    """Escanea texto. `where` es solo la etiqueta (archivo o diff)."""
    out: list[Finding] = []
    names_re = [re.compile(r"(?i)(?<![A-Za-z0-9])" + re.escape(n) + r"(?![A-Za-z0-9])") for n in ident.names]
    for i, line in enumerate(text.splitlines(), first_line):
        def add(kind: str, sample: str) -> None:
            if sample not in allow:
                out.append(Finding(where, i, kind, sample))

        for n, rx in zip(ident.names, names_re):
            for m in rx.finditer(line):
                add("nombre de usuario/equipo de esta máquina", m.group(0))
        for m in USER_PATH.finditer(line):
            who = m.group(1)
            if who.lower() in SYSTEM_USER_DIRS or (who.startswith("%") or who.startswith("<")):
                continue
            add("ruta C:\\Users\\<nombre>", m.group(0))
        for m in EMAIL.finditer(line):
            domain = m.group(0).rsplit("@", 1)[1].lower()
            if domain.endswith(RESERVED_DOMAINS) or domain in RESERVED_EXACT:
                continue  # dominios reservados para ejemplos (RFC 2606 / 6761): no son personas
            add("correo", m.group(0))
        for m in IPV4.finditer(line):
            try:
                ip = ipaddress.IPv4Address(m.group(0))
            except ValueError:
                continue
            if ip.is_loopback or ip == ipaddress.IPv4Address("0.0.0.0"):
                continue
            add("IPv4 no loopback", m.group(0))
        for m in IPV6.finditer(line):
            cand = m.group(0)
            try:
                ip = ipaddress.IPv6Address(cand)
            except ValueError:
                continue
            if ip.is_loopback or ip.is_unspecified or cand.endswith("::"):
                continue  # `fe80::` / `fc00::` sueltos son prefijos de documentación, no una dirección
            if ip.is_link_local or (int(ip) >> 121) == 0b1111110:  # fe80::/10 y fc00::/7
                add("IPv6 local", cand)
    return out


def _git(args: list[str], cwd: Path) -> str:
    r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120)
    if r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout


def added_lines(diff: str) -> list[tuple[str, int, str]]:
    """(archivo, nº de línea nuevo, texto) de las líneas añadidas de un diff unificado."""
    res: list[tuple[str, int, str]] = []
    cur, ln = "", 0
    for raw in diff.splitlines():
        if raw.startswith("+++ "):
            cur = raw[4:].strip()
            cur = cur[2:] if cur.startswith("b/") else cur
        elif raw.startswith("@@"):
            m = re.search(r"\+(\d+)", raw)
            ln = int(m.group(1)) if m else 0
        elif raw.startswith("+") and not raw.startswith("+++"):
            res.append((cur, ln, raw[1:]))
            ln += 1
        elif raw.startswith(" "):
            ln += 1
    return res


def scan_repo(repo: Path, ident: Identity, allow: set[str], include_unstaged: bool = False) -> list[Finding]:
    findings: list[Finding] = []
    base = "HEAD" if (include_unstaged and _has_head(repo)) else "--cached"
    diff = _git(["diff", base, "--unified=0", "--no-color", "--no-ext-diff"], repo)
    by_file: dict[str, list[tuple[int, str]]] = {}
    for f, ln, text in added_lines(diff):
        by_file.setdefault(f, []).append((ln, text))
    for f, rows in by_file.items():
        for ln, text in rows:
            findings += scan_text(text, f, ident, allow, ln)
    untracked = [p for p in _git(["ls-files", "--others", "--exclude-standard", "-z"], repo).split("\0") if p]
    for rel in untracked:
        p = repo / rel
        try:
            if not p.is_file() or p.stat().st_size > MAX_BYTES:
                continue
            data = p.read_bytes()
        except OSError:
            continue
        if b"\0" in data[:4096]:
            continue  # binario
        findings += scan_text(data.decode("utf-8", errors="replace"), rel, ident, allow)
    return findings


def _has_head(repo: Path) -> bool:
    return subprocess.run(["git", "rev-parse", "--verify", "-q", "HEAD"], cwd=repo, capture_output=True).returncode == 0


def self_test() -> int:
    """Repositorio temporal con una identidad INVENTADA: cada tipo de dato debe detectarse; el texto limpio, no."""
    ident = Identity(("zq-persona-prueba", "ZQ-EQUIPO-PRUEBA"))
    fails: list[str] = []
    # Se construyen en tiempo de ejecución para que este archivo no contenga datos que parezcan reales.
    cases = {
        "usuario": f"propietario: {ident.names[0]}",
        "equipo": f"host={ident.names[1].lower()}",
        "ruta": "log en " + "C:" + "\\" + "Users" + "\\" + "alguien" + "\\" + "AppData",
        "ruta JSON": "C:" + "\\\\" + "Users" + "\\\\" + "alguien" + "\\\\" + "AppData",
        "correo": "contacto: " + "nombre" + "@" + "ejemplo.org",
        "IPv4": "servidor 192.168." + "1.20",
        "IPv6 enlace local": "addr fe80::" + "1ff:fe23:4567:890a",
        "IPv6 única local": "addr fd12:3456:" + "789a:1::1",
    }
    clean = {
        "marcadores": "C:" + "\\Users\\" + "%USERNAME%\\x y C:" + "\\Users\\" + "<usuario>\\x",
        "bucle local": "127.0.0.1 localhost ::1 0.0.0.0",
        "versión": "windows 0.62.2 y tauri 2.12.4",
        "carpetas de sistema": "C:" + "\\Users\\" + "Public\\Documents y C:" + "\\Users\\" + "Default\\x",
        "hora y MAC": "a las 12:34:56 y aa:bb:cc:dd:ee:ff",
        "permitido": "ver " + "seguridad" + "@" + "proyecto.example",
    }
    allow = {"seguridad" + "@" + "proyecto.example"}
    for label, text in cases.items():
        if not scan_text(text, "x", ident, allow):
            fails.append(f"no detectó: {label}")
    for label, text in clean.items():
        got = scan_text(text, "x", ident, allow)
        if got:
            fails.append(f"falso positivo en {label}: {[str(g) for g in got]}")
    # flujo completo con Git: staging + sin seguimiento + limpio
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        env_git = ["-c", "user.name=t", "-c", "user.email=t@t.invalid", "-c", "commit.gpgsign=false"]
        _git(["init", "-q"], repo)
        (repo / "base.txt").write_text("hola\n", encoding="utf-8")
        _git(["add", "base.txt"], repo)
        _git([*env_git, "commit", "-q", "-m", "base"], repo)
        if scan_repo(repo, ident, allow):
            fails.append("repositorio limpio con falsos positivos")
        (repo / "staged.txt").write_text("ok\n" + cases["usuario"] + "\n", encoding="utf-8")
        _git(["add", "staged.txt"], repo)
        f = scan_repo(repo, ident, allow)
        if not any(x.where == "staged.txt" and x.line == 2 for x in f):
            fails.append(f"no detectó el cambio en staging con su número de línea: {f}")
        (repo / "nuevo.txt").write_text(cases["ruta"] + "\n", encoding="utf-8")
        f = scan_repo(repo, ident, allow)
        if not any(x.where == "nuevo.txt" for x in f):
            fails.append("no detectó el archivo sin seguimiento")
        (repo / "binario.bin").write_bytes(b"\0\1\2" + cases["usuario"].encode())
        f = scan_repo(repo, ident, allow)
        if any(x.where == "binario.bin" for x in f):
            fails.append("analizó un binario")
        (repo / "mod.txt").write_text("limpio\n", encoding="utf-8")
        _git(["add", "mod.txt"], repo)
        _git([*env_git, "commit", "-q", "-m", "m"], repo)
        (repo / "mod.txt").write_text("limpio\n" + cases["correo"] + "\n", encoding="utf-8")  # sin staging
        if any(x.where == "mod.txt" for x in scan_repo(repo, ident, allow)):
            fails.append("el modo por defecto no debe mirar cambios sin staging")
        if not any(x.where == "mod.txt" for x in scan_repo(repo, ident, allow, include_unstaged=True)):
            fails.append("--include-unstaged no detectó el cambio del árbol de trabajo")
    if fails:
        print("AUTOTEST FALLÓ:\n  " + "\n  ".join(fails))
        return 1
    print(f"autotest OK: {len(cases)} tipos detectados, {len(clean)} textos legítimos sin falsos positivos, staging/sin seguimiento/sin staging")
    return 0


def main(argv: list[str]) -> int:
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8")
        except Exception:  # noqa: BLE001
            pass
    if "--self-test" in argv:
        return self_test()
    ident = Identity.from_environment()
    findings = scan_repo(ROOT, ident, load_allowlist(), include_unstaged="--include-unstaged" in argv)
    if findings:
        print(f"{len(findings)} coincidencia(s) de datos personales (valores enmascarados):")
        for f in findings:
            print("  " + str(f))
        return 1
    print("OK: sin datos personales en staging ni en archivos sin seguimiento"
          + (" ni en cambios sin staging" if "--include-unstaged" in argv else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
