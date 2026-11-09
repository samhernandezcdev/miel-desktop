# E0b: arnés de medición del costo de animación (SOLO medición; no es parte del producto)

Mide CPU por proceso (miel.exe + árbol WebView2), Private Bytes y GPU Engine con variantes CSS **de prueba** sin tocar el producto ni los SVG.

1. Compilar el binario de diagnóstico **sin** `custom-protocol` (carga `devUrl`, `http://localhost:1420`) fuera del repositorio:
   `cargo build --release --locked --features diag --target-dir <dir-fuera-del-repo>` (en `src-tauri`).
2. Generar variantes y plan intercalado: `python -I tools/e0b/variants.py <work> 5` (y `mk_round2..5.py <work> <work-base>`); copiar `dist/` y `server.py` a `<work>`.
3. Ejecutar: `tools/e0b/run.ps1 -Work <work> -Exe <miel.exe diag> -Dist <work>\dist -Out docs\measurements\v020-e0b-<nombre> -IdleFrom 10 -IdleTo 30 -SwitchMs 32000 -RestFrom 40 -RestTo 60`
   (`real.ps1` mide estados reales sin fijar `data-state`). Respalda y restaura `settings.json` (SHA-256) y no mueve el cursor ni genera entradas.
4. Resumir: `python -I -X utf8 tools/e0b/analyze.py <prefijo>` y `tables.py`.

`server.py` sirve una copia de `dist/` con el CSS de la variante activa y un script que fija `data-state` (`idle`/`resting`); solo escucha en `127.0.0.1:1420`. Resultados y conclusiones: `docs/plan/v0.2.0-baseline.md` (§E0b).

## Fase 2 (binarios de producción)
`prodrun.ps1` compara varios `.exe` de producción (release + `custom-protocol`; con `diag` solo para el guion de pausa/ocultar y el registro de estados) con
repeticiones intercaladas y muestreo de 1 s; `analyze_prod.py <prefijo>` agrega por estado del motor. Resultados: `docs/plan/v0.2.0-baseline.md` §8.
