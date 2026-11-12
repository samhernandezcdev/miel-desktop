# Miel Desktop — Registro de pruebas (v0.1.0)

Convención: **AUTO** = prueba automatizada repetible · **VIVO** = observado en la app real **sin intervención
humana y sin usar la interfaz del usuario** (capturas de pantalla, estilos Win32, registros del motor, mensajes
`WM_CLOSE` enviados a la propia ventana de Miel) · **MANUAL** = requiere una persona.
Estados: **Completada**, **Fallida**, **Pendiente**, **No aplicable**. No se marca nada como completado sin
haberlo ejecutado u observado. No se usa el cursor, el teclado, la resolución ni el menú nativo del usuario.

## Comandos

```powershell
npx tsc --noEmit
npm run build
npm test                                                   # vitest
cargo check  --manifest-path src-tauri/Cargo.toml
cargo test   --manifest-path src-tauri/Cargo.toml          # unitarias + integración (runtime simulado de Tauri)
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
npm run tauri build                                        # producción + instalador NSIS
```

## Resultado de la batería final (Etapa 7) — AUTO

| Comprobación | Resultado |
|---|---|
| `tsc --noEmit`, `npm run build` | OK |
| `vitest` | **57 de 57** (7 archivos) |
| `cargo test` | **156 unitarias + 5 de integración**, estables en 5 ejecuciones consecutivas |
| `cargo clippy --all-targets` | 0 avisos |
| `npm run tauri build` | OK → instalador NSIS (ver abajo) |
| Recursos SVG originales | idénticos byte a byte (`cmp` contra `miel-bee-mascot/`) |
| Búsqueda de rutas personales / secretos / código de red en el repositorio | sin coincidencias |

No se eliminó ni debilitó ninguna prueba anterior: crecieron de 115+5 / 31 (Etapa 6) a 156+5 / 50.

## Instalador (artefacto de la Etapa 7) — SUPERADO

> Esta tabla describe un instalador **anterior**. El instalador a publicar se generará desde un clon limpio del código versionado
> (ver `docs/CLEAN_CLONE_VALIDATION.md`); el provisional del 2026-11-01 figura más abajo.

| Dato | Valor |
|---|---|
| Archivo | `Miel_0.1.0_x64-setup.exe` |
| Ruta | `src-tauri/target/release/bundle/nsis/` |
| Tamaño | 1 258 510 bytes (1,20 MiB) |
| SHA-256 | `4004a73cba3739b29b4e3f6ebe9b0c641649f27a1eef2f9b253ab78f24483470` **(SUPERADO: instalador anterior, sin `THIRD_PARTY_LICENSES.txt`; no es el que se publica)** |
| `miel.exe` | 3 355 648 bytes, SHA-256 `ae062b76aa89a4389c117b8af7e79cee420aef0f0dc20a946bec591565c7bab9` **(SUPERADO)** |
| Versión / arquitectura | 0.1.0 / x64 |
| Modo | por usuario (`currentUser`), sin privilegios de administrador |
| WebView2 | `downloadBootstrapper` (se usa el del sistema; no se incluye Chromium) |
| Firma | **sin firma** (SmartScreen puede advertir) |
| Identificador | **`lat.bkode.miel`** (definitivo) · `MANUFACTURER` NSIS = `bkode` (derivado) · producto `Miel` |

**El hecho de que el instalador se generara NO demuestra que funcione.** Instalación, actualización y
desinstalación están **PENDIENTES** (no se ejecutó el instalador para no modificar el equipo).

## Corrección de la regresión de CSP/estilos (rostros apilados en producción)

**Síntoma** (binario de producción/NSIS): los 8 rostros apilados a la vez, mejillas muy rojas, sin animación.

**Causa — CONFIRMADA** (con una precisión sobre la hipótesis): `style-src` **sí** tenía `'unsafe-inline'`, pero
Tauri añade un `'nonce-…'` y el navegador **ignora `'unsafe-inline'`** si hay nonce o hash. Evidencia, leída del
propio binario con el puerto de depuración de WebView2 (solo en el proceso de prueba):

| | Producción (antes) | Desarrollo | Producción (después) |
|---|---|---|---|
| CSP efectiva `style-src` | `'self' 'unsafe-inline' 'nonce-…'` | `devCsp` (sin nonce) | `'self' 'nonce-…'` |
| Violaciones de CSP | 2 ("Applying inline style … has been blocked") | 0 | **0** |
| `<style>` dentro del SVG | 1 (bloqueado) | 1 (aplicado) | **0** |
| Opacidad de los 8 rostros | todos = 1 | idle=1, resto 0 | idle=1, resto 0 |
| `.float` animación | `none` | `bee-float` | `bee-float` |

**Corrección** (sin `'unsafe-inline'`): `bee-rig.nostyle.svg` (derivado por `tools/make_nostyle_rig.py`, solo difiere
del original en la ausencia del `<style>`) + `bee.css` como archivo propio importado por `Bee.tsx`; se quitó el
`'unsafe-inline'` de `style-src` de producción (era inoperante). Ver `docs/ARCHITECTURE.md`.

**Verificación**
- AUTO — `vitest` +7 pruebas (57/57): el SVG renderizado no contiene `<style>` ni atributos `style`;
  `bee.css` (leído del disco) incluye los 8 selectores `.bee[data-state="…"] .face-…`, la regla `.bee .face{opacity:0`,
  los 9 `@keyframes` y `prefers-reduced-motion`; `bee.css` == `<style>` del rig original; `nostyle` == original sin
  `<style>`; `Bee.tsx` importa el rig sin estilo y el CSS; la CSP de producción no contiene `unsafe-inline`/`unsafe-eval`
  ni URLs externas; se conserva "sin remount" y los tamaños.
- AUTO — `tools/check_dist.mjs` (en `npm run build`): **falló** sobre el `dist` con la regresión (18 errores) y **pasa**
  sobre el corregido (CSS 6 560 B con los 8 estados y animaciones; JS sin CSS del rig).
- AUTO — `tools/make_nostyle_rig.py --check`; los 11 recursos originales de `src/assets/bee` siguen idénticos byte a byte.
- VIVO — **binario de producción** (`target/release/miel.exe`, vía CDP; capturas por estado en el equipo de desarrollo):
  para cada uno de los 8 estados, `data-state` correcto, **un único rostro visible y 7 con opacidad 0**, animaciones
  en ejecución (14–16), aleteo `bee-flap-l` activo y fotogramas distintos (≥ 694 píxeles de diferencia en 0,34 s);
  **0 violaciones de CSP**. Se comprobó además que `happy` tiene opacidad 1 en svg, escena y rostro.
  *Nota*: la diferencia de píxeles puede incluir cambios del fondo del escritorio; la señal principal es
  `document.getAnimations()` en ejecución. Las capturas **no** se incluyen en `docs/` porque muestran contenido del
  escritorio de la persona.
- **Pendiente (MANUAL)**: ver el resultado visual del **instalador** ya instalado (misma compilación) y repetir
  las mediciones de rendimiento (ver abajo).

**Consecuencia para otros resultados**: las mediciones de `docs/PERFORMANCE.md` se tomaron con la build afectada
(sin animaciones CSS) → **subestiman la CPU de WebView2** y deben repetirse. Las capturas de la Etapa 7 de
producción (p. ej. `docs/img/` de la ventana real en producción) que mostraban rostros apilados no se publicaron;
las de `docs/img/` proceden de compilaciones de desarrollo (con `devCsp`) y son válidas.

**Lección**: jsdom/Vitest y Edge headless no aplican la CSP final → una regresión así solo se ve ejecutando el
binario de producción. La Etapa 7 declaró "binario de producción: renderiza la abeja" por haber visto una captura
sin examinarla con cuidado (mostraba rostros apilados); **esa afirmación era incorrecta** y queda corregida aquí.

## Cierre de identidad de v0.1.0 (`com.example.miel` → `lat.bkode.miel`)

- **Cambio**: solo `identifier` en `src-tauri/tauri.conf.json` (nombre visible `Miel`, versión `0.1.0` intactos). El
  identificador no aparece en el código Rust ni en el frontend (se obtiene de la configuración de Tauri).
- **Batería tras el cambio — AUTO, todo OK**: `tsc`, `npm run build`, `vitest` 50/50, `cargo check`, `cargo test`
  156 unitarias + 5 de integración (3 ejecuciones), `clippy --all-targets` 0 avisos.
- **NSIS regenerado — comprobado en `installer.nsi`**: `BUNDLEID "lat.bkode.miel"`, `MANUFACTURER "bkode"`,
  `PRODUCTNAME "Miel"`, `VERSION "0.1.0"`, `INSTALLMODE "currentUser"`, directorio de instalación
  `$LOCALAPPDATA\Miel`, clave de desinstalación por nombre de producto. El desinstalador solo borra datos
  (`$APPDATA\lat.bkode.miel` y `$LOCALAPPDATA\lat.bkode.miel`) si se marca la casilla *Eliminar datos de la aplicación*.
- **VIVO (binario de producción, sin instalar)**: se crea `%LOCALAPPDATA%\lat.bkode.miel\EBWebView`; **no** se crea
  `settings.json` (sin cambios de preferencias); la 2.ª ejecución con la nueva identidad termina sola (1 proceso);
  `WM_CLOSE` cierra sin huérfanos. `%LOCALAPPDATA%\com.example.miel` (residuo de desarrollo) **sigue existiendo**:
  no se borró por no estar autorizado; `%APPDATA%\com.example.miel` ya no existe (se eliminó antes un archivo de
  prueba que yo mismo había generado) y no se migró ninguna preferencia.
- **Pendiente (MANUAL)**: instalar/desinstalar (incluida la casilla de datos), comprobar el *Publisher* `bkode` en
  *Aplicaciones instaladas*, y que una instalación posterior sobre otra existente la detecte como actualización.

## Etapa 7 — pruebas por subsistema

### Persistencia — AUTO (`settings.rs`) + VIVO
- AUTO: valores por defecto válidos; archivo ausente no escribe; ida y vuelta de todos los campos; escritura
  atómica sin `.tmp` residual; JSON inválido / no objeto / vacío → copia diagnóstica y valores por defecto
  (máx. 3 copias); campos con tipo erróneo → solo ese campo vuelve al defecto; rangos ajustados (velocidad,
  intervalo, tamaño al valor permitido más cercano); campos desconocidos ignorados; esquema futuro tolerado
  con copia previa; el almacén solo escribe si cambia algo (5 actualizaciones iguales = 0 escrituras);
  nunca persiste valores fuera de rango; supervivencia a un "reinicio"; error de escritura = `Err`, sin pánico.
- VIVO (variante `diag` de producción, 21 comprobaciones, **todas Completadas**): archivo corrupto → Miel
  arranca, copia diagnóstica creada, original intacto · pausa y seguimiento desactivado (por guion) se
  escriben a disco; sin campo `hidden`; sin `.tmp` · reinicio → arranca **pausada y sin seguimiento**, sin
  actividad durante 14 s · reanudar tras reiniciar guarda `paused=false` · ocultar **no** escribe preferencias
  y tras reiniciar Miel está visible · `pet_size` 64/128 → ventanas 260×168 y 260×232 con burbuja completa
  (capturas hechas en el equipo de desarrollo; **no se publican** porque incluían contenido del escritorio) · valores inválidos → campos restablecidos/ajustados.
  Se eliminó el directorio de configuración de prueba al terminar.

### Instancia única — AUTO + VIVO
- AUTO: una segunda ejecución solo pide *mostrar* (`Cmd::Hidden(false)`): no toca pausa ni seguimiento.
- VIVO (binario de producción, 2 ejecuciones): la segunda termina sola (código 0), **1 proceso `miel`**,
  la primera sigue visible; sin segunda ventana/motor.
- **MANUAL — Pendiente**: comprobar que sigue habiendo un solo icono de bandeja tras la segunda ejecución
  (se infiere: la segunda instancia termina antes de crear bandeja; no se observó el icono).

### Recuperación del frontend — AUTO
`ErrorBoundary`: renderiza hijos; ante un fallo no pinta nada e informa una vez; el informe no contiene rutas;
se recupera cuando llega una vista nueva; **no entra en bucle** (máx. 3 reintentos); sin cambio de vista no
reintenta. `errorReport`: compacta, nunca lanza, captura errores globales y promesas rechazadas y se
desinstala. Rust: saneo de rutas/saltos de línea, archivo ≤ 64 KB conservando lo reciente, tope de 25
informes por ejecución. **Pendiente (MANUAL)**: provocar un fallo real de renderizado en la app y ver el
archivo `frontend-errors.log`. **Límites** (documentados en `ARCHITECTURE.md`): no cubre errores de
manejadores/asíncronos (solo se informan) ni la caída de WebView2.

### Ciclo de vida y cierre — AUTO + VIVO
- AUTO: cierre con Follow activo, durante Reaction, con mensaje visible, pausada, oculta, y pausada+oculta
  con aviso de geometría pendiente; tras el cierre **0 actividad** del sistema; `request_stop` no bloquea;
  `wait_stopped` con plazo devuelve `false` si una llamada está atascada y `true` al liberarse; `shutdown`
  idempotente y desde 6–8 hilos simultáneos; `send` tras el cierre se descarta; la subclase Win32 se retira
  una sola vez (`remove`) o por `WM_NCDESTROY`, sin doble liberación (ventana de solo mensajes real).
- VIVO: `WM_CLOSE` (cierre del sistema) → proceso terminado en 0,03–0,04 s, código 0, 0 WebView2 huérfanos
  (binario de producción, dos veces). "Cerrar Miel" por bandeja: ver Etapa 6 (abajo).
- **Cierre forzado** (plazo de 2 s superado): no se hace `join` del hilo atascado y se registra `cierre
  forzado`; el hilo lo destruye el SO al terminar el proceso. **No se probó con un fallo real del SO**, solo
  con un `Host` simulado atascado (AUTO). No se afirma que el hilo terminara en ese caso.

### Geometría, DPI y tamaño — AUTO
Métricas por tamaño (64/96/128) en Rust y TypeScript con los mismos números (ventana 260×168/200/232; ancla a
112/128/144 px); espacio de burbuja constante; ancla descentrada con coordenadas negativas, escalas
100/125/150/200 %, área menor que la ventana; avisos de geometría agrupados; reubicación estando pausada u
oculta; al mostrar se revalida. **VIVO**: tamaños 64 y 128 con la ventana del tamaño esperado.
**Pendiente (MANUAL)**: cambios reales de resolución/DPI/monitores (no se alteró la configuración del equipo).

### Menú desde preferencias persistentes — AUTO
Motor arrancado desde preferencias (pausada, sin seguimiento): la vista refleja ambos, sin actividad ni
mensajes hasta reanudar; `menu_model`/`MenuSync` y los 5 tests con elementos reales de Tauri (runtime
simulado) cubren etiquetas, orden, casillas y texto Pausar/Reanudar para todas las combinaciones.

### Rendimiento — VIVO (ver `docs/PERFORMANCE.md`) — ⚠️ AFECTADO por la regresión de CSP: repetir
*(**INVALIDADO**: medido con la build sin animaciones CSS; no representa a v0.1.0. Pendiente de repetir con el instalador definitivo.)*
Producción + `diag`: 10 min con guion (113 muestras) y **30 min con tiempos reales** (179 muestras). 7
procesos constantes (`miel.exe` + 6 WebView2 propios). CPU de todo el período 0,40 % de un núcleo (mediana
0,15 %); Paused/Hidden ≈ 0,0–0,1 %; Private Bytes ≈ 158–160 MB (de los cuales `miel.exe` ≈ 5,7 MB); tendencia
+0,10 MB/min **no concluyente**. **Pendiente: prueba de 8 h**, memoria física exclusiva, GPU, energía.

### Producción — VIVO
~~Binario de producción (CSP estricta): renderiza la abeja~~ **Corregido**: la afirmación original de la Etapa 7 era
**incorrecta** (el binario mostraba los 8 rostros apilados y sin animación por la CSP; ver la sección de la
regresión). Lo que sí era cierto: ventana 260×200, click-through/no-activate/topmost y sin escribir en disco al
arrancar. El renderizado correcto en producción está verificado **solo tras la corrección** (8 estados, 0 violaciones de CSP).

## Etapa 6 (conservado) — bandeja y cierre

- AUTO: modelo del menú, `MenuSync`, `dispatch`, elementos reales (runtime simulado), motor
  (mostrar/ocultar idempotentes, pausa/ocultar independientes, alternancias en ráfaga, seguimiento en pausa),
  arranque (primer `show`), subclase Win32 real.
- VIVO (realizado con UI Automation sobre el menú nativo **antes** de que se decidiera no automatizarlo; no
  se repite): un único icono; Pausar/Ocultar/Mostrar conservan la pausa; Seguir cursor desactivado → 0 vistas
  Follow en 40 s; 5 ciclos ocultar/mostrar sin errores; "Cerrar Miel" → proceso terminado en < 0,1 s, subclase
  retirada una vez, motor unido, 0 huérfanos, icono retirado. Fallo de bandeja simulado → arranque abortado.

## Checklist final para Windows 11

| # | Comprobación | Estado | Evidencia / nota |
|---|---|---|---|
| 1 | Transparencia real | **Completada** | Confirmada por el usuario (Etapa 2, compilación de desarrollo); capturas de la ventana en producción sin fondo |
| 2 | Ausencia de marco o fondo | **Completada** | Ídem (usuario, Etapa 2) y capturas; *re-comprobar con el instalador*: Pendiente |
| 3 | Click-through con clic real | **Completada** (Etapa 2, usuario) · *con el instalador*: **Pendiente** | Estilos `TRANSPARENT` verificados en producción (VIVO) |
| 4 | Foco al escribir en VS Code | **Completada** (Etapa 2, usuario: no roba foco) · escritura real en VS Code con el instalador: **Pendiente** | `NOACTIVATE` verificado |
| 5 | Suavidad del movimiento | **Pendiente** | Requiere ojos humanos |
| 6 | Seguimiento del cursor | **Pendiente** (lógica AUTO + trayectorias VIVO de la Etapa 4) | Sin mover el cursor del usuario |
| 7 | Descanso | **Pendiente** visual · AUTO y captura `resting` Completadas | Captura hecha en el equipo de desarrollo, no publicada (contenía contenido del escritorio) |
| 8 | Ocho expresiones | **Completada** (desarrollo + **binario de producción corregido**, VIVO) · con el instalador instalado: **Pendiente** | 8×3 tamaños (headless) + CDP/capturas en producción, un único rostro y animaciones |
| 9 | Burbuja de diálogo | **Completada** | Capturas reales 64/96/128 sin recortes |
| 10 | Menú de bandeja | **Parcial**: funcional Completada (AUTO + VIVO Etapa 6) · **captura visual y clic real: Pendiente** | No se automatiza el menú nativo |
| 11 | Pausa y reanudación | **Completada** | AUTO + VIVO + persistencia |
| 12 | Mostrar y ocultar | **Completada** | AUTO + VIVO |
| 13 | Segunda ejecución | **Completada** (proceso) · un solo icono de bandeja: **Pendiente** | VIVO ×2 |
| 14 | Cierre durante movimiento | AUTO **Completada** · en vivo: **Pendiente** | |
| 15 | Cierre con burbuja visible | AUTO **Completada** · en vivo: **Pendiente** | |
| 16 | Monitores secundarios | **Pendiente** | El equipo tiene un 2.º monitor a la izquierda (x<0); no se probó el cruce |
| 17 | Cambios de DPI / resolución | **Pendiente** | AUTO + mensaje sintético; no se cambió la configuración |
| 18 | Instalación NSIS | **Pendiente** | No se ejecutó el instalador |
| 19 | Desinstalación | **Pendiente** | Ídem; comprobar también qué queda en `%APPDATA%`/`%LOCALAPPDATA%` |
| 20 | Actualización sobre una versión previa | **No aplicable** hasta existir una 0.1.1 con el identificador definitivo | |
| 21 | Ejecución sin conexión | **Pendiente** (verificación real sin red) | Auditoría estática: sin código de red; CSP solo `ipc:`/recursos propios |
| 22 | Consumo de memoria y CPU | **Pendiente de repetir** (10 + 30 min medidos con la build afectada por la CSP) | `docs/PERFORMANCE.md` (aviso) |
| 23 | Uso prolongado (8 h) | **Pendiente** | |
| 24 | Archivo de preferencias corrupto | **Completada** | VIVO + AUTO |
| 25 | Seguridad: capabilities/CSP/dependencias | **Completada** la auditoría estática · `cargo audit`: **Pendiente** (no instalado) | Ver informe |


## Preparación de publicación open source (Hito 0)

- Revisión de capturas: `size-64.png`, `size-128.png`, `msg-full.png` y `expr-RestResting.png` mostraban contenido del escritorio (barra de estado de un editor, texto de código y una ruta de sesión). Se **retiraron del repositorio** y se movieron fuera de él (`..\miel-desktop-private-captures`). `docs/img/` conserva solo `bubbles-dark.png` y `expr-Reaction.png`, renderizadas sobre fondo liso. **Pendiente**: tomar capturas reales de la ventana sobre un fondo neutro antes de la release.

**Validaciones de la preparación (2026-11-01, AUTO)** — sin cambios de código de la aplicación:

| Comprobación | Resultado |
|---|---|
| `npx tsc --noEmit` | OK |
| `vitest` | **57 de 57** (7 archivos) |
| `npm run build` (incluye `check:dist`) y `node tools/check_dist.mjs` | OK |
| `cargo test` | **156 unitarias + 5 de integración**, OK |
| `cargo clippy --all-targets -- -D warnings` | 0 avisos |
| `cargo metadata --locked`, `npm ci --dry-run` (requisitos de la CI) | OK |
| Sintaxis YAML de `.github/**` (cargada con PyYAML) | OK; **el workflow no se ha ejecutado** (no hay remoto) |
| SVG de `src/assets/bee/` frente a `miel-bee-mascot/assets/bee/` (`cmp`) | idénticos (`bee-rig.nostyle.svg` es el derivado documentado) |
| Instalador existente `Miel_0.1.0_x64-setup.exe` | **sin cambios**: SHA-256 `4004a73c…3470`, igual que arriba |
| `docs/THIRD_PARTY.md` regenerado | idéntico salvo una línea de cabecera nueva (enlace a `THIRD_PARTY_NOTES.md`); no se eliminó ningún aviso |
| Búsqueda de credenciales, rutas personales y datos del equipo en texto/código | sin coincidencias |

No se ejecutó: el workflow de GitHub Actions, la instalación/desinstalación, `cargo audit`/`npm audit`, ni ninguna prueba manual.

## Avisos de licencias de terceros y reconstrucción del instalador (2026-11-01)

**Qué se hizo** (sin tocar Git ni el identificador `lat.bkode.miel`): `THIRD_PARTY_LICENSES.txt` generado con
`tools/gen_third_party_licenses.py` (cargo-about 0.9.3 para Rust; archivos de licencia de `node_modules` para npm),
incluido como recurso del instalador (`bundle.resources` en `tauri.conf.json`) y check de CI (job `licenses`).
Cambios de configuración: `publish = false` en `src-tauri/Cargo.toml` (para que cargo-about no trate la crate propia
como tercero; `Cargo.lock` no cambió) y dos scripts npm (`licenses`, `licenses:check`).

| Comprobación | Resultado |
|---|---|
| Archivo generado | 209 crates de Rust (135 enlazadas según `cargo tree -e normal,no-proc-macro` + macros de compilación) y 4 paquetes npm de producción (`react`, `react-dom`, `scheduler`, `@tauri-apps/api`) |
| Cobertura | el propio script falla si alguna crate enlazada no aparece en la salida de cargo-about; **pasa** |
| Reproducibilidad | `--verify` (regenera en memoria y compara byte a byte): idéntico |
| Check de CI (`--check`) | pasa con los lockfiles actuales; **falla** (exit 1) simulando un `Cargo.lock` y un `package-lock.json` modificados (copias temporales); **no falla** con saltos de línea CRLF (checkout de Windows) |
| Crates MPL-2.0 (5) | `.crate` == checksum de `Cargo.lock`, código extraído == `.crate`, sin `[patch]`/`[replace]`/rutas en `Cargo.toml`: **sin modificar**; enlaces al código fuente en el archivo |
| `tsc`, `vitest`, `npm run build` (+ `check:dist`) | OK · vitest **57 de 57** |
| `cargo test --locked` | **156 unitarias + 5 de integración**, OK (igual que antes) |
| `cargo clippy --all-targets --locked -- -D warnings` | 0 avisos |

**Instalador reconstruido desde una copia limpia** (no un `git clone`: el repositorio aún no tiene commits y no se toca Git; se copiaron los
archivos del proyecto sin `node_modules`, `target`, `target-diag`, `dist`, `gen` ni `.git`, y se ejecutó `npm ci` + `npm run tauri build`; `check:dist` pasó dentro del build):

| Dato | Valor |
|---|---|
| Archivo | `Miel_0.1.0_x64-setup.exe`, 1 276 357 bytes (antes 1 258 510) |
| SHA-256 | `2d2be581475d6acbdbec6523ba10b3ab9cb65ae48958a6833d4b856efcbba4ed` |
| `miel.exe` | 3 355 648 bytes, SHA-256 `1dd1fd3a33b90fbad49e4f0aee5d015dbc2b0fca2b52af7ba63a927e559dd22b` |
| `THIRD_PARTY_LICENSES.txt` | SHA-256 `c412ceff56d6fcc192f5f84e75aa72342d1cf5d025dfa15816df7bd6fa26cff0` |
| Evidencia de que el recurso va en el instalador | `installer.nsi` generado contiene `File /a "/oname=THIRD_PARTY_LICENSES.txt" …` y `Delete "$INSTDIR\THIRD_PARTY_LICENSES.txt"` en la desinstalación; la copia en `target/release/` es idéntica (`cmp`) al archivo del repositorio |
| Ubicación | `C:\dev\miel-release-artifacts\v0.1.0-PROVISIONAL-no-publicar\` (fuera del repositorio) con `SHA256SUMS.txt`; **provisional: salió de una copia de archivos, no de un commit; no se publica** |

**NO verificado / pendiente (MANUAL):**
- No se abrió el `.exe` para listar su contenido (no hay 7-Zip) ni se **ejecutó** el instalador: que el archivo aparezca realmente en
  `%LOCALAPPDATA%\Miel` y se borre al desinstalar **está sin comprobar**.
- El hash anterior (`4004a73c…`) ya no es el del instalador a publicar. Las pruebas manuales de este documento que se dieron como
  **Pendiente** con el instalador anterior (instalación, actualización y desinstalación, click-through y foco con el instalador instalado,
  varios monitores, DPI, uso prolongado, burbuja de mensajes tras 5–6 min) **siguen pendientes y deben repetirse con este instalador nuevo**.
- El `miel.exe` nuevo tiene el mismo tamaño que el anterior pero distinto hash; las compilaciones no son reproducibles bit a bit (rutas, metadatos).

## v0.2.0 · E0: privacidad y DCO (2026-11-08)

Alcance: `docs/PRIVACY.md`, `tools/check_privacy.py` (+ `tools/privacy_policy.json`, `tools/privacy_baseline.json`), `tools/check_dco.py` y los trabajos `privacy` y `dco` de `.github/workflows/ci.yml`. Sin cambios en código de la aplicación ni en SVG.

| Comprobación | Resultado |
|---|---|
| `python -I tools/check_privacy.py` sobre el código actual | OK. **0 coincidencias** de símbolos prohibidos en `src-tauri/src/**` y `src/**`; capabilities y permisos de aplicación iguales a la lista aprobada; features de `windows` y `tauri` dentro de la lista; CSP sin hosts de red; las 23 API de `windows` usadas están en la lista permitida y de las 42 entradas de la lista, las 40 que no son de `core::` existen en `windows 0.62.2` (las 2 de `core::` no se comprueban); sin diferencias respecto a la línea base |
| `python -I tools/check_privacy.py --self-test` | OK: falla ante cada símbolo prohibido inyectado (Rust y JS), permisos nuevos, archivo de capabilities nuevo, feature de red, CSP remota, API de `windows` no aprobada o inexistente, crate o paquete npm nuevo; pasa con comentarios y nombres que solo contienen el símbolo como parte de otra palabra |
| Mutaciones del autotest (regex roto, sin quitar comentarios, línea base ciega, capabilities ciegas) | El autotest FALLA en las cuatro: el propio control está probado |
| `python -I tools/check_dco.py --self-test` | OK: 11 casos con y sin firma + historial simulado |
| `check_dco.py` sobre historial real (`6964b11..HEAD`) | Falla como se espera: `6964b11` y `9df4154` no llevan `Signed-off-by` (el control solo se aplica a pull requests nuevos) |
| `cargo test --locked`, `clippy`, `vitest`, `tsc`, `check:dist` | Ver el resumen de la entrega; sin cambios de código |
| **CI remota** (`privacy`, `dco`, y que `python` de `windows-latest` admita `tomllib`) | **Pendiente**: no se ha ejecutado en GitHub Actions |

### v0.2.0 · E0: conexiones de red de `miel.exe` en ejecución (2026-11-08) — solo lectura

**Qué se hizo.** Build de producción compilada ese día (`cargo build --release --locked --features tauri/custom-protocol` sobre `dist/` recién generado; `src-tauri/target/release/miel.exe`, 3 355 648 B), lanzada con los ajustes reales de la persona (respaldados antes; comprobados **idénticos byte a byte** con SHA-256 después). Durante 240 s se muestreó cada 5 s (43 muestras) el **árbol de procesos de ese `miel.exe`** (por PPID: `miel.exe` + los `msedgewebview2.exe` que cuelgan de él; 8 procesos) con `Get-NetTCPConnection` y `Get-NetUDPEndpoint` filtrados por PID. No se movió ni generó ninguna entrada; no se tocó ningún otro proceso `msedgewebview2` del equipo (los de otras aplicaciones se excluyen por no ser descendientes). Al terminar se detuvo solo ese árbol: **0 procesos `miel` restantes** y el `settings.json` original restaurado. Datos: `docs/measurements/v020-e0-net-connections.csv` y `v020-e0-net-procs.csv` (PID, protocolo, direcciones, estado; sin contenido ni capturas).

**Resultado (hechos).**
- **`miel.exe` (PID del proceso principal): 0 conexiones TCP y 0 puntos UDP en las 43 muestras.**
- Procesos `msedgewebview2.exe` hijos de Miel: sí hubo sockets TCP en el puerto 443 (IPv6) y un punto UDP local:
  - PID 25792: 2 conexiones `Established` hacia `2603:1036:311:1002::2:443`, entre t = 15 s y 110 s.
  - PID 29248: conexiones `Established` hacia `2620:1ec:33::11:443`, entre t = 63 s y 184 s, y un punto UDP local (`:::60711`).
  - Después de t = 184 s no se vio ningún socket.
- Las dos direcciones remotas pertenecen a **Microsoft Corporation (red `MSFT`, consulta RDAP de ARIN el mismo día)**. No se identificó el nombre del servidor (sin DNS inverso) ni la finalidad.

**Interpretación y límites (no extrapolar).**
- Es coherente con lo que `docs/PRIVACY.md` declara: el motor Rust de Miel no abre sockets y el tráfico observado lo originan los **procesos del runtime de WebView2** (componente de Microsoft con su propio comportamiento de red y de diagnóstico, fuera del control de Miel). Esta medición **no permite** distinguir si ese tráfico lo causa el runtime por sí mismo (actualización, diagnóstico, comprobaciones) o alguna acción de la página de Miel; el código de la interfaz no contiene `fetch`/`XMLHttpRequest`/`WebSocket` (comprobado por `tools/check_privacy.py`) y la CSP de producción solo permite `ipc:` y `http://ipc.localhost`, pero eso no explica por sí solo el tráfico del runtime.
- Es un **muestreo de instantáneas** cada 5 s: no ve conexiones más breves que el intervalo, ni el contenido, ni los nombres consultados por DNS (no se usó ETW ni captura de paquetes: exigirían privilegios y no estaban autorizados). Cada fila es el estado en ese instante.
- **Una sola ejecución de 4 minutos**, un equipo, una red (IPv6 activo), con la configuración real de la persona; no cubre arranque en frío del runtime, actualizaciones periódicas, otras redes ni sesiones largas. No se repitió con el runtime sin red ni con otra versión de WebView2.
- Las conexiones del runtime no son una violación de la política (que prohíbe red **en el código de Miel**), pero sí significan que **"Miel no genera tráfico de red" no puede afirmarse para el proceso en su conjunto**: la documentación solo afirma que Miel no añade conexiones por su cuenta.
- **Pendiente:** repetir tras instalar el instalador definitivo y con una sesión más larga si se quiere acotar el comportamiento del runtime.

## v0.2.0 · E0b Fase 2: capa de rendimiento de la animación (2026-11-08)

Cambios: `tools/build_bee.py` (+ copia en `miel-bee-mascot/tools/`), `src/assets/bee/bee.css` (capa añadida; también en `miel-bee-mascot/assets/bee/`), `tools/make_nostyle_rig.py`, `tools/check_dist.mjs`, pruebas (`Bee.test.tsx`, `src/test/checkDist.test.ts`), `tools/anim_compare/`, `tools/e0b/`, `docs/ANIMATION_PERFORMANCE.md`; los GIF de `docs/img/anim-compare/` no se versionan (carpeta ignorada; se regeneran con `tools/anim_compare/`). **Sin cambios de código de la aplicación ni de SVG.**

| Comprobación | Resultado |
|---|---|
| SHA-256 de los 18 SVG (`src/assets/bee/bee-*.svg` y `miel-bee-mascot/assets/bee/bee-*.svg`) antes/después | **Idénticos** (incluido `bee-rig.nostyle.svg`); regenerar con `python -I tools/build_bee.py` produce los SVG byte a byte iguales |
| `vitest` | 69/69 (antes 57: +4 de la capa en `Bee.test.tsx`, +8 en `checkDist.test.ts`; la prueba «bee.css es exactamente el `<style>`» se reformuló, no se eliminó) |
| `check:dist` | OK; falla (probado) si falta cualquiera de las reglas de la capa |
| Mediciones antes/después (binario de producción, 5 repeticiones intercaladas) | Ver `plan/v0.2.0-baseline.md` §8: Idle −51 %, Rest −70 % (20 fps) / −75 % (10 fps); Paused y Hidden ≈ 0 |

### Aprobación y valores fijados (2026-11-08)

Fase 2 aprobada con **Rest/Sleep a 20 fps** e **Idle con aleteo estándar a 30 fps** (defecto fijo del generador: `DEFAULT_REST_FPS = 20`, `DEFAULT_IDLE_FPS = 30`, `--idle-flap standard`; las demás opciones se conservan). Se regeneró `bee.css` con `python -I tools/build_bee.py`: resultado idéntico al ya presente; copia de `miel-bee-mascot/` igual; `make_nostyle_rig.py --check` OK. SHA-256 de los 19 SVG (`bee-*.svg` de `src/assets/bee/` y `miel-bee-mascot/assets/bee/`) comparados con `HEAD`: **idénticos**. Cifras que respaldan la elección (165 Hz, un equipo): Idle 5,30 % [4,58–6,18], Rest 2,98 % [2,76–3,69], Paused y Hidden ≈ 0.


## v0.2.0 · E1a: `Clock` y `SystemProbe` sin cambio de comportamiento (2026-11-09)

Cambios: `mascot/clock.rs` (trait `Clock`, `SystemClock`, `FakeClock`), `mascot/probe.rs` (trait `SystemProbe`, aritmética pura `idle_ms_from_ticks`, `notif_from_raw`, `FakeProbe`), `windows_probe.rs` (`WindowsProbe`, **no se invoca desde producción**), `mascot/engine.rs` (el reloj se inyecta: `Engine::spawn` crea un `SystemClock` y `spawn_with_clock` admite otro; `Instant` → milisegundos del reloj en `last_refresh`, `last_tick`, `geom_due`), `Cargo.toml` (features de `windows`: `Win32_System_SystemInformation`, `Win32_UI_Input_KeyboardAndMouse`). `tools/privacy_policy.json` **sin cambios**: ya listaba esas features y las API usadas (`GetLastInputInfo`, `LASTINPUTINFO`, `GetTickCount`, `SHQueryUserNotificationState`). Sin crates nuevas, sin cambios de SVG/CSS ni de `Cargo.lock`.

| Comprobación | Resultado |
|---|---|
| `cargo test` | 173 + 5 (antes 156 + 5): +17 nuevas (3 de `clock`, 10 de `probe`, 1 de `windows_probe`, 3 del motor con `FakeClock`). Ninguna aserción previa se modificó; en un test se adaptó solo el constructor (`Worker::new` recibe un reloj). 5 ejecuciones seguidas sin fallos |
| `vitest` / `tsc` / `check:dist` / `check_privacy.py` (+autotest) / clippy `-D warnings` | 69/69 / OK / OK / OK / sin avisos |
| Aritmética de `idle_ms` | probada: `dwTime` cerca de `0xFFFFFFFF` con tick envuelto, tick menor que `dwTime` (→ 0), límite exacto 2^31, fallo de la llamada (→ `Err`), saltos grandes (24 días), tope de 30 días |

Notas: (1) con contadores de 32 bits el delta válido nunca pasa de 2^31 ms (≈ 24,9 días), así que el tope de 30 días no puede alcanzarse con `u32`; se conserva como defensa y se prueba con entradas de 64 bits. (2) Cambio mínimo de resolución: el `dt` del movimiento pasa de precisión de nanosegundos a milisegundos (el reloj del motor es en ms); `Motion::step` ya limitaba y protegía `dt` pequeño. (3) Las esperas reales de la `Condvar` siguen siendo de tiempo de pared; con `FakeClock` el hilo se despierta por tiempo real pero decide con el reloj inyectado.

### Medición de no regresión (binario de producción con `custom-protocol` + `diag`, 165 Hz, mismo método que E0b)

`base` = `HEAD` (commit de E0c) compilado igual; `e1a` = árbol con E1a. Medias por repetición, mediana entre repeticiones y [rango]; 4 repeticiones intercaladas (`tools/e0b/prodrun.ps1`, `analyze_prod.py`). Datos: `docs/measurements/v020-e1a-prod-*`.

| Estado | `base` | `e1a` | Calibración A/A (`base` frente a copia idéntica `base2`) |
|---|---|---|---|
| Idle | 6,57 [5,72–7,12] | 7,06 [6,04–7,24] | 5,43 [5,05–6,07] frente a 5,98 [5,19–6,62] |
| Rest | 3,91 [3,09–4,65] | 4,28 [3,80–4,68] | 3,37 [2,95–3,60] frente a 3,03 [2,86–3,25] |
| Paused | 0,00 | 0,00 | 0,00 frente a 0,04 |
| Hidden | 0,00 | 0,00 | 0,04 frente a 0,04 |

Lectura: `e1a` sale +7 % (Idle) y +9 % (Rest) sobre `base` con el mismo signo en las 4 repeticiones, pero la calibración A/A con binarios **idénticos** da diferencias de −9 % a +10 % de signo opuesto: **la resolución de este método es ≈ ±10 %**, y la diferencia queda dentro de ella. Conclusión: no se detecta empeoramiento; no se puede afirmar una igualdad más fina. Los niveles absolutos varían entre sesiones (Idle de `base`: 6,57 en la tanda `e1a` frente a 5,43 en la A/A y 5,30 en E0b; la reacción, que E1a no toca, también subió ≈ 25 % en la tanda `e1a`): comparar solo dentro de una misma tanda. Paused y Hidden siguen en el suelo.

Incidencias: (a) una primera tanda se **descartó** (guardada como `v020-e1a-run1-descartada-*`): el equipo se suspendió durante la noche a mitad de la medición y una repetición `base` se cortó a los 20 s; (b) `tools/e0b/prodrun.ps1` se amplió para sembrar un `settings.json` temporal si no existe (Miel estaba desinstalada) y eliminarlo al terminar; al final, `settings.json` quedó ausente como al empezar; (c) el intento de evitar la suspensión con `SetThreadExecutionState` falló en la tanda `e1a` por un error de conversión de tipo (esa tanda terminó sin suspensiones de todos modos) y funcionó en la A/A; (d) los binarios de medición incluyen `diag` y se compilaron en un directorio temporal, ya eliminado. Sleep (`sleepy`) sigue sin figura propia (1–5 muestras por variante).


## v0.2.0 · E1b: infraestructura de supresión, sin detección automática (2026-11-10)

Cambios (sin comportamiento nuevo: en producción `suppressed` y `sleeping` son siempre `false`, y nada envía `Cmd::Suppressed`/`Cmd::DndOverride`):
- `mascot/state.rs`: banderas `suppressed`, `dnd_override` y `sleeping` en la FSM; funciones puras `suppression_active`, `visible`, `running` y `messages_allowed`; `is_suspended()` = `!running` (con `suppressed=false` es exactamente `paused || hidden`); `View` gana `suppressed` (supresión **en vigor**, es decir, sin anular) y `sleeping`, aditivos. `hidden`/`paused` conservan su significado. Mostrar con `hidden=true` y supresión en vigor quita `hidden` **y** anula el episodio; "Mostrar de todos modos" solo actúa con supresión en vigor. Dormir se presenta como Rest estable (`sleepy` → `resting`), congela (no descarta) los plazos propios y no consume el RNG.
- `mascot/engine.rs`: `Cmd::Suppressed(bool)` y `Cmd::DndOverride`; la visibilidad de la ventana y la cancelación de mensajes salen de `visible`/`messages_allowed`; `same_view` incluye los campos nuevos.
- `diag.rs`: `suppress`, `unsuppress` y `override` en `MIEL_DIAG_SCRIPT`. `host.rs`: el registro diag muestra los campos nuevos.
- `tray.rs`: `suppression_menu_model` (solo modelo, probado). `MenuModel`/`menu_model` y el menú real **no cambian**.
- `src/types/view.ts`: `suppressed?` y `sleeping?` opcionales (ausentes = `false`).

| Comprobación | Resultado |
|---|---|
| `cargo test` | 201 + 5 (antes 173 + 5): +28 nuevas (20 de `state`, 6 del motor, 1 de `tray`, 1 de `diag`). Aserciones previas sin modificar; solo se adaptaron los 4 constructores literales de `View` (`messages.rs`, `tray.rs`, `tests/tray_menu.rs`, test de serialización de `engine.rs`) con los dos campos nuevos |
| `vitest` | 71/71 (antes 69: +2 de compatibilidad de la vista de v0.1.0) |
| `tsc` / `check:dist` / `check_privacy.py` (+autotest) / `licenses:check` / clippy `-D warnings` | OK / OK / OK / OK / sin avisos |

Cubierto por pruebas de tabla: las 16 combinaciones `hidden × paused × suppressed × sleeping` (visible, corre, mensajes, expresión, `next_wakeup`, avance del reloj y recuperación con rebase de plazos), equivalencia de `is_suspended` con `paused || hidden` en todo el espacio, y los casos 1–9 de §5.2 (incluidos 8 y 8b). El caso 10 (errores consecutivos de la sonda) corresponde al detector de E4 y no existe todavía. Sin medición de CPU (no toca lo visual ni la temporización: `Fsm` solo añade ramas con las banderas a `false`).

## v0.2.0 · E2: ajustes de esquema 2 e IPC mínimo, sin ventana (2026-11-10)

Cambios (sin cambio visible: ningún interruptor nuevo tiene efecto todavía; la ventana de ajustes es E3):
- `settings.rs`: `SCHEMA_VERSION = 2`; campos `dnd_auto_enabled` (true), `sleep_enabled` (true), `sleep_after_minutes` (10; permitidos 5/10/15/30/60, D8; otro valor → el permitido más cercano), `proximity_enabled` (true), `time_of_day_enabled` (true). Migración 1→2 al cargar sin reescribir: copia fiel a `settings.v1.bak.json` con `create_new` (nunca sobrescribe una copia existente) y, si no pudo hacerse, se reintenta antes del primer guardado, que no sobrescribe el original si falla. `LoadOutcome::Migrated`. Archivo corrupto: valores por defecto y copia diagnóstica; el original no se borra al cargar.
- IPC: `get_settings`, `set_setting(key, value)` (lista cerrada de 10 claves, tipos estrictos, rangos saneados en Rust, devuelve las preferencias aplicadas) y evento `settings://changed` (solo a la ventana `settings`, solo si hubo cambio real). `paused`, `follow_enabled` y `schema_version` no son modificables por IPC.
- `capabilities/settings.json` (ventana `settings`: `core:event:allow-listen/unlisten`, `allow-get-settings`, `allow-set-setting`); `default.json` (`main`) sin cambios. `tools/privacy_policy.json` y el autotest de `check_privacy.py` actualizados (5 negativos nuevos). `.gitignore`: excepción para esa capability (el patrón `settings.json` la ocultaba) y `settings.v*.bak.json` ignorados.
- Las pruebas usan solo directorios temporales; ninguna toca el `settings.json` real (la carpeta de configuración de la persona ni existe en esta máquina).

| Comprobación | Resultado |
|---|---|
| `cargo test` | 222 + 5 (antes 201 + 5): +21 en `settings`; aserciones previas sin modificar (solo se amplió una aserción de `describe_outcome` en `lib.rs`: añadida, no cambiada) |
| `vitest` | 71/71 |
| clippy `-D warnings`, `tsc`, `check:dist`, `check_privacy.py` + autotest, `licenses:check` | OK |
| SVG | 19, sin diferencias con HEAD |

Compatibilidad hacia atrás (copia congelada del lector de v0.1.0 en un test): ante un archivo de esquema 2, v0.1.0 detecta esquema futuro (2), carga los campos comunes, copia el archivo a `settings.v2.bak.json` y, en su siguiente guardado, lo reescribe como esquema 1 sin los campos nuevos. Al volver a v0.2.0 los interruptores regresan a «activado»; lo apagado sigue recuperable a mano en `settings.v2.bak.json`.


## v0.2.0 · E3: ventana de ajustes (2026-11-10)

Cambios: `settings_window.rs` (ventana `settings` creada desde Rust, instancia única bajo candado, creación fuera del hilo principal con reintentos si la etiqueta aún no se liberó, destruida al cerrar; ventana normal: marco, enfocable, en la barra de tareas, **sin** «siempre encima»); `tray.rs` («Ajustes…» entre Pausar y Cerrar; `MenuModel.settings_enabled`, `MenuSink::set_settings_enabled`, `Control::open_settings`, `TrayAction::OpenSettings`); `lib.rs` (la carga de página y el cambio de escala solo cuentan para `main`; `apply_setting` compartido por el comando y por `diag`); `diag.rs` (`MIEL_DIAG_UI_SCRIPT`: `open_settings`, `close_settings`, `set:clave=valor`, con marca de tiempo Unix); frontend: segunda entrada `settings.html` + `src/settings/*` (React, CSS externo, es/en en `strings.ts`), `vite.config.ts` (multi-entrada), `tools/check_dist.mjs` (recorre ambas entradas). `capabilities/settings.json` **sin cambios** (la ventana real lo usó tal cual); `main` sigue sin acceso a `get_settings`/`set_setting`. Herramientas nuevas: `tools/e3/` (`verify_settings_window.mjs`, `winattrs.ps1`, `e3run.ps1`, `analyze_e3.py`). `tools/e0b/analyze_prod.py`: su expresión regular se actualizó porque E1b añadió `suppressed`/`sleeping` al registro de vistas (con los registros antiguos da las mismas cifras: Idle 6,57, Rest 3,91 de E1a).

| Comprobación | Resultado |
|---|---|
| `cargo test` | 224 + 5 + 5 (antes 222 + 5): +2 unitarias (`tray`, `diag`), +5 de `tests/settings_window.rs`; `tests/tray_menu.rs` mantiene sus 5. Las de ventana, repetidas 5 veces sin fallos |
| `vitest` | 118/118 (antes 71): +47 (ajustes: carga, rechazo, restauración, evento, idioma, accesibilidad; `check:dist` de dos entradas; textos es/en) |
| clippy `-D warnings`, `tsc`, `check:dist`, `check_privacy.py` + autotest, `licenses:check` | OK |
| SVG / CSS de la mascota | 19 SVG idénticos a HEAD (SHA-256 del conjunto `26b2d3ee415739f3`, igual que antes de E3); `bee.css` sin cambios |

Las aserciones existentes modificadas (autorizado) están listadas en el informe de la entrega; solo afectan a `tray.rs` y `tests/tray_menu.rs`.

### Verificación con la aplicación real (`tools/e3/verify_settings_window.mjs`; datos: `docs/measurements/v020-e3-verify.json`)
Binario de producción con `custom-protocol` + `diag`; WebView2 con puerto de depuración **local** solo durante la prueba; teclado **sintético dentro de la página** (CDP), sin tocar el teclado ni el cursor reales. **29/29 comprobaciones**: la ventana se abre desde Rust con las propiedades esperadas (marco, redimensionable, barra de tareas, sin `WS_EX_TOPMOST`, activable) y la mascota conserva click-through/sin foco/siempre encima; CSS externo aplicado, sin `<style>` ni `style=""`; **cero violaciones de CSP**, sin errores de consola y las 7 peticiones de la página son locales; **`settings://changed` llega a la ventana real** (Rust cambió el intervalo a 33 y el tamaño a 64 con `set:` y los controles cambiaron sin que la página llamara a `get_settings` ni a `set_setting`); segunda petición de apertura = `Focused` (una sola ventana); Tab recorre las 5 paradas con foco visible; flechas/Espacio/Enter aplican valores en Rust; un valor rechazado muestra aviso y restaura el control; fuera de rango (99999) Rust ajusta a 600 y se avisa; tema claro/oscuro, alto contraste y movimiento reducido responden (emulados); sin desborde horizontal en 360/520/900 px × escala 100/125/150/200 % × texto 100–200 % (**emulado** con CDP, no con la escala real de Windows); con el navegador en inglés la ventana sale en inglés; al cerrar la ventana **desaparece** (se destruye, sin ventana oculta), reabrir funciona (Rust: 2 creaciones, 1 enfoque) y la ventana reabierta muestra sus 7 controles. `settings.json` ausente como al empezar. Una primera ejecución falló dos comprobaciones: un desborde real a 360 px con texto al 200 % (corregido en `settings.css`) y un artefacto de temporización del propio script (comprobaba tras la reapertura; se reordenó).

**No cubierto automáticamente (manual, paso a paso):** (1) clic real en «Ajustes…» de la bandeja: botón derecho en el icono de Miel → «Ajustes…» → debe aparecer **una** ventana y enfocarse; un segundo clic debe traer la misma al frente, no abrir otra; (2) cerrar con la X y volver a abrir (3 veces seguidas, rápido); (3) escala real de Windows al 125 % y 150 % (*Configuración → Pantalla → Escala*) con la ventana abierta: texto legible, sin recortes; (4) lector de pantalla (Narrador): cada control se anuncia con su etiqueta y la ayuda. Lo automatizado llega a la ventana por `diag`, no por el menú nativo; el camino de la bandeja se cubre con pruebas del modelo, del despacho y de `open_in_background` sobre el runtime simulado.

### Medición: la ventana abierta y tras cerrarla (binario de producción `custom-protocol` + `diag`, 165 Hz, método de E1a/E0b)
Variantes intercaladas, 4 repeticiones de 230 s (`tools/e3/e3run.ps1`, `MIEL_FAST=1`): `e1a` = commit de E1a (`git archive 87eaf16` compilado igual, sin ventana), `e3ctl` = binario de E3 **sin abrir nunca** la ventana, `e3` = binario de E3 con la ventana abierta a los 65 s y cerrada a los 125 s. Fases: A (antes de abrir), B (abierta; se descartan los 10 s siguientes a abrir), C (tras cerrar; se descartan 10 s). Las variantes sin ventana se parten en las mismas franjas de tiempo. Datos: `docs/measurements/v020-e3-prod-*`; resumen: `v020-e3-prod-summary.md`.

| CPU (% de un núcleo; mediana de medias por repetición [rango]) | Fase | `e1a` | `e3ctl` | `e3` |
|---|---|---|---|---|
| Idle | A | 5,62 [4,90–7,42] | 6,36 [5,50–7,56] | 5,65 [5,19–6,58] |
| Idle | B | 6,06 [5,20–9,27] | 6,32 [5,54–9,81] | 6,38 [5,46–7,03] |
| Idle | C | 6,20 [5,07–6,95] | 6,29 [3,26–6,46] | **6,19** [4,00–6,98] |
| Rest | A | 4,04 [3,43–5,86] | 3,22 [2,57–4,13] | 3,28 [2,75–3,56] |
| Rest | B | 3,29 [2,91–3,42] | 3,72 [2,93–4,18] | 2,79 [2,11–4,64] |
| Rest | C | 2,80 [2,50–3,08] | 3,04 [2,52–4,08] | **3,65** [3,35–4,23] |

| Procesos / memoria (mediana de la fase) | A | B | C |
|---|---|---|---|
| `e3`: procesos | 7 | **8** | 7 |
| `e3`: Private MB | 179,9 | 212,5 | 183,4 |
| `e3`: Working set MB | 387,4 | 464,9 | 403,8 |
| `e3ctl`: procesos / Private / WS | 7 / 179,9 / 389,0 | 7 / 179,6 / 392,4 | 7 / 181,1 / 395,7 |
| `e1a`: procesos / Private / WS | 7 / 180,1 / 389,7 | 7 / 180,1 / 393,7 | 7 / 181,4 / 396,3 |

Lectura (sin ir más allá de los datos): (1) **procesos**: 7 → 8 con la ventana → 7 al cerrarla, en las 4 repeticiones. (2) **Memoria**: con la ventana abierta +32,6 MB privados y +77,5 MB de working set; tras cerrarla quedan +3,5 MB privados y +16,4 MB de working set sobre la fase A, frente a +1,2/+1,3 MB y +6,7/+6,6 MB que suben, en esas mismas franjas, `e3ctl` y `e1a` sin ventana: **un residuo de unos 2 MB privados y ~10 MB de working set por encima de la deriva de los controles**, que esta medición (una sola apertura por ejecución) no permite atribuir con certeza. (3) **CPU, Idle (el estado con animación continua)**: en C, `e3` 6,19 frente a `e1a` 6,20 (−0,2 %) y a `e3ctl` 6,29 (−1,6 %): dentro de ±10 %. (4) **CPU, Rest**: en C, `e3` 3,65 frente a `e1a` 2,80 (+30 %) y a `e3ctl` 3,04 (+20 %); **no se puede afirmar ±10 %** para Rest. En el mismo lote, `e1a`, que nunca tuvo ventana, baja de 4,04 (A) a 2,80 (C) (−31 %) y `e3ctl` oscila entre 2,52 y 4,08 por repetición en una misma fase: la variación entre repeticiones y entre franjas del mismo binario sin ventana es de ese orden, y los rangos de `e3` en C (3,35–4,23) y de `e3ctl` en C (2,52–4,08) se solapan. Ni se demuestra ni se descarta un efecto en Rest con 4 repeticiones; haría falta más muestra o un control más largo. (5) El método tiene una resolución de ≈ ±10 % (calibración A/A de E1a); los niveles absolutos de esta tanda difieren de los de E1a (Idle 5,6–6,4 frente a 6,6–7,1), por eso solo se comparan variantes dentro del mismo lote.

Límites: una sola apertura de la ventana por ejecución; 165 Hz; `MIEL_FAST=1`; el `diag` añade solo registro; la memoria de la verificación real (`v020-e3-verify.json`) se tomó 1,5 s tras arrancar y no es comparable con estas cifras de régimen estable. `settings.json` ausente al terminar, sin procesos de Miel sobrantes.

Incidencias de la medición: (a) en la primera ejecución del lote (`e1a`, repetición 1) el `settings.json` sembrado por `e3run.ps1` llevaba marca BOM (Windows PowerShell 5.1, `Set-Content -Encoding UTF8`), así que esa ejecución arrancó con valores por defecto tras registrar «archivo corrupto» (`follow_off@1` del guion y la exclusión de muestras con mensaje limitan el efecto; las demás ejecuciones leyeron el archivo que reescribe el propio Miel, sin BOM); (b) las variantes comparten la carpeta de configuración: tras cada ejecución de `e1a` (que solo conoce el esquema 1) el archivo vuelve a esquema 1, y las de `e3`/`e3ctl` lo migran al arrancar (`Migrated` con `settings.v1.bak.json` en los registros), de modo que sin proponérselo se ejercitó la migración y la copia con binarios reales; la carpeta temporal se elimina al terminar. (c) Los registros de `docs/measurements/v020-e3-*` se sanearon sustituyendo la ruta personal por `%APPDATA%`.

## v0.2.0 · E4a: refactor del bucle del motor + corrección de BOM (2026-11-10)

### Bloque A: corrección de BOM en `settings.json` (solo `src-tauri/src/settings.rs`)
Un `settings.json` que empieza con BOM UTF-8 se cargaba como corrupto (se descubrió en E3: la siembra de `e3run.ps1` con `Set-Content -Encoding UTF8` de Windows PowerShell 5 añade BOM). Ahora `load_from` ignora el BOM al leer: sin copia diagnóstica por esa causa y sin reescribir al cargar; `save_to` ya escribía sin BOM y ahora hay una prueba que lo exige. Pruebas nuevas (6): BOM + esquema 1 (migra igual que sin BOM; la copia v1 es fiel al original), BOM + esquema 2 (sin copias ni reescritura), BOM + contenido realmente corrupto (sigue corrupto, con copia fiel), BOM solo (igual que un archivo vacío: corrupto), guardado tras cargar un archivo con BOM (sin BOM, y relee igual). Nota: la copia `settings.v1.bak.json` es byte a byte del original, incluido su BOM.

### Bloque B: refactor puro del motor (sin cambio de comportamiento)
`next_deadline(&Worker, now) -> Option<Ms>` (mínimo absoluto de plazo de la FSM, fin de burbuja del hablante, cadencia de movimiento y *debounce*; `None` oculta/en pausa/suprimida sin anulación, salvo un *debounce* pendiente) y `Worker::tick(now, cmds)` (cuerpo del bucle: órdenes, `fsm.step`, hablante, emisión, *debounce*, movimiento). `run` solo espera y llama a `tick`; `wait_timeout` = `deadline − now`. Sin `Waiter` ni cambio de la espera de la `Condvar`, sin sondeos nuevos. Único matiz: la cadencia de movimiento pasó de `Duration` fraccionaria (33,3 ms) a ms enteros redondeados hacia arriba (34 ms), que es lo que la `Condvar` de Windows ya hacía (redondea hacia arriba), por lo que no se espera cambio de espera efectiva (no medido a nivel de milisegundo).

| Comprobación | Resultado |
|---|---|
| `cargo test` | 235 + 5 + 5 (antes 224 + 5 + 5): +6 de BOM, +5 del motor. Ninguna aserción existente modificada (el diff solo elimina líneas del cuerpo del bucle y de `wait_timeout`, movidas) |
| Pruebas nuevas del motor (sin hilos, `FakeClock`) | `next_deadline` es `None` oculta/pausada/suprimida y vuelve con anulación; solo cuenta el *debounce* mientras está suspendida; el mínimo cambia al avanzar el reloj (debounce → Idle → cadencia de 34 ms); en Follow vigila el cursor; durmiendo solo queda sleepy→resting (más el movimiento hacia el rincón); 10 minutos sin actividad se simulan en milisegundos, deterministas con la misma semilla |
| clippy `-D warnings` | OK |

**Equivalencia (diag, misma semilla).** Binarios de producción `diag` del commit de E3 (`git archive HEAD`, sin tocar el repositorio) y del árbol de trabajo, ambos con el mismo parche de solo-prueba `MIEL_SEED` (4 líneas bajo `DIAG` en `engine_config`; **no está en el repositorio**: se aplicó a las dos copias de `scratchpad`). `MIEL_FAST=1`, semilla 42, guion `pause@20,resume@28,hide@36,show@44,suppress@52,override@60,unsuppress@68,follow_off@76,follow_on@84,suppress@92,unsuppress@100`, 125 s, 3 repeticiones por binario intercaladas, cada una sin `settings.json` de partida (`tools/e3/eqrun.ps1`, comparación en `tools/e3/compare_eq.py`; registros en `docs/measurements/v020-e4a-equiv-logs/`). Se compara cada línea `vista v<N> …` (versión, comportamiento, expresión, pausa, oculta, suprimida, durmiendo, seguir, mensaje) y `diag:` sin la marca de tiempo: **34 eventos y 25 vistas en cada una de las 6 ejecuciones; las 15 parejas son idénticas** (3 e3–e3, 3 e4–e4, 9 e3–e4). Límite: el guion no ejercita `sleeping` (no existe orden de motor) ni el mensaje por reloj de pared más allá de lo que salga con esa semilla; la ventana de ajustes no se abrió.

**Medición** (método de E1a/E0b: producción + `diag`, `MIEL_FAST=1`, `follow_off@1`, 4 repeticiones de 230 s intercaladas, `tools/e3/e3run.ps1` sin abrir la ventana; datos `docs/measurements/v020-e4a-prod-*`). CPU en % de un núcleo, mediana de medias por repetición [rango]:

| Estado | `e3` | `e4a` | e4a vs e3 |
|---|---|---|---|
| Idle | 5,81 [5,50–7,71] | 5,18 [4,92–5,65] | −10,8 % |
| Rest | 3,09 [2,90–3,32] | 3,17 [3,06–3,40] | +2,6 % |

Lectura: Rest dentro de ±10 %. Idle queda 0,6 puntos por debajo (−10,8 %, en dirección de menos CPU); el rango de `e3` es amplio (una repetición a 7,71) y se solapa con el de `e4a`; sin esa repetición la diferencia es −7,4 % (Rest +7,8 %). Con 4 repeticiones y resolución de ≈ ±10 % no se afirma ni mejora ni empeoramiento. Incidencia: la primera ejecución de `e3` (`e3-r1`) arrancó con valores por defecto porque `e3run.ps1` sembró `settings.json` con BOM y el binario de E3 lo trató como corrupto (el defecto del Bloque A); el resto de ejecuciones releyeron el archivo ya reescrito sin BOM. Sus datos no se descartaron (mensajes activos, que se excluyen de Idle/Rest por etiqueta); la comparación sin esa repetición se da arriba.

## v0.2.0 · E4b: No molestar automático (2026-11-11)

### Preparación
- **Las pruebas de BOM sí pueden fallar.** Se quitó a mano (sin Git) la corrección de BOM de `src-tauri/src/settings.rs` y se ejecutaron las pruebas: fallan `bom_with_schema_1_loads_like_without_it`, `bom_with_schema_2_loads_clean_without_copies_or_rewrite` y `saving_never_writes_a_bom_even_after_loading_one_that_had_it` (ida y vuelta); las otras dos (`bom_alone…`, BOM con contenido corrupto) siguen en verde porque ese comportamiento ya era el correcto sin la corrección. Se restauró editando de vuelta: SHA-256 antes = después = `1e7a3ae4a3bb388b089129568f1ca684c2ef2138b027fd608b988e5f14e6b141` y las 44 pruebas de `settings` en verde.
- `tools/common.ps1` (`Write-Utf8NoBom`, `Clear-PersonalPaths*`): todo script que siembra `settings.json` lo escribe **sin BOM** (`e3run.ps1`, `eqrun.ps1`, `e0b/*`, `e4/*`) y sanea rutas en el origen.
- `MIEL_SEED` es oficial **solo con `diag`/`debug`** (como `MIEL_FAKE_HOUR`), con pruebas. Comprobado: `MIEL_SEED`, `MIEL_DIAG_NOTIF`, `MIEL_DIAG_SCRIPT` y `MIEL_FAKE_HOUR` aparecen 0 veces en el `miel.exe` de `cargo build --release --features tauri/custom-protocol`.
- `tools/check_svg_hashes.py` (SHA-256 por archivo de los 19 SVG contra `tools/svg_hashes.json`; ignora solo CRLF; `--self-test` negativo) y `tools/check_personal.py` (nombre de usuario y equipo leídos del entorno en ejecución, nunca escritos; rutas `C:\Users\<nombre>`, correos, IPv4 no loopback, IPv6 locales; revisa staging y archivos sin seguimiento; `--self-test`).

### Qué hace E4b
`mascot/dnd.rs` (detector puro: solo `QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN` y `QUNS_PRESENTATION_MODE` suprimen; 2 lecturas para entrar y 2 para salir; 3 errores seguidos terminan el episodio; un error nunca suprime y rompe las rachas). El motor sondea cada 2,5 s **solo** con `dnd_auto_enabled` y sin que la persona haya ocultado/pausado a Miel (también mientras está suprimida), dentro de `next_deadline` (mismo hilo y `Condvar`; sin hilo ni temporizador nuevos). La anulación («Mostrar de todos modos») vale por episodio y se borra al terminar; al terminar reaparece sin mensaje inmediato y los plazos congelados se reanudan. `set_setting("dnd_auto_enabled")` envía `Cmd::DndAuto` y actúa al instante (apagar termina el episodio y detiene el sondeo). UI: interruptor «No molestar automático» (es/en, «se aplica al instante»), frase de privacidad y elemento de bandeja «Mostrar de todos modos» solo con supresión y sin ocultación manual.

### Resultados automáticos
| Comprobación | Resultado |
|---|---|
| `cargo test` | 276 + 5 + 7 (antes 235 + 5 + 5); 1 prueba `ignored` (microbenchmark de la consulta) |
| clippy `-D warnings`, `tsc`, `check:dist` (2 entradas), `licenses:check` | OK |
| vitest | 126 (antes 118) |
| `check_privacy.py` + `--self-test` | OK (el autotest falla ante las entradas indebidas, incluida la nueva) |
| `check_svg_hashes.py` + `--self-test`, `check_personal.py` + `--self-test` | OK |
| Diff de SVG / CSS de la mascota | vacío |
| Coste de una consulta `SHQueryUserNotificationState` (prueba ignorada, `windows_probe.rs`) | ≈ 52 µs por llamada → cada 2,5 s ≈ 0,002 % de un núcleo |

### Aserciones existentes modificadas (autorizado)
- `src-tauri/src/tray.rs`: los dos literales `MenuModel{…}` de pruebas existentes ganan los campos `show_anyway`/`tooltip`; el `FakeSink` implementa los métodos nuevos de `MenuSink`; la prueba de modelo de supresión se **sustituyó** (antes esperaba que el menú no cambiara con la supresión; ahora aparece «Mostrar de todos modos»). Las demás se añadieron. `tests/tray_menu.rs`: solo import y 2 pruebas nuevas.
- vitest (consecuencia del interruptor nuevo): `SettingsApp.test.tsx` — «NO muestra…» (interruptores 2→3 y se retiró la palabra prohibida), controles 7→8, `h2` 4→5, `fakeRust` acepta booleano; `strings.test.ts` — lista de palabras prohibidas de la sección Privacidad.

### Verificación con la app REAL (sonda programada, sin tocar el sistema)
`MIEL_DIAG_NOTIF` inyecta la secuencia de estados en el motor real (`busy@15,ok@45,busy@60,ok@110,busy@125,ok@175`), `MIEL_DIAG_SCRIPT` (`override@75,hide@165,show@185`) y `MIEL_DIAG_UI_SCRIPT` (`set:dnd_auto_enabled=false@140,…=true@150`). Visibilidad de la ventana cada 0,5 s (`tools/e4/verify_dnd.ps1`, `analyze_dnd.py`; datos `docs/measurements/v020-e4b-verify*`): **22/22**: se oculta tras 2 lecturas y reaparece tras 2; «Mostrar de todos modos» la devuelve y la anulación no sobrevive al episodio; apagar el interruptor en vivo termina el episodio en ≤ 3 s y sin sondeo; encenderlo reanuda; ocultar a mano detiene el sondeo (el cambio `ok@175` no se observó hasta mostrar a los 185 s). **No verificado automáticamente:** el elemento nativo de la bandeja y su *tooltip* (el *runtime* simulado no los expone; solo se probó la estructura del menú) → matriz manual.

### Medición (`tools/e4/e4run.ps1`; `miel.exe` SOLO, sin WebView2; producción + `diag`, `MIEL_FAST=1`, `follow_off@1`, 4 repeticiones × 200 s intercaladas; datos `docs/measurements/v020-e4b-*`)
CPU en % de un núcleo, mediana de medias por repetición:

| Variante | Estado | CPU | Rango |
|---|---|---|---|
| `dnd_on` (sondeo activo) | Idle | 0,000 | 0,000–0,000 |
| `dnd_off` | Idle | 0,000 | 0,000–0,050 |
| `dnd_on` | Rest | 0,000 | 0,000–0,000 |
| `dnd_off` | Rest | 0,000 | 0,000–0,215 |
| suprimida por `diag` (sondeo real de recuperación) | fijo | 0,000 | 0,000–0,009 |
| oculta por la persona | fijo | 0,000 | 0,000–0,027 |
| pausada por la persona | fijo | 0,000 | 0,000–0,083 |

Lectura honesta: el proceso raíz queda por debajo de la resolución del contador de CPU de Windows (ticks de ~15,6 ms por muestra de 1 s ⇒ ≈ 0,1 %); **no hay diferencia medible** entre sondeo encendido y apagado, ni en suprimida, ni en oculta/pausada. Es coherente con el coste de la consulta (≈ 52 µs cada 2,5 s ≈ 0,002 %), pero **la medición no puede demostrar** que el coste sea cero ni distinguir diferencias menores de ≈ 0,1 %; el criterio ±10 % no es aplicable a cifras en 0,0 % y no se interpreta más. Los picos de 0,05–0,2 % en una repetición son ruido de un solo tick (aparecen también con el sondeo apagado). Límites: 165 Hz, `MIEL_FAST=1`, solo CPU del proceso raíz (no memoria ni WebView2), la «supresión» es la de `diag` pero el sondeo es la llamada real a Windows. `settings.json` ausente al terminar, sin procesos de Miel sobrantes.

### Desviaciones
1. «Mostrar de todos modos» solo aparece con `suppressed && !hidden` (según el encargo); el plan D7 lo permitía también ocultando: si la persona oculta, «Mostrar» ya existe.
2. No hay casilla de bandeja para el interruptor de No molestar (solo en ajustes).
3. Al reanudar tras ocultar/pausar a mano, el sondeo hace una lectura inmediata con rachas nuevas (2 lecturas de histéresis); no se recalcula antes de renderizar.
4. Se modificaron aserciones de vitest además de las de bandeja (listadas arriba).
5. `MIEL_SEED` y `MIEL_DIAG_NOTIF` solo existen con `diag`/`debug`.
6. Tooltip y elemento nativo de la bandeja: solo verificados a nivel de modelo/mock.
7. `e3run.ps1` sembraba con BOM (corregido).

### Matriz de pruebas MANUALES (pendiente de ejecutar por la persona; rellenar «Resultado»)
Preparación: ajustes → «No molestar automático» activado; Miel visible y sin ocultar/pausar a mano. Retraso esperado: ~5 s para ocultarse y ~5 s para reaparecer (2 lecturas cada 2,5 s).

| # | Caso | Esperado | Resultado |
|---|---|---|---|
| 1 | Juego en pantalla completa **exclusiva** (Direct3D) | Miel se oculta en ~5 s; al salir reaparece en ~5 s sin mensaje inmediato; «Mostrar de todos modos» aparece en la bandeja mientras dura | |
| 2 | Juego en ventana **sin bordes** | Puede NO ocultarse (Windows no lo marca como pantalla completa exclusiva); anotar el comportamiento. No es un fallo | |
| 3 | Vídeo del navegador con **F11** | Puede ocultarse (Windows lo trata como aplicación a pantalla completa); reaparece al salir | |
| 4 | **PowerPoint** en modo presentación | Se oculta; reaparece al terminar la presentación | |
| 5 | Pantalla completa en un **segundo monitor** con Miel en el primero | Windows informa del estado global, no por monitor: puede ocultarla aunque esté en otro monitor (esperado, documentado en PRIVACY); anotarlo | |
| 6 | **Ventana de ajustes abierta** durante una supresión | Miel se oculta pero la ventana de ajustes sigue abierta y utilizable; apagar el interruptor hace que Miel reaparezca al instante; encenderlo con la pantalla completa aún activa la vuelve a ocultar en ~5 s | |
| 7 | «Mostrar de todos modos» durante 1–4 | Miel reaparece y sigue visible hasta el fin del episodio; en el siguiente episodio vuelve a ocultarse | |
| 8 | Ocultar a Miel a mano antes de abrir el juego | No reaparece al terminar; sin sondeo mientras está oculta | |

### Comandos de verificación (desde la raíz del repositorio)
`cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings` · `npm test` · `npx tsc --noEmit` · `npm run check:dist` · `python -I tools/check_privacy.py` y `--self-test` · `npm run licenses:check` · `python -I tools/check_svg_hashes.py` y `--self-test` · `python -I tools/check_personal.py` y `--self-test` (esta, antes de cada commit).
