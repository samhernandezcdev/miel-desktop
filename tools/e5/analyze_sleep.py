#!/usr/bin/env python3
"""E5: analiza la salida de tools/e5/verify_sleep.ps1 (app REAL con sonda programada de inactividad).

Uso: python -I -X utf8 tools/e5/analyze_sleep.py <prefijo>   (lee <prefijo>.csv, <prefijo>.log y <prefijo>-meta.json)

El tiempo 0 es la primera lectura de la sonda programada (el motor arranca y consulta al instante); los guiones usan segundos
desde ese mismo instante, así que las comprobaciones son relativas y con tolerancia (±3 s).
Línea de la vista: `[miel] t=<ms Unix> vista v<N> <Comportamiento>/<Expresión> paused=.. hidden=.. suppressed=.. sleeping=.. follow=.. msg=..`
Línea de despertar del planificador (E5, solo `diag`): `[miel] t=<ms Unix> despertar <estado>`.
Visibilidad: muestras cada 0,5 s de IsWindowVisible de la ventana principal.
"""
import csv
import json
import re
import sys
from pathlib import Path

VIEW = re.compile(r"t=(\d+) vista v(\d+) (\w+)/(\w+) paused=(\w+) hidden=(\w+) suppressed=(\w+) sleeping=(\w+) follow=\w+ msg=(\S+)")
WAKE = re.compile(r"t=(\d+) despertar (\w+)")
PROBE = re.compile(r"t=(\d+) diag: la sonda simulada informa inactividad")


def main(prefix: str) -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    log = Path(prefix + ".log").read_text(encoding="utf-8", errors="replace").splitlines()
    samples = [(int(r["t_ms"]), r["visible"]) for r in csv.DictReader(open(prefix + ".csv", encoding="utf-8"))]
    views = []  # (t, behavior, expression, paused, hidden, sleeping, msg)
    for l in log:
        m = VIEW.search(l)
        if m:
            views.append((int(m.group(1)), m.group(3), m.group(4), m.group(5) == "true", m.group(6) == "true", m.group(8) == "true", m.group(9)))
    wakes = [(int(m.group(1)), m.group(2)) for m in (WAKE.search(l) for l in log) if m]
    probes = [int(m.group(1)) for m in (PROBE.search(l) for l in log) if m]
    if not probes:
        print("sin lecturas de la sonda programada en el registro")
        return 2
    t0 = probes[0]
    rel = lambda t: (t - t0) / 1000.0  # noqa: E731

    # (rel, sleeping) en cada cambio, y la última vista antes de t
    sleep_changes = []
    last = None
    for t, _b, _e, _p, _h, s, _m in views:
        if s != last:
            sleep_changes.append((rel(t), s))
            last = s

    def sleeping_at(sec: float):
        cur = False
        for t, _b, _e, _p, _h, s, _m in views:
            if rel(t) <= sec:
                cur = s
        return cur

    def first_change(value: bool, lo: float, hi: float):
        return next((t for t, s in sleep_changes if s == value and lo <= t <= hi), None)

    def always(value: bool, lo: float, hi: float) -> bool:
        pts = [lo + i * 0.5 for i in range(int((hi - lo) / 0.5) + 1)]
        return all(sleeping_at(p) == value for p in pts)

    def expr_after(sec: float, window: float = 2.5):
        """Expresiones vistas en (sec, sec+window]."""
        return {e for t, _b, e, _p, _h, _s, _m in views if sec < rel(t) <= sec + window}

    def vis_between(a: float, b: float):
        return {v for t, v in samples if a <= rel(t) <= b}

    results: list[tuple[str, bool, str]] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        results.append((name, bool(ok), detail))

    check("0. la sonda programada se usó (no Windows)", any("SIMULADA" in l for l in log))
    check("0b. visible y despierta durante los primeros 18 s", always(False, 3, 18) and vis_between(5, 18) == {"1"}, f"{vis_between(5, 18)}")
    s1 = first_change(True, 18, 25)
    check("1. con inactividad = umbral (20 s) se duerme", s1 is not None, f"{s1}")
    check("1b. durmiendo la ventana SIGUE visible (sueño no es suspensión)", vis_between(s1 + 1.5, 43) == {"1"} if s1 else False, f"{vis_between(30, 43)}")
    # mientras duerme el sondeo ligero cuenta despertares
    n_wake = sum(1 for t, lab in wakes if lab == "durmiendo" and 27 <= rel(t) <= 44)
    check("1c. durmiendo: sondeo ligero (~1 despertar cada 2 s)", 6 <= n_wake <= 12, f"{n_wake} despertares en 17 s")
    w1 = first_change(False, 45, 49)
    check("2. actividad a los 45 s: despierta en ≤ 4 s", w1 is not None, f"{w1}")
    check("2b. despierta con la expresión «surprised»", "Surprised" in expr_after(44.5, 5), f"{expr_after(44.5, 5)}")
    s2 = first_change(True, 62, 71)
    check("3. 20 s después de esa actividad vuelve a dormirse", s2 is not None, f"{s2}")
    # interruptor en vivo
    off = first_change(False, 74, 78)
    check("4. «Sueño» apagado a los 75 s con ella dormida: despierta al instante", off is not None, f"{off}")
    check("4b. y no vuelve a dormirse con el interruptor apagado (77–84 s)", always(False, 78, 84), "")
    check("4c. encenderlo a los 85 s NO la duerme de golpe (la acción cuenta como actividad reciente)", always(False, 86, 92), "")
    s3 = first_change(True, 90, 100)
    check("4d. …y se duerme cuando pasa el umbral desde esa acción", s3 is not None, f"{s3}")
    w2 = first_change(False, 100, 104)
    check("5. actividad a los 100 s: despierta", w2 is not None, f"{w2}")
    # bloqueo
    l1 = first_change(True, 104.5, 108)
    check("6. bloqueo a los 105 s: duerme AL INSTANTE (inactividad de solo ~5 s)", l1 is not None and l1 <= 107, f"{l1}")
    check("6b. bloqueada no se sondea la inactividad (ningún despertar «durmiendo» salvo el paso sleepy→resting)", sum(1 for t, lab in wakes if lab == "durmiendo" and 110 <= rel(t) <= 124) == 0, "sin contar el paso sleepy→resting (~4 s tras dormirse)")
    u1 = first_change(False, 124, 128)
    check("7. desbloqueo a los 125 s: despierta", u1 is not None, f"{u1}")
    check("7b. …con la sorpresa", "Surprised" in expr_after(124.5, 5), f"{expr_after(124.5, 5)}")
    check("7c. y NO vuelve a dormirse enseguida (la lectura aún dice 25 s): despierta 128–142 s", always(False, 128, 142), "")
    s4 = first_change(True, 142, 150)
    check("7d. se duerme un umbral después del desbloqueo", s4 is not None, f"{s4}")
    # ocultar / mostrar dormida
    hid = next((rel(t) for t, _b, _e, _p, h, _s, _m in views if h and 149 <= rel(t) <= 153), None)
    check("8. oculta desde la bandeja a los 150 s estando dormida", hid is not None and sleeping_at(154), f"{hid}")
    check("8b. oculta: ningún despertar del planificador por sondeo (sin consultas)", sum(1 for t, lab in wakes if lab == "oculta" and 152 <= rel(t) <= 159) == 0, "")
    check("8c. la ventana está oculta", vis_between(152, 159) == {"0"}, f"{vis_between(152, 159)}")
    shown = first_change(False, 159, 163)
    check("9. mostrar a los 160 s termina el sueño al instante (sin esperar al sondeo: no se consultaba)", shown is not None, f"{shown}")
    check("9b. …visible y con sorpresa", vis_between(162, 170) == {"1"} and "Surprised" in expr_after(159.5, 5), f"{vis_between(162, 170)} {expr_after(159.5, 5)}")
    s5 = first_change(True, 176, 186)
    check("9c. un umbral después de esa acción vuelve a dormirse", s5 is not None, f"{s5}")
    # cambio de umbral en vivo estando dormida
    w3 = first_change(False, 184, 189)
    check("10. subir el tiempo a 30 min a los 185 s estando dormida: despierta al instante", w3 is not None, f"{w3}")
    check("10b. y sigue despierta hasta el final (umbral de 30 min)", always(False, 190, 200), "")
    # sin mensajes
    msg_asleep = [(round(rel(t), 1), m) for t, _b, _e, _p, _h, s, m in views if s and m.startswith("Some")]
    check("11. durmiendo no hay NUNCA un mensaje", not msg_asleep, f"{msg_asleep}")
    msg_surprise = [(round(rel(t), 1), m) for t, _b, e, _p, _h, _s, m in views if e == "Surprised" and m.startswith("Some")]
    check("11b. la sorpresa de despertar no lleva mensaje", not msg_surprise, f"{msg_surprise}")
    check("12. el proceso sigue vivo hasta el final", not json.load(open(prefix + "-meta.json", encoding="utf-8"))["exited_early"])

    width = max(len(n) for n, _, _ in results)
    bad = 0
    for n, ok, d in results:
        print(f"{'OK ' if ok else 'FALLA'}  {n:<{width}}  {d}")
        bad += not ok
    print(f"\n{len(results) - bad}/{len(results)} comprobaciones OK")
    print("transiciones de sueño (s desde la 1ª lectura):", [(round(t, 1), s) for t, s in sleep_changes])
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
