# Rendimiento de Miel Desktop

> # ⛔ CIFRAS INVALIDADAS: NO CITAR NI PRESENTAR COMO ACTUALES
>
> **Ninguna cifra de CPU, memoria ni "tamaño" de este documento describe la versión que se publica.** Se midieron el
> 2026-10-31 con una compilación de producción en la que la CSP **bloqueaba el CSS del rig**: no había **ninguna animación CSS**
> (aleteo, flotación, parpadeo) y se apilaban los 8 rostros. Esa compilación **no es** Miel v0.1.0. Además, el instalador
> y `miel.exe` de las tablas de tamaño ya no son los vigentes.
>
> **Pendiente:** repetir todas las mediciones con el instalador definitivo. Hasta entonces el proyecto **no afirma** ningún
> consumo de CPU ni de memoria (el README, las notas de release y el ROADMAP no incluyen cifras). Lo que se conserva aquí es
> la **metodología** y los datos en bruto, solo como registro histórico.

*Texto original (histórico):* las cifras de este documento son **mediciones reales** realizadas el 2026-10-31 en el equipo de
desarrollo (Windows 11 Pro 10.0.26200, 16 procesadores lógicos, WebView2 156.0.4314.50), **con la build afectada**. **No son
garantías**: otras máquinas, controladores de GPU, escalado de pantalla o versiones de WebView2 darán
valores distintos. Lo que no se midió se indica como *pendiente*.

> ⚠️ **Aviso (2026-10-31, tras la corrección de la CSP): estas mediciones están AFECTADAS.** Se tomaron con una
> compilación de producción en la que el `<style>` del rig quedaba **bloqueado por la CSP**: los 8 rostros
> apilados y **sin ninguna animación CSS** (aleteo, flotación, parpadeo). Por tanto la CPU de WebView2 en
> Idle/Follow/Rest/mensaje **está subestimada** y no representa a la versión corregida. Los resultados de
> Paused/Hidden y el árbol de procesos son cualitativamente aplicables, pero **todas las cifras deben repetirse
> con el binario corregido** (pendiente). Las conclusiones sobre `miel.exe` (motor Rust) no dependen de las
> animaciones, pero también se repetirán.

## Metodología

- **Binario medido**: compilación de **producción** (`--release`, `opt-level = "s"`, LTO, `panic = abort`,
  frontend embebido con `tauri/custom-protocol`) más la característica de diagnóstico `diag`
  (`cargo build --release --features "diag,tauri/custom-protocol" --target-dir src-tauri/target-diag`).
  `diag` solo añade registros con marca de tiempo y el guion `MIEL_DIAG_SCRIPT`; **el instalador no la incluye**.
  La diferencia de rendimiento entre ambos es despreciable (los registros ocurren solo en cambios de estado).
- **Procesos contados**: `tools/measure.ps1` identifica el **árbol** de procesos de Miel (`miel.exe` y sus
  descendientes por PPID). **No** se suman los `msedgewebview2.exe` ajenos del equipo (había decenas de
  otras aplicaciones).
- **Métricas por proceso y muestra**: CPU acumulada (→ % de **un** núcleo entre muestras), *Working Set* y
  *Private Bytes* (memoria comprometida privada).
- **Clasificación por estado**: cada muestra se asigna al estado vigente según el registro del motor
  (`tools/analyze_measure.py`): Idle, Follow, Rest, Reaction, "+mensaje" (burbuja visible), Paused, Hidden.
  Las muestras de 5 s pueden contener transiciones; por eso hay varianza.
- **Sin interferencias**: no se movió el cursor ni se usó la interfaz durante las mediciones; el equipo
  estaba sin otras cargas de compilación. El cursor del usuario sí pudo moverse libremente durante Follow.
- **Caveat de memoria**: sumar *Working Sets* **cuenta varias veces** las páginas compartidas entre los
  procesos WebView2 (DLL de Chromium, etc.). La suma **no** equivale a la memoria física exclusiva de
  Miel. *Private Bytes* es la cifra más adecuada para detectar fugas, pero tampoco es "RAM física".
  Para una cifra de memoria física real haría falta, p. ej., `Working Set - Shareable` por proceso (pendiente).

Reproducir:

```powershell
cargo build --release --features "diag,tauri/custom-protocol" --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target-diag
$env:MIEL_FAST='1'; $env:MIEL_DIAG_SCRIPT='pause@120,resume@180,hide@240,show@300'
.\tools\measure.ps1 -Exe .\src-tauri\target-diag\release\miel.exe -Seconds 575 -IntervalSec 5 -Out .\docs\measurements\fast
python -I tools/analyze_measure.py docs/measurements/fast
```

Datos en bruto: `docs/measurements/` (`*-samples.csv` agregado por muestra; `*-procs.csv` por proceso;
`*-engine.log` registro del motor).

## Resultados (INVALIDADOS: medidos con la build sin animaciones CSS; solo histórico)

### Tamaño del producto (instaladores anteriores, ya no vigentes)

| Elemento | Tamaño |
|---|---|
| Instalador NSIS `Miel_0.1.0_x64-setup.exe` | 1 259 051 bytes (1,20 MiB) |
| `miel.exe` (release, LTO, sin símbolos) | 3 356 160 bytes (3,2 MiB) |

### Árbol de procesos

7 procesos en todo momento: `miel.exe` + **6 procesos de WebView2** (navegador, GPU, utilidades,
renderizador…). El número **no cambió** en ningún escenario, incluidos 10 ciclos ocultar/mostrar. El
motor de Miel es **un único hilo** (`miel-engine`) más un hilo opcional de diagnóstico que no existe en el instalador.

### Escenarios — prueba de 10 min con tiempos acelerados (`MIEL_FAST`) y guion

(113 muestras cada 5 s; guion: pausa, reanudación, ocultar, mostrar, pausa+ocultar, y 10 ciclos ocultar/mostrar.)

| Estado | n | CPU total % de 1 núcleo (media / mediana / máx) | `miel.exe` CPU % | WebView2 CPU % | Private total (MB) |
|---|---|---|---|---|---|
| Idle | 9 | 0,44 / 0,31 / 0,92 | 0,20 | 0,24 | 161,0 |
| Idle + mensaje visible | 8 | 1,41 / 1,23 / 2,44 | 0,61 | 0,80 | 167,9 |
| Follow | 19 | 1,71 / 1,23 / 5,21 | 1,13 | 0,55 | 162,0 |
| Rest | 9 | 1,50 / 0,62 / 5,53 | 0,99 | 0,51 | 161,5 |
| Reaction | 9 | 0,61 / 0,31 / 1,84 | 0,07 | 0,55 | 165,5 |
| **Paused** | 28 | **0,08 / 0,00 / 0,92** | 0,01 | 0,07 | 161,6 |
| **Hidden** | 17 | **0,13 / 0,00 / 0,61** | 0,02 | 0,11 | 161,1 |
| Hidden + Paused | 8 | 0,04 / 0,00 / 0,31 | 0,00 | 0,04 | 162,8 |

Lecturas (con cautela, son muestras de 5 s con transiciones):

- ~~**Paused y Hidden reducen efectivamente la actividad**: ≈ 0,0–0,1 % de un núcleo, frente a 0,4 % en Idle.~~ **No concluyente**: sin animaciones CSS no puede compararse con la versión corregida (donde Paused/Hidden deben pausarlas); pendiente de repetir.
- **Follow/Rest** (≈ 1,5 %) es el estado más caro: `miel.exe` reposiciona la ventana ≈ 30 veces por segundo
  mientras se mueve; es el coste de mover una ventana nativa, no de animar.
- **Mensaje visible** casi triplica el coste de Idle (burbuja + fundido + cambio de cara); es transitorio (4–6 s, y con cooldown de 12 min por defecto).
- **Memoria**: `miel.exe` ≈ **5,9 MB** de Private Bytes; el resto (≈ 155 MB) son los 6 procesos de WebView2.
  Con mensaje visible sube ≈ 6 MB (cambia mientras el WebView compone la burbuja) y baja al terminar.
- **Sin deriva visible**: Private total 162,7 → 162,2 MB en 9,5 min, incluidos 10 ciclos ocultar/mostrar
  (163,7 MB antes de los ciclos → 162,7 MB después). *Working Set* sumado 376 → 392 MB (la suma incluye
  páginas compartidas y varía con el sistema; no es una fuga demostrada ni descartada con esta ventana corta).

### Prueba de 30 minutos con tiempos reales (sin aceleración ni guion)

179 muestras cada 10 s durante 29,9 min (tiempos y pesos **por defecto**; sin `MIEL_FAST` ni guion). Durante
la prueba se produjeron 87 cambios de vista (23 de Follow/Reaction, 3 con mensaje visible: el cooldown de
12 min limita los mensajes). Nadie tocó la interfaz.

| Estado | n | CPU total % de 1 núcleo (media / mediana / máx) | `miel.exe` CPU % | WebView2 CPU % | Private total (MB) |
|---|---|---|---|---|---|
| Idle | 73 | 0,25 / 0,15 / 2,17 | 0,21 | 0,04 | 159,0 |
| Rest | 87 | 0,39 / 0,15 / 5,42 | 0,31 | 0,08 | 158,5 |
| Follow | 11 | 1,37 / 1,39 / 3,25 | 1,00 | 0,37 | 159,7 |
| Reaction | 5 | 0,25 / 0,15 / 0,47 | 0,12 | 0,12 | 158,7 |
| Follow + mensaje | 1 | 2,64 (una sola muestra) | 1,24 | 1,40 | 165,4 |
| **Todo el período** | 179 | **0,40 / 0,15 / 5,42** | | | |

- **Procesos**: 7 constantes (`miel.exe` + 6 WebView2) durante los 30 min.
- **Memoria (Private Bytes, suma de los 7 procesos)**: media de los primeros 5 min **157,8 MB** → últimos
  5 min **159,6 MB** (rango 155,0–165,4 MB); `miel.exe` ≈ 5,7 MB, WebView2 ≈ 153 MB.
  *Working Set* sumado 354 → 364 MB (incluye páginas compartidas).
- **Tendencia**: regresión lineal de +0,10 MB/min sobre Private Bytes (+1,8 MB en 30 min). **Es una
  tendencia leve y positiva que esta ventana no permite calificar como fuga ni descartarla**: Chromium/WebView2
  mantiene cachés que crecen y se liberan. Extrapolar linealmente a 8 h (+≈50 MB) sería especulativo; **la
  prueba de 8 h sigue pendiente**.
- **CPU**: mediana 0,15 % de un núcleo; los picos (≈ 5 %) coinciden con arranques de movimiento. Con
  tiempos reales Miel pasa la mayor parte del tiempo en Idle/Rest (≈ 90 % de las muestras).
- Los 104 registros "relectura de monitores" en 30 min ocurren al despertar el motor o al moverse
  (cada ≥ 2 s en movimiento): **no hay polling en reposo**.

## Qué NO se midió (pendiente)

- Ejecución prolongada de **8 horas** (lo prometido para una "jornada completa").
- Memoria física exclusiva (Working Set privado vs compartido) y consumo de **GPU** y de **energía**.
- Otras máquinas, escalado de pantalla ≠ 100 %, varios monitores, equipos sin GPU dedicada.
- Comparación con y sin la burbuja a distintos tamaños (64/128 px).

## Optimizaciones: decisiones y su fundamento

Se investigó antes de cambiar nada. **No se aplicó ninguna optimización nueva** en esta etapa porque las
mediciones no la justifican:

| Candidato | Evidencia | Decisión |
|---|---|---|
| Frecuencia del motor (30 Hz en Follow) | `miel.exe` ≈ 1,1 % de un núcleo solo durante Follow/Rest | Se mantiene; bajar la frecuencia degradaría la suavidad. |
| Polling en reposo | Idle/Paused/Hidden: el hilo del motor duerme en `Condvar` (CPU de `miel.exe` ≈ 0 en Paused/Hidden). Probado con tests (0 llamadas al cursor/monitores en reposo). | Nada que optimizar. |
| Animaciones CSS en Idle | ~~WebView2 ≈ 0,24 % de un núcleo en Idle con 8 animaciones del rig~~ **INVALIDADO**: esa build no ejecutaba las animaciones CSS; el coste real está sin medir | Se mantienen (diseño original); **medir con la build corregida** antes de afirmar nada. |
| Re-renderizados de React | Solo cambia el estado al recibir un evento de Rust (no por frame) | Nada que optimizar. |
| IPC | Un evento por cambio de estado; 0 IPC por frame | Nada que optimizar. |
| Procesos de WebView2 | 6 procesos fijos (arquitectura de WebView2) | No controlable desde Miel. |

Si en equipos con GPU modestas el coste de composición del WebView resultase alto, la palanca real sería
reducir/pausar animaciones en Rest (ya se pausan en Paused/Hidden). No se hizo sin evidencia.
