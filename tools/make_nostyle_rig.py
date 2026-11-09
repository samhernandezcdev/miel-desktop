"""Genera src/assets/bee/bee-rig.nostyle.svg: el rig maestro SIN el elemento <style>.

Por qué existe
--------------
En producción Tauri añade un nonce a `style-src` de la CSP; con un nonce presente el navegador ignora
'unsafe-inline', así que el <style> embebido en bee-rig.svg (inyectado al DOM como SVG inline) queda
bloqueado y se pierden `.face{opacity:0}`, los selectores `[data-state]` y los @keyframes. La solución es
servir el CSS como archivo propio (src/assets/bee/bee.css, permitido por 'self') y usar un rig sin <style>.

Origen (sin tocar los originales)
---------------------------------
Se genera con la MISMA fuente de verdad: `tools/build_bee.py::build_rig_svg("")` (CSS vacío) y se elimina
por completo el <style> vacío resultante. Los archivos originales (bee-rig.svg, bee.css, bee-*.svg,
palette.json) NO se modifican. El script comprueba:
  1. el resultado no contiene ningún <style>;
  2. original bee-rig.svg sin su <style> == resultado (la única diferencia es el <style>);
  3. bee.css == build_bee.build_css(): la tabla BASE (idéntica al <style> del rig standalone, que no cambia)
     más la capa de rendimiento de E0b (steps() alineados y sin animar rostros ocultos).

Uso:  python -I tools/make_nostyle_rig.py            (genera y verifica)
      python -I tools/make_nostyle_rig.py --check    (solo verifica que el archivo está al día)
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import build_bee  # noqa: E402  (fuente única de verdad del personaje)

BEE = ROOT / "src" / "assets" / "bee"
TARGET = BEE / "bee-rig.nostyle.svg"


def norm(text: str) -> str:
    return text.replace("\r\n", "\n")


def build() -> str:
    svg = build_bee.build_rig_svg("")
    empty_style = "<style>\n\n</style>\n"
    if empty_style not in svg:
        raise SystemExit("build_rig_svg('') ya no produce el <style> vacío esperado: revisar el generador")
    return svg.replace(empty_style, "", 1)


def verify(nostyle: str) -> None:
    if "<style" in nostyle:
        raise SystemExit("ERROR: el rig generado todavía contiene <style>")
    original = norm((BEE / "bee-rig.svg").read_text(encoding="utf-8"))
    original_wo_style = re.sub(r"<style>.*?</style>\n", "", original, count=1, flags=re.S)
    if original_wo_style != nostyle:
        raise SystemExit("ERROR: el rig sin estilo difiere del original en algo más que el <style>")
    css_original = norm((BEE / "bee.css").read_text(encoding="utf-8"))
    if css_original.strip() != build_bee.build_css().strip():
        raise SystemExit("ERROR: bee.css no coincide con build_bee.build_css() (regenerar con python -I tools/build_bee.py)")
    # el <style> que se quitó del original es exactamente la tabla BASE; bee.css = base + capa de rendimiento
    base = build_bee.build_base_css().strip()
    m = re.search(r"<style>\n(.*?)\n</style>", original, flags=re.S)
    if not m or m.group(1).strip() != base:
        raise SystemExit("ERROR: el <style> del rig original no coincide con build_bee.build_base_css()")
    if not css_original.strip().startswith(base):
        raise SystemExit("ERROR: bee.css no empieza por la tabla base (el rig standalone y bee.css divergen)")


def main() -> None:
    nostyle = build()
    verify(nostyle)
    if "--check" in sys.argv:
        current = norm(TARGET.read_text(encoding="utf-8")) if TARGET.exists() else ""
        if current != nostyle:
            raise SystemExit(f"{TARGET.name} no está al día: ejecutar python -I tools/make_nostyle_rig.py")
        print(f"OK: {TARGET.name} está al día y su única diferencia con bee-rig.svg es la ausencia del <style>")
        return
    TARGET.write_text(nostyle, encoding="utf-8", newline="\n")
    print(f"OK -> {TARGET} ({len(nostyle)} bytes); única diferencia con bee-rig.svg: sin <style>")


if __name__ == "__main__":
    main()
