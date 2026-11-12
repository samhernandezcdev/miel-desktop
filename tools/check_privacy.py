#!/usr/bin/env python3
"""
Miel: comprobación PREVENTIVA de privacidad (ver docs/PRIVACY.md y docs/plan/v0.2.0.md §10).

Es un control preventivo, NO una demostración de que Miel no accede a datos: busca símbolos y
configuraciones que contradicen la política y no sustituye a la revisión de código.

Comprobaciones (todas deben pasar):
  1. Símbolos prohibidos (APIs de teclado/ratón globales, captura de pantalla, lectura de títulos de
     ventana, UI Automation; en el frontend, red del navegador y captura) en src-tauri/src/** y src/**,
     con límites de palabra y sin contar comentarios.
  2. capabilities/*.json y permisos de aplicación: lista permitida EXACTA (tools/privacy_policy.json).
  3. Cargo.toml: features de `windows` y `tauri` dentro de la lista permitida; CSP de producción sin
     hosts de red.
  4. Toda API de la crate `windows` importada está en la lista permitida y TODA la lista permitida
     existe en la versión de `windows` en uso (se lee el código fuente del registro de cargo).
  5. Línea base de dependencias con capacidades de red (cargo tree) y de paquetes npm de producción
     (npm ls): falla ante una dependencia nueva (tools/privacy_baseline.json).

Uso:
  python -I tools/check_privacy.py                 comprobación completa (necesita cargo y npm ci)
  python -I tools/check_privacy.py --self-test     prueba negativa: debe FALLAR sobre entradas malas
  python -I tools/check_privacy.py --update-baseline   reescribe la línea base (revísala antes de subirla)
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
POLICY_FILE = ROOT / "tools" / "privacy_policy.json"
BASELINE_FILE = ROOT / "tools" / "privacy_baseline.json"
TARGET = "x86_64-pc-windows-msvc"

# ---- símbolos prohibidos -------------------------------------------------------------------------
# (nombre, expresión con límites de palabra). Editable: añadir, no quitar, sin aprobación.
RUST_FORBIDDEN = [
    ("SetWindowsHookEx*", r"\bSetWindowsHookEx\w*"),
    ("RegisterRawInputDevices", r"\bRegisterRawInputDevices\b"),
    ("GetRawInputData", r"\bGetRawInputData\b"),
    ("GetRawInputBuffer", r"\bGetRawInputBuffer\b"),
    ("GetAsyncKeyState", r"\bGetAsyncKeyState\b"),
    ("GetKeyState", r"\bGetKeyState\b"),
    ("GetKeyboardState", r"\bGetKeyboardState\b"),
    ("BitBlt", r"\bBitBlt\b"),
    ("PrintWindow", r"\bPrintWindow\b"),
    ("GetWindowText*", r"\bGetWindowText\w*"),
    ("InternalGetWindowText", r"\bInternalGetWindowText\b"),
    ("Identificar la ventana o app en primer plano (E4b)", r"\bGetForegroundWindow\b|\bGetWindowThreadProcessId\b|\bQueryFullProcessImageName\w*|\bGetModuleFileNameEx\w*|\bEnumWindows\b|\bFindWindow\w*"),
    ("UI Automation", r"\b(?:C|I)?UIAutomation\w*|\bWin32_UI_Accessibility\b|::Accessibility::"),
    ("Desktop Duplication", r"\bDuplicateOutput\d*\b|\bIDXGIOutputDuplication\w*|\bDXGI_OUTDUPL\w*"),
    ("Graphics Capture", r"\bGraphicsCapture\w*|\bIGraphicsCaptureItemInterop\b|Graphics::Capture|Graphics_Capture"),
]
JS_FORBIDDEN = [
    ("fetch", r"\bfetch\b"),
    ("XMLHttpRequest", r"\bXMLHttpRequest\b"),
    ("WebSocket", r"\bWebSocket\b"),
    ("EventSource", r"\bEventSource\b"),
    ("sendBeacon", r"\bsendBeacon\b"),
    ("getDisplayMedia", r"\bgetDisplayMedia\b"),
]

# Nombres de crates que indican capacidad de red o complementos nuevos (cargo tree).
CARGO_NET_WATCH = re.compile(
    r"^(reqwest|hyper|hyper-.*|h2|h3|ureq|curl|curl-sys|isahc|surf|attohttpc|http|httparse|http-body.*|"
    r"tungstenite|tokio-tungstenite|tokio|mio|socket2|async-std|async-net|smol|rustls.*|native-tls|"
    r"openssl.*|schannel|webpki.*|quinn.*|hickory-.*|trust-dns.*|tower|tower-http|url|"
    r"tauri-plugin-.*)$"
)
NPM_NET_WATCH = re.compile(r"^(axios|node-fetch|cross-fetch|isomorphic-fetch|undici|got|ky|superagent|request|ws|socket\.io.*|engine\.io.*|whatwg-fetch)$")


# ---- eliminación de comentarios ------------------------------------------------------------------
def strip_comments(text: str, lang: str) -> str:
    """Sustituye los comentarios por espacios (conserva saltos de línea). lang: 'rust' | 'js'."""
    out: list[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        two = text[i:i + 2]
        if two == "//":
            while i < n and text[i] != "\n":
                out.append(" ")
                i += 1
            continue
        if two == "/*":
            depth = 1
            out.append("  ")
            i += 2
            while i < n and depth:
                if lang == "rust" and text[i:i + 2] == "/*":
                    depth += 1
                    out.append("  ")
                    i += 2
                elif text[i:i + 2] == "*/":
                    depth -= 1
                    out.append("  ")
                    i += 2
                else:
                    out.append("\n" if text[i] == "\n" else " ")
                    i += 1
            continue
        if lang == "rust" and c == "r" and re.match(r'r#*"', text[i:i + 8]) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            m = re.match(r'r(#*)"', text[i:])
            end = '"' + m.group(1)
            j = text.find(end, i + len(m.group(0)))
            j = n if j == -1 else j + len(end)
            out.append(text[i:j])
            i = j
            continue
        if c in "\"`" or (c == "'" and lang == "js"):
            q = c
            j = i + 1
            while j < n and text[j] != q:
                j += 2 if text[j] == "\\" else 1
            j = min(j + 1, n)
            out.append(text[i:j])
            i = j
            continue
        if lang == "rust" and c == "'":
            # literal de carácter ('x', '\n') frente a lifetime ('a)
            if text[i + 1:i + 2] == "\\":
                j = text.find("'", i + 2)
                j = n if j == -1 else j + 1
                out.append(text[i:j])
                i = j
                continue
            if text[i + 2:i + 3] == "'":
                out.append(text[i:i + 3])
                i += 3
                continue
        out.append(c)
        i += 1
    return "".join(out)


def _lines_with_matches(code: str, regex: str) -> list[tuple[int, str]]:
    res = []
    for m in re.finditer(regex, code):
        line = code.count("\n", 0, m.start()) + 1
        res.append((line, m.group(0)))
    return res


# ---- 1. símbolos ---------------------------------------------------------------------------------
def check_sources(root: Path) -> list[str]:
    problems: list[str] = []
    for path in sorted((root / "src-tauri" / "src").rglob("*.rs")):
        code = strip_comments(path.read_text(encoding="utf-8"), "rust")
        for name, rx in RUST_FORBIDDEN:
            for line, found in _lines_with_matches(code, rx):
                problems.append(f"{path.relative_to(root).as_posix()}:{line}: símbolo prohibido {name} ('{found}')")
    for path in sorted((root / "src").rglob("*")):
        if path.suffix not in {".ts", ".tsx", ".js", ".jsx", ".mjs", ".html"} or not path.is_file():
            continue
        code = strip_comments(path.read_text(encoding="utf-8"), "js")
        for name, rx in JS_FORBIDDEN + RUST_FORBIDDEN:
            for line, found in _lines_with_matches(code, rx):
                problems.append(f"{path.relative_to(root).as_posix()}:{line}: símbolo prohibido {name} ('{found}')")
    return problems


# ---- 2. capabilities y permisos --------------------------------------------------------------------
def check_capabilities(root: Path, policy: dict) -> list[str]:
    problems: list[str] = []
    cap_dir = root / "src-tauri" / "capabilities"
    allowed = policy["capabilities"]
    found = {p.name for p in cap_dir.glob("*.json")}
    for extra in sorted(found - set(allowed)):
        problems.append(f"capabilities/{extra}: archivo de capacidades NO aprobado")
    for name, spec in allowed.items():
        p = cap_dir / name
        if not p.exists():
            problems.append(f"capabilities/{name}: falta (la lista permitida lo exige)")
            continue
        data = json.loads(p.read_text(encoding="utf-8"))
        perms = data.get("permissions", [])
        for perm in perms:
            if not isinstance(perm, str):
                problems.append(f"capabilities/{name}: permiso con ámbito/objeto {perm!r}: requiere aprobación explícita")
        got = {x for x in perms if isinstance(x, str)}
        for extra in sorted(got - set(spec["permissions"])):
            problems.append(f"capabilities/{name}: permiso NO aprobado '{extra}'")
        for missing in sorted(set(spec["permissions"]) - got):
            problems.append(f"capabilities/{name}: falta el permiso aprobado '{missing}' (actualiza la política si fue deliberado)")
        if sorted(data.get("windows", [])) != sorted(spec["windows"]):
            problems.append(f"capabilities/{name}: ventanas {data.get('windows')} distintas de las aprobadas {spec['windows']}")
        if "remote" in data:
            problems.append(f"capabilities/{name}: 'remote' (URLs remotas con acceso IPC) no está aprobado")
    perm_dir = root / "src-tauri" / "permissions" / "autogenerated"
    got_files = {p.stem for p in perm_dir.glob("*.toml")} if perm_dir.exists() else set()
    for extra in sorted(got_files - set(policy["app_permission_files"])):
        problems.append(f"permissions/autogenerated/{extra}.toml: comando de aplicación NO aprobado")
    for missing in sorted(set(policy["app_permission_files"]) - got_files):
        problems.append(f"permissions/autogenerated/{missing}.toml: falta (actualiza la política si fue deliberado)")
    return problems


# ---- 3. manifiesto y CSP ---------------------------------------------------------------------------
def _features(dep) -> list[str]:
    return list(dep.get("features", [])) if isinstance(dep, dict) else []


def check_manifest(root: Path, policy: dict) -> list[str]:
    problems: list[str] = []
    cargo = tomllib.loads((root / "src-tauri" / "Cargo.toml").read_text(encoding="utf-8"))
    tables = [cargo.get("dependencies", {}), cargo.get("dev-dependencies", {}), cargo.get("build-dependencies", {})]
    for tgt in cargo.get("target", {}).values():
        tables += [tgt.get("dependencies", {}), tgt.get("dev-dependencies", {}), tgt.get("build-dependencies", {})]
    for t in tables:
        for feat in _features(t.get("windows")):
            if feat not in policy["windows_features"]:
                problems.append(f"Cargo.toml: feature de `windows` NO aprobada '{feat}'")
        for feat in _features(t.get("tauri")):
            if feat not in policy["tauri_features"]:
                problems.append(f"Cargo.toml: feature de `tauri` NO aprobada '{feat}'")
    conf = json.loads((root / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    csp = conf.get("app", {}).get("security", {}).get("csp", "")
    if isinstance(csp, dict):
        csp = "; ".join(f"{k} {v}" for k, v in csp.items())
    for host in re.findall(r"(?:https?|wss?)://[^\s;]+", csp):
        if host not in policy["csp_allowed_hosts"]:
            problems.append(f"tauri.conf.json: la CSP de producción permite el host '{host}'")
    if re.search(r"'unsafe-eval'|\*|\bdata:|\bblob:", csp):
        problems.append("tauri.conf.json: la CSP de producción incluye 'unsafe-eval', comodines, data: o blob:")
    return problems


# ---- 4. API de la crate `windows` -----------------------------------------------------------------
def _parse_use_tree(prefix: str, body: str) -> list[str]:
    body = body.strip()
    if "{" not in body:
        return [f"{prefix}{body}"] if body else []
    head, rest = body.split("{", 1)
    inner = rest.rsplit("}", 1)[0]
    parts, depth, cur = [], 0, ""
    for ch in inner:
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    parts.append(cur)
    out: list[str] = []
    for p in parts:
        p = re.sub(r"\s+as\s+\w+$", "", p.strip())
        if p:
            out += _parse_use_tree(prefix + re.sub(r"\s+", "", head), p)
    return out


def used_windows_api(root: Path) -> set[str]:
    used: set[str] = set()
    for path in (root / "src-tauri" / "src").rglob("*.rs"):
        code = strip_comments(path.read_text(encoding="utf-8"), "rust")
        for m in re.finditer(r"\buse\s+windows::([^;]+);", code):
            for item in _parse_use_tree("", m.group(1)):
                if item.endswith("::self"):
                    item = item[: -len("::self")]
                used.add(item)
        no_use = re.sub(r"\buse\s+windows::[^;]+;", lambda m: re.sub(r"[^\n]", " ", m.group(0)), code)
        for m in re.finditer(r"(?<![\w:])windows::((?:Win32|core|Foundation)(?:::\w+)*::[A-Za-z_]\w*)", no_use):
            tail = m.group(1)
            if tail.split("::")[-1][:1].isalpha():
                used.add(tail)
    return used


def windows_crate_dir(root: Path) -> Path | None:
    req = tomllib.loads((root / "src-tauri" / "Cargo.toml").read_text(encoding="utf-8"))
    dep = None
    for t in [req.get("dependencies", {})] + [v.get("dependencies", {}) for v in req.get("target", {}).values()]:
        if "windows" in t:
            dep = t["windows"]
    ver = dep["version"] if isinstance(dep, dict) else dep
    mm = re.search(r"(\d+)\.(\d+)", ver or "")
    r = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", TARGET,
         "--manifest-path", str(root / "src-tauri" / "Cargo.toml")],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("cargo metadata falló:\n" + r.stderr[-800:])
    for pkg in json.loads(r.stdout)["packages"]:
        if pkg["name"] == "windows" and mm and pkg["version"].startswith(f"{mm.group(1)}.{mm.group(2)}."):
            return Path(pkg["manifest_path"]).parent
    return None


def check_windows_api(root: Path, policy: dict, crate_dir: Path | None, verify_exists: bool = True) -> list[str]:
    problems: list[str] = []
    allowed = set(policy["windows_api"])
    for item in sorted(used_windows_api(root)):
        if item not in allowed:
            problems.append(f"API de `windows` usada y NO aprobada: windows::{item} (añádela a la política solo tras revisión de privacidad)")
    if not verify_exists:
        return problems
    if crate_dir is None:
        return problems + ["no se encontró la crate `windows` en el registro de cargo (¿falta `cargo fetch`?)"]
    for item in sorted(allowed):
        parts = item.split("::")
        if parts[0] == "core":
            continue  # re-exporta windows-core; no se comprueba en el código de `windows`
        mod_file = crate_dir / "src" / "Windows" / Path(*parts[:-1]) / "mod.rs"
        if not mod_file.exists():
            problems.append(f"lista permitida: no existe el módulo de {item} en windows {crate_dir.name}")
            continue
        text = mod_file.read_text(encoding="utf-8", errors="replace")
        name = re.escape(parts[-1])
        if not re.search(rf"pub\s+(?:unsafe\s+)?(?:const\s+|static\s+)?(?:fn|const|static|struct|type|enum|union)\s+{name}\b", text):
            problems.append(f"lista permitida: el símbolo {item} NO existe en {crate_dir.name}")
    return problems


# ---- 5. dependencias con capacidad de red -----------------------------------------------------------
def cargo_tree_crates(root: Path) -> dict[str, str]:
    r = subprocess.run(
        ["cargo", "tree", "--locked", "--manifest-path", str(root / "src-tauri" / "Cargo.toml"),
         "--target", TARGET, "-e", "normal,build", "--prefix", "none", "--no-dedupe"],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("cargo tree falló:\n" + r.stderr[-800:])
    crates: dict[str, str] = {}
    for line in r.stdout.splitlines():
        m = re.match(r"^([\w.-]+) v([\w.+-]+)", line)
        if m:
            crates[m.group(1)] = m.group(2)
    return crates


def npm_runtime_packages(root: Path) -> dict[str, str]:
    npm = shutil.which("npm") or shutil.which("npm.cmd")
    if not npm:
        raise RuntimeError("npm no está en el PATH")
    if not (root / "node_modules").exists():
        raise RuntimeError("falta node_modules: ejecuta `npm ci` antes")
    r = subprocess.run([npm, "ls", "--omit=dev", "--all", "--json"], capture_output=True, text=True,
                       encoding="utf-8", cwd=root)
    data = json.loads(r.stdout or "{}")
    found: dict[str, str] = {}

    def walk(deps: dict) -> None:
        for name, info in (deps or {}).items():
            found[name] = info.get("version", "?")
            walk(info.get("dependencies", {}))

    walk(data.get("dependencies", {}))
    return found


def net_snapshot(cargo: dict[str, str], npm: dict[str, str]) -> dict:
    return {
        "cargo_network_capable": {k: v for k, v in sorted(cargo.items()) if CARGO_NET_WATCH.match(k)},
        "npm_runtime": dict(sorted(npm.items())),
    }


def check_baseline(snapshot: dict, baseline: dict) -> list[str]:
    problems: list[str] = []
    for name, ver in snapshot["cargo_network_capable"].items():
        if name not in baseline["cargo_network_capable"]:
            problems.append(f"crate con capacidad de red NUEVA respecto a la línea base: {name} {ver}")
    for name, ver in snapshot["npm_runtime"].items():
        if name not in baseline["npm_runtime"]:
            tag = " (con capacidad de red)" if NPM_NET_WATCH.match(name) else ""
            problems.append(f"paquete npm de producción NUEVO respecto a la línea base: {name} {ver}{tag}")
    return problems


# ---- ejecución ---------------------------------------------------------------------------------------
def load_policy() -> dict:
    return json.loads(POLICY_FILE.read_text(encoding="utf-8"))


def run_all(root: Path, update_baseline: bool) -> int:
    policy = load_policy()
    problems = check_sources(root) + check_capabilities(root, policy) + check_manifest(root, policy)
    try:
        crate_dir = windows_crate_dir(root)
        problems += check_windows_api(root, policy, crate_dir)
        snapshot = net_snapshot(cargo_tree_crates(root), npm_runtime_packages(root))
    except RuntimeError as e:
        print(f"ERROR de entorno: {e}")
        return 2
    if update_baseline:
        old = json.loads(BASELINE_FILE.read_text(encoding="utf-8")) if BASELINE_FILE.exists() else {}
        snapshot["known_network_points"] = old.get("known_network_points", [])
        snapshot["_comment"] = old.get("_comment", "")
        BASELINE_FILE.write_text(json.dumps(snapshot, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"línea base reescrita: {BASELINE_FILE.relative_to(ROOT)} (revisa el diff antes de subirla)")
    else:
        problems += check_baseline(snapshot, json.loads(BASELINE_FILE.read_text(encoding="utf-8")))
    if problems:
        print("PRIVACIDAD: la comprobación FALLÓ:")
        for p in problems:
            print("  -", p)
        return 1
    print("PRIVACIDAD: OK (control preventivo; no demuestra la ausencia de acceso a datos).")
    return 0


# ---- prueba negativa ----------------------------------------------------------------------------------
def self_test() -> int:
    policy = load_policy()
    base = json.loads(BASELINE_FILE.read_text(encoding="utf-8"))
    failures: list[str] = []

    def expect(label: str, problems: list[str], should_fail: bool) -> None:
        if bool(problems) != should_fail:
            failures.append(f"{label}: se esperaba {'FALLO' if should_fail else 'PASO'} y fue {'fallo' if problems else 'paso'} {problems[:2]}")

    with tempfile.TemporaryDirectory(prefix="miel-privacy-selftest-") as tmp:
        t = Path(tmp)
        for rel in ("src-tauri/src", "src", "src-tauri/capabilities", "src-tauri/permissions"):
            shutil.copytree(ROOT / rel, t / rel)
        for rel in ("src-tauri/Cargo.toml", "src-tauri/tauri.conf.json"):
            (t / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / rel, t / rel)

        expect("control positivo (copia limpia): fuentes", check_sources(t), False)
        expect("control positivo: capabilities", check_capabilities(t, policy), False)
        expect("control positivo: manifiesto", check_manifest(t, policy), False)
        expect("control positivo: API windows (sin crate)", check_windows_api(t, policy, None, verify_exists=False), False)

        rs = t / "src-tauri" / "src" / "lib.rs"
        original_rs = rs.read_text(encoding="utf-8")
        # cada símbolo prohibido, uno a uno
        samples = {
            "SetWindowsHookEx*": "SetWindowsHookExW", "RegisterRawInputDevices": "RegisterRawInputDevices",
            "GetRawInputData": "GetRawInputData", "GetRawInputBuffer": "GetRawInputBuffer",
            "GetAsyncKeyState": "GetAsyncKeyState", "GetKeyState": "GetKeyState",
            "GetKeyboardState": "GetKeyboardState", "BitBlt": "BitBlt", "PrintWindow": "PrintWindow",
            "GetWindowText*": "GetWindowTextW", "InternalGetWindowText": "InternalGetWindowText",
            "Identificar la ventana o app en primer plano (E4b)": "GetForegroundWindow",
            "UI Automation": "IUIAutomation", "Desktop Duplication": "DuplicateOutput1",
            "Graphics Capture": "GraphicsCaptureSession",
        }
        for name, sym in samples.items():
            rs.write_text(original_rs + f"\nfn x() {{ let _ = {sym}; }}\n", encoding="utf-8")
            expect(f"inyección Rust {name}", check_sources(t), True)
        # comentarios y límites de palabra: NO deben fallar
        rs.write_text(original_rs + "\n// GetAsyncKeyState SetWindowsHookExW\n/* BitBlt /* anidado PrintWindow */ GetKeyState */\n"
                      "/// GetKeyboardState\nfn MyGetKeyStateCache() {}\nfn y() { let _s = 'a'; let _l: &'static str = \"ok\"; }\n", encoding="utf-8")
        expect("comentarios y límites de palabra (Rust)", check_sources(t), False)
        rs.write_text(original_rs + '\nfn z() { let _ = "GetAsyncKeyState"; }\n', encoding="utf-8")
        expect("símbolo en una cadena cuenta (conservador)", check_sources(t), True)
        rs.write_text(original_rs, encoding="utf-8")

        ts = t / "src" / "selftest_inject.ts"
        for name, sym in {"fetch": "fetch('x')", "XMLHttpRequest": "new XMLHttpRequest()", "WebSocket": "new WebSocket('ws://x')",
                          "EventSource": "new EventSource('x')", "sendBeacon": "navigator.sendBeacon('x')",
                          "getDisplayMedia": "navigator.mediaDevices.getDisplayMedia()"}.items():
            ts.write_text(f"export const a = () => {sym};\n", encoding="utf-8")
            expect(f"inyección JS {name}", check_sources(t), True)
        ts.write_text("// fetch( XMLHttpRequest\n/* WebSocket */\nexport const fetchView = () => 1;\nexport const q = `ok`;\n", encoding="utf-8")
        expect("comentarios y límites de palabra (JS)", check_sources(t), False)
        ts.unlink()

        cap = t / "src-tauri" / "capabilities" / "default.json"
        cap_orig = cap.read_text(encoding="utf-8")
        for extra in ("http:default", "fs:default", "shell:allow-open", "core:default"):
            data = json.loads(cap_orig)
            data["permissions"].append(extra)
            cap.write_text(json.dumps(data), encoding="utf-8")
            expect(f"permiso nuevo {extra}", check_capabilities(t, policy), True)
        data = json.loads(cap_orig)
        data["permissions"][0] = {"identifier": "fs:allow-read", "allow": [{"path": "*"}]}
        cap.write_text(json.dumps(data), encoding="utf-8")
        expect("permiso con ámbito", check_capabilities(t, policy), True)
        data = json.loads(cap_orig)
        data["permissions"].pop()
        cap.write_text(json.dumps(data), encoding="utf-8")
        expect("permiso aprobado que falta", check_capabilities(t, policy), True)
        cap.write_text(cap_orig, encoding="utf-8")
        (t / "src-tauri" / "capabilities" / "otra.json").write_text(cap_orig, encoding="utf-8")
        expect("archivo de capacidades nuevo", check_capabilities(t, policy), True)
        (t / "src-tauri" / "capabilities" / "otra.json").unlink()
        # E2: la capability de la ventana de ajustes está acotada a su ventana y a sus permisos exactos
        scap = t / "src-tauri" / "capabilities" / "settings.json"
        scap_orig = scap.read_text(encoding="utf-8")
        for label, mutate in (
            ("settings: permiso nuevo core:default", lambda d: d["permissions"].append("core:default")),
            ("settings: permiso de otra ventana (allow-get-view)", lambda d: d["permissions"].append("allow-get-view")),
            ("settings: también para main", lambda d: d.__setitem__("windows", ["main", "settings"])),
            ("settings: permiso aprobado que falta", lambda d: d["permissions"].pop()),
            ("settings: remote", lambda d: d.__setitem__("remote", {"urls": ["https://example.com"]})),
        ):
            data = json.loads(scap_orig)
            mutate(data)
            scap.write_text(json.dumps(data), encoding="utf-8")
            expect(label, check_capabilities(t, policy), True)
        scap.write_text(scap_orig, encoding="utf-8")
        data = json.loads(cap_orig)
        data["permissions"].append("allow-set-setting")  # `main` no debe poder cambiar ajustes
        cap.write_text(json.dumps(data), encoding="utf-8")
        expect("main con allow-set-setting", check_capabilities(t, policy), True)
        cap.write_text(cap_orig, encoding="utf-8")
        expect("control positivo tras restaurar capabilities", check_capabilities(t, policy), False)
        (t / "src-tauri" / "permissions" / "autogenerated" / "nuevo_comando.toml").write_text("x = 1\n", encoding="utf-8")
        expect("comando de aplicación nuevo", check_capabilities(t, policy), True)
        (t / "src-tauri" / "permissions" / "autogenerated" / "nuevo_comando.toml").unlink()

        cargo = t / "src-tauri" / "Cargo.toml"
        cargo_orig = cargo.read_text(encoding="utf-8")
        cargo.write_text(cargo_orig.replace('"Win32_UI_Shell"', '"Win32_UI_Shell", "Win32_Networking_WinSock"'), encoding="utf-8")
        expect("feature de windows de red", check_manifest(t, policy), True)
        cargo.write_text(cargo_orig.replace('features = ["tray-icon", "test"]', 'features = ["tray-icon", "test", "devtools"]'), encoding="utf-8")
        expect("feature de tauri nueva", check_manifest(t, policy), True)
        cargo.write_text(cargo_orig, encoding="utf-8")
        conf = t / "src-tauri" / "tauri.conf.json"
        conf_orig = conf.read_text(encoding="utf-8")
        conf.write_text(conf_orig.replace("connect-src ipc: http://ipc.localhost", "connect-src ipc: http://ipc.localhost https://example.com", 1), encoding="utf-8")
        expect("CSP con host remoto", check_manifest(t, policy), True)
        conf.write_text(conf_orig, encoding="utf-8")

        rs.write_text(original_rs + "\nuse windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;\n", encoding="utf-8")
        expect("API windows no aprobada y prohibida", check_windows_api(t, policy, None, verify_exists=False), True)
        rs.write_text(original_rs + "\nuse windows::Win32::System::Ole::{OleInitialize, OleUninitialize};\n", encoding="utf-8")
        expect("API windows no aprobada (grupo)", check_windows_api(t, policy, None, verify_exists=False), True)
        rs.write_text(original_rs, encoding="utf-8")
        fake = json.loads(json.dumps(policy))
        fake["windows_api"].append("Win32::UI::Shell::SimboloQueNoExiste")
        crate = None
        try:
            crate = windows_crate_dir(ROOT)
        except RuntimeError:
            crate = None
        if crate:
            expect("símbolo permitido inexistente", check_windows_api(t, fake, crate), True)
            expect("lista permitida real existe en la crate", check_windows_api(ROOT, policy, crate), False)
        else:
            print("aviso: crate `windows` no disponible; se omite la comprobación de existencia en el autotest")

    snap = {"cargo_network_capable": dict(base["cargo_network_capable"]), "npm_runtime": dict(base["npm_runtime"])}
    expect("línea base sin cambios", check_baseline(snap, base), False)
    snap["cargo_network_capable"]["reqwest"] = "0.12.0"
    expect("crate de red nueva", check_baseline(snap, base), True)
    snap = {"cargo_network_capable": dict(base["cargo_network_capable"]), "npm_runtime": dict(base["npm_runtime"])}
    snap["npm_runtime"]["axios"] = "1.0.0"
    expect("paquete npm nuevo", check_baseline(snap, base), True)

    if failures:
        print("AUTOTEST FALLÓ (la comprobación de privacidad no detecta lo que debería):")
        for f in failures:
            print("  -", f)
        return 1
    print("AUTOTEST OK: la comprobación falla ante todas las entradas indebidas y pasa con las correctas.")
    return 0


def main() -> int:
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8", errors="replace")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--self-test", action="store_true", help="prueba negativa (inyecta símbolos prohibidos en una copia temporal)")
    g.add_argument("--update-baseline", action="store_true", help="reescribe tools/privacy_baseline.json")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    return run_all(ROOT, a.update_baseline)


if __name__ == "__main__":
    sys.exit(main())
