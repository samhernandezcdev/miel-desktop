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
