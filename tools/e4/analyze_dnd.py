#!/usr/bin/env python3
"""E4b: analiza la salida de tools/e4/verify_dnd.ps1 (app REAL con sonda programada).

Uso: python -I tools/e4/analyze_dnd.py <prefijo>   (lee <prefijo>.csv, <prefijo>.log y <prefijo>-meta.json)

El tiempo 0 es la primera lectura de la sonda programada (el motor arranca y sondea al instante); el guion de
estados usa segundos desde ese mismo instante, así que las comprobaciones son relativas y con tolerancia.
Línea del registro del motor: `[miel] t=<ms Unix> vista v<N> <Comportamiento>/<Expresión> paused=.. hidden=.. suppressed=.. ...`.
Visibilidad: muestras cada 0,5 s de IsWindowVisible de la ventana principal.
"""
import csv
import json
import re
import sys
from pathlib import Path

VIEW = re.compile(r"t=(\d+) vista v(\d+) (\w+)/(\w+) paused=(\w+) hidden=(\w+) suppressed=(\w+) sleeping=(\w+)")
PROBE = re.compile(r"t=(\d+) diag: la sonda simulada devuelve (.+?) \(a los (\d+) s\)")
SETLINE = re.compile(r"t=(\d+) diag: Set\(\"(\w+)\", Bool\((\w+)\)\) a los (\d+) s")


def main(prefix: str) -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    log = Path(prefix + ".log").read_text(encoding="utf-8", errors="replace").splitlines()
    samples = []
    for r in csv.DictReader(open(prefix + ".csv", encoding="utf-8")):
        samples.append((int(r["t_ms"]), r["visible"]))
    # (t_ms, paused, hidden, suppressed)
    views = [(t, p, h, s) for (t, p, h, s) in ((int(m.group(1)), m.group(5) == "true", m.group(6) == "true", m.group(7) == "true") for m in (VIEW.search(l) for l in log) if m)]
    probes = [(int(m.group(1)), m.group(2), int(m.group(3))) for m in (PROBE.search(l) for l in log) if m]
    sets = [(int(m.group(1)), m.group(2), m.group(3)) for m in (SETLINE.search(l) for l in log) if m]
    if not probes:
        print("sin lecturas de la sonda programada en el registro")
        return 2
    t0 = probes[0][0]
    rel = lambda t: (t - t0) / 1000.0  # noqa: E731

    def vis_between(a: float, b: float) -> set[str]:
        return {v for t, v in samples if a <= rel(t) <= b}

    sup_changes = []  # (rel, suppressed)
    last = None
    for t, p, h, s in views:
        if s != last:
            sup_changes.append((rel(t), s))
            last = s
    hid_changes = []
    last = None
    for t, p, h, s in views:
        if h != last:
            hid_changes.append((rel(t), h))
            last = h

    results: list[tuple[str, bool, str]] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        results.append((name, bool(ok), detail))

    def first_change(value: bool, lo: float, hi: float):
        return next((t for t, s in sup_changes if s == value and lo <= t <= hi), None)

    show_seen = vis_between(8, 14)
    check("0. antes del primer episodio la ventana está visible", show_seen == {"1"}, f"muestras {show_seen}")
    check("0b. la sonda programada se usó (no Windows)", any("SIMULADA" in l for l in log))
    # episodio 1
    s1 = first_change(True, 15, 25)
    e1 = first_change(False, 45, 55)
    check("1a. episodio 1: empieza entre 15 y 25 s (dos lecturas seguidas)", s1 is not None, f"{s1}")
    check("1b. episodio 1: la ventana está oculta durante él", s1 is not None and e1 is not None and vis_between(s1 + 1.5, e1 - 0.6) == {"0"}, f"{vis_between(s1 + 1.5, e1 - 0.6) if s1 and e1 else None}")
    check("1c. episodio 1: termina entre 45 y 55 s y la ventana reaparece", e1 is not None and vis_between(e1 + 1.5, 58) == {"1"}, f"{e1}")
    # episodio 2 + anulación
    s2 = first_change(True, 60, 72)
    check("2a. episodio 2: empieza entre 60 y 72 s", s2 is not None, f"{s2}")
    ov = first_change(False, 74, 79)
    check("2b. «Mostrar de todos modos» (override@75) anula el episodio: vista sin supresión", ov is not None, f"{ov}")
    check("2c. …y la ventana está visible de 78 a 108 s aunque el juego siga en pantalla", vis_between(78, 108) == {"1"}, f"{vis_between(78, 108)}")
    check("2d. el fin del episodio (ok@110) no cambia nada visible (ya estaba visible)", vis_between(108, 124) == {"1"}, f"{vis_between(108, 124)}")
    # episodio 3: la anulación no sobrevive al episodio
    s3 = first_change(True, 125, 137)
    check("3a. episodio 3 (juego otra vez): vuelve a suprimir (la anulación no sobrevivió)", s3 is not None, f"{s3}")
    check("3b. …y la ventana se oculta", s3 is not None and vis_between(s3 + 1.5, 138.5) == {"0"}, f"{vis_between(s3 + 1.5, 138.5) if s3 else None}")
    # interruptor en vivo
    off = next((rel(t) for t, k, v in sets if k == "dnd_auto_enabled" and v == "false"), None)
    check("4a. el guion apagó el interruptor (~140 s)", off is not None and 138 <= off <= 144, f"{off}")
    off_end = first_change(False, off or 0, (off or 0) + 3)
    check("4b. apagarlo termina el episodio de inmediato (≤ 3 s)", off_end is not None, f"{off_end}")
    check("4c. …la ventana reaparece y sigue visible con el juego aún en pantalla (hasta 149 s)", off is not None and vis_between(off + 2.5, 149) == {"1"}, f"{vis_between((off or 0) + 2.5, 149)}")
    check("4d. sin sondeo con el interruptor apagado: ninguna supresión entre 143 y 149,5 s", not any(s and 143 <= t <= 149.5 for t, s in sup_changes), f"{[c for c in sup_changes if 143 <= c[0] <= 149.5]}")
    on = next((rel(t) for t, k, v in sets if k == "dnd_auto_enabled" and v == "true"), None)
    check("5a. el guion encendió el interruptor (~150 s)", on is not None and 148 <= on <= 154, f"{on}")
    s5 = first_change(True, (on or 0), (on or 0) + 12)
    check("5b. encenderlo reanuda el sondeo: vuelve a suprimir en ≤ 12 s", s5 is not None, f"{s5}")
    # ocultar a mano detiene el sondeo
    h_on = next((t for t, h in hid_changes if h and 160 <= t <= 172), None)
    h_off = next((t for t, h in hid_changes if not h and 182 <= t <= 192), None)
    check("6a. hide@165: la persona oculta a Miel", h_on is not None, f"{h_on}")
    check("6b. la ventana está oculta durante ese intervalo", h_on is not None and h_off is not None and vis_between(h_on + 1.5, h_off - 0.6) == {"0"}, "")
    ok_read = next((n for t, st, n in probes if "AcceptsNotifications" in st and n >= 170), None)
    check("6c. oculta a mano: el cambio ok@175 NO se observa hasta mostrar (≥ 183 s): el sondeo estaba detenido", ok_read is not None and ok_read >= 183, f"primera lectura 'ok' a los {ok_read} s")
    check("6d. show@185 con supresión en vigor: visible (anulación) y la supresión termina enseguida", h_off is not None and vis_between(h_off + 1.5, h_off + 4.0) == {"1"} and not sup_changes[-1][1], f"{sup_changes[-1]}")
    check("7. el proceso sigue vivo hasta el final", not json.load(open(prefix + "-meta.json", encoding="utf-8"))["exited_early"])

    width = max(len(n) for n, _, _ in results)
    bad = 0
    for n, ok, d in results:
        print(f"{'OK ' if ok else 'FALLA'}  {n:<{width}}  {d}")
        bad += not ok
    print(f"\n{len(results) - bad}/{len(results)} comprobaciones OK")
    print("transiciones de supresión (s desde la 1ª lectura):", [(round(t, 1), s) for t, s in sup_changes])
    print("transiciones de ocultación manual:", [(round(t, 1), h) for t, h in hid_changes])
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
