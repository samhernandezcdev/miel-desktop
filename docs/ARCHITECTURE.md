# Arquitectura de Miel Desktop

Miel es una aplicación **Tauri 2** (Rust + WebView2) con un frontend **React + TypeScript** que solo
*dibuja*. Toda la lógica de comportamiento vive en Rust, que es la **única fuente de verdad**.

```
┌────────────────────────── proceso miel.exe ──────────────────────────┐
│  Hilo principal (Tauri/tao)        Hilo "miel-engine" (1)             │
│  ├ ventana transparente            ├ FSM (Idle/Follow/Rest/Reaction)  │
│  ├ bandeja + menú (tray.rs)        ├ Talker (mensajes + cooldown)     │
│  ├ subclase Win32 (display_watch)  ├ movimiento (motion/geometry)     │
│  └ ciclo de vida (lifecycle.rs)    └ Condvar: duerme hasta un plazo   │
│            ▲   órdenes (Cmd) ───────────┘   │ vistas versionadas      │
└────────────┼────────────────────────────────┼────────────────────────┘
             │                                ▼  evento mascot://view
        WebView2 (React): MascotStage = <Bee/> (SVG inline) + <SpeechBubble/>
```

## Rust (`src-tauri/src`)

| Módulo | Responsabilidad |
|---|---|
| `lib.rs` | Arranque: instancia única → preferencias → ventana → motor → bandeja → aviso de pantalla → mostrar. Comandos IPC (`get_view`, `get_display_prefs`, `report_frontend_error`). |
| `settings.rs` | Preferencias JSON versionadas (carga tolerante, escritura atómica, copia diagnóstica). |
| `window.rs` | Click-through, no activable, tamaño según sprite (`metrics`), posición inicial en el área de trabajo. |
| `mascot/state.rs` | **FSM pura** y determinista: `Behavior` (qué hace) ≠ `Expression` (qué cara). `paused`/`hidden` ortogonales que congelan temporizadores. |
| `mascot/messages.rs` | Catálogo de frases, `Selector` (historial, categorías, `min_repeat`) y `Talker` (cooldown de 12 min, duración 4–6 s). |
| `mascot/motion.rs`, `geometry.rs` | Cinemática con dt real (velocidad/aceleración limitadas) y geometría en píxeles físicos (monitores, DPI, coordenadas negativas, ancla descentrada). |
| `mascot/engine.rs` | **Un único hilo** + `Condvar`. Une FSM, Talker y movimiento; sin polling en Idle/Rest/Paused/Hidden. |
| `mascot/host.rs` | Implementación de `Host` sobre Tauri (cursor, monitores, ventana, eventos). El motor se prueba con un `Host` falso. |
| `tray.rs` | Icono único y menú; `MenuSync` aplica solo lo que cambió; `dispatch` traduce acciones en órdenes. |
| `lifecycle.rs` | Primer `show` (bandeja lista + página cargada), cierre en dos fases, `fatal`. |
| `display_watch.rs` | Subclase Win32 (`WM_DISPLAYCHANGE`, `WM_DPICHANGED`, área de trabajo) → `Cmd::GeometryChanged`. |
| `errlog.rs` | Registro local, acotado y saneado de errores del frontend. |
| `diag.rs` | Solo diagnóstico (`debug` o `--features diag`): guion `MIEL_DIAG_SCRIPT` para medir sin automatizar la UI. |

### La FSM y los mensajes

- **Idle → {Follow, Rest, Reaction}** por pesos (40/25/35 por defecto) tras 20–45 s; Follow 6–12 s con
  cooldown de 90 s; Rest 40–90 s (`sleepy` 4 s y luego `resting`); Reaction 5–7 s (cooldown 40 s).
  Transiciones inválidas se ignoran.
- **Pausa/Oculta** suspenden todo: los plazos se congelan y se reanudan con el tiempo restante. Son
  independientes (mostrar una Miel pausada la deja pausada).
- **Mensajes**: cuando vence el cooldown, Miel espera al *siguiente cambio de comportamiento* y elige una
  frase coherente con él (y con la expresión de la reacción). El cooldown cuenta desde el *fin* del mensaje.
  Pausar/ocultar cancela el mensaje visible y congela el cooldown.

### IPC

- **Rust → React**: evento `mascot://view` solo cuando la vista compuesta cambia (nunca por frame).
  La vista tiene `version` monótona (suma de los contadores de FSM y Talker); React descarta versiones
  anteriores. React se suscribe **y después** pide `get_view`, de modo que no se pierde ningún cambio.
- **React → Rust**: tres comandos, cada uno con su permiso explícito (`allow-get-view`,
  `allow-get-display-prefs`, `allow-report-frontend-error`) + `core:event:allow-listen/unlisten`. Sin
  permisos de ventana, archivos, shell ni red.
- **Bandeja → motor**: `Cmd::{Hidden, TogglePause, ToggleFollow, GeometryChanged, …}` (no bloqueante).
  El menú se actualiza desde la vista del motor mediante `run_on_main_thread` (el hilo del motor nunca
  toca el menú).

## React (`src`)

- `App` → `ErrorBoundary` → `MascotStage` → `Bee` (SVG inline original; los estados cambian con
  `data-state`, sin remontar el nodo) + `SpeechBubble` (presentacional; sin temporizadores de negocio).
- Geometría: `components/MascotStage/layout.ts` es espejo de `window::metrics` (mismos números, probados
  en ambos lados): ventana `260 × (104 + tamaño)`, sprite a `80 + tamaño/2` px del borde superior.
- `services/`: suscripción versionada (`mascotEvents`), preferencias de presentación (`displayPrefs`) e
  informe de errores (`errorReport`).

## CSP y estilos: por qué no hay `'unsafe-inline'`

**CSP de producción**: `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; object-src 'none';
base-uri 'none'; form-action 'none'; connect-src ipc: http://ipc.localhost`. La de desarrollo (`devCsp`) añade lo
que necesita Vite (`'unsafe-inline'`, `ws://localhost:1420`) y **no** se usa en el instalador.

**Lo que ocurrió (regresión corregida)**: en producción Tauri inyecta un `'nonce-…'` en `style-src` cuando el
HTML tiene un `<style>` (el de `index.html`). Con un nonce o un hash presente, el navegador **ignora
`'unsafe-inline'`** ("Note that 'unsafe-inline' is ignored if either a hash or nonce value is present"). El rig
`bee-rig.svg` llevaba su CSS en un `<style>` embebido; al montarse como SVG inline, ese `<style>` quedaba
**bloqueado**: se perdían `.face{opacity:0}`, los selectores `[data-state]` y los `@keyframes` → los 8 rostros
apilados y sin animación. En desarrollo no pasaba (no hay nonce en `devCsp`).

**Cómo se evita** (sin relajar la CSP):
- `Bee.tsx` importa **`bee-rig.nostyle.svg`** (derivado reproducible del generador original con
  `tools/make_nostyle_rig.py`: `build_rig_svg("")` sin el `<style>`; la única diferencia con `bee-rig.svg` es la
  ausencia del `<style>`, verificada por el propio script y por tests) e importa **`bee.css`** como archivo
  (Vite lo empaqueta como CSS propio, servido desde `'self'`). Los originales no se tocan.
- Los `style={{…}}` de React (variables CSS `--bee-size`, `--bee-anchor-y`) se aplican por **CSSOM**
  (`style.setProperty`), que la CSP permite. El `<style>` de `index.html` recibe el nonce de Tauri.
- **Red de seguridad**: `tools/check_dist.mjs` se ejecuta al final de `npm run build` (y por tanto de
  `tauri build`) y falla si el CSS compilado no contiene la regla que oculta los rostros inactivos, los 8
  selectores de estado, los `@keyframes` y `prefers-reduced-motion`, o si el JS compilado incluye CSS del rig
  (= `<style>` embebido).

**Lección**: las pruebas con jsdom/Vitest y con Edge headless **no aplican la CSP final de Tauri** (ni su nonce),
por eso esta regresión pasó desapercibida durante varias etapas. Lo que valida la CSP real es ejecutar el
**binario de producción** (se puede leer la CSP efectiva y las violaciones con el puerto de depuración de
WebView2) y la comprobación automática de `dist/`.

## Ciclo de vida y cierre

1. **Arranque**: la ventana nace oculta; se dimensiona/configura/posiciona; arrancan motor y bandeja
   (si la bandeja falla, el arranque se **aborta**: no queda una mascota click-through sin salida); solo
   cuando la bandeja está lista *y* la página cargó se hace el primer `show` (una sola vez).
2. **"Cerrar Miel"** (o cualquier `ExitRequested`): `begin_shutdown` = `Engine::request_stop()` (bandera +
   Condvar, **no bloquea**) + `DisplayWatch::remove()` (retira `SetWindowSubclass` y libera el callback) →
   `app.exit(0)` (Tauri vacía el icono de bandeja y oculta la ventana) → `RunEvent::Exit` →
   `Engine::shutdown_timeout(2 s)`.
3. **Por qué dos fases**: los *getters* de Tauri (`outer_position`, `cursor_position`, monitores…) esperan
   con `recv()` **sin timeout** a que responda el hilo principal. Un `join` del motor desde el hilo
   principal con el bucle vivo podría interbloquearse; por eso la espera con plazo ocurre en `Exit`.
4. **Cierre forzado**: si el motor no termina en 2 s (llamada del sistema atascada) **no se hace `join`**,
   se registra `cierre forzado` y el proceso continúa. No se afirma que el hilo terminara: al finalizar
   el proceso Windows destruye el hilo y libera sus recursos. Nada impide la salida del proceso.
5. Todo es **idempotente** y se prueba (`request_stop` repetido y desde varios hilos, cierre en Follow,
   Reaction, con mensaje, en pausa, oculta, con aviso de geometría pendiente).

## Persistencia

`settings.json` en el directorio de configuración de la app (`app_config_dir`, derivado del
*identifier*): `follow_enabled`, `messages_enabled`, `message_interval_minutes`, `pet_size` (64/96/128),
`speed`, `reduced_motion`, `paused`. **`hidden` no se guarda.** Pausa y seguimiento se reflejan al cambiar
desde la bandeja; solo se escribe si el valor cambió. Ver `src-tauri/src/settings.rs`.

## Límites conocidos de la recuperación del frontend

El `ErrorBoundary` captura errores de **renderizado**. No captura errores en manejadores de eventos, código
asíncrono, promesas rechazadas ni la caída del proceso de WebView2. Los dos primeros se *informan* (no se
recuperan) con `window.onerror`/`unhandledrejection`; si WebView2 muere, la ventana queda vacía pero la
bandeja (Rust) sigue funcionando y permite cerrar Miel.

## Identidad de la aplicación

| Campo | Valor |
|---|---|
| Identificador (`tauri.conf.json`) | **`lat.bkode.miel`** (definitivo; dominio de referencia `bkode.lat`, marca BKODE) |
| Nombre visible / producto | `Miel` |
| Versión | `0.1.0` |
| Ejecutable | `miel.exe` |

Se usó `com.example.miel` solo durante el desarrollo y las pruebas. **No se migran** preferencias de ese
identificador y **no se borra** ningún directorio anterior sin autorización de la persona titular.

### Dónde se refleja el identificador (impacto del cambio)

| Elemento | Con `lat.bkode.miel` | Observación |
|---|---|---|
| Preferencias | `%APPDATA%\lat.bkode.miel\settings.json` (Roaming) | Se crea al primer cambio de pausa/seguimiento (no al arrancar). No se migra nada del identificador anterior. |
| Datos de WebView2 | `%LOCALAPPDATA%\lat.bkode.miel\EBWebView` | **Verificado** ejecutando el binario de producción. |
| Registro de errores del frontend | directorio de registros de Tauri bajo el identificador (`…\lat.bkode.miel\logs\`) | No observado (solo se crea al informar un error). |
| Instancia única | derivada del identificador (valor por defecto del plugin) | **Verificado**: una 2.ª ejecución con la nueva identidad termina sola y queda 1 proceso. Un binario compilado con **otro** identificador no detecta a este (p. ej. compilaciones antiguas `com.example.miel`). |
| Instalador NSIS | `BUNDLEID = lat.bkode.miel`; `MANUFACTURER = bkode` (derivado del identificador; es el *Publisher* de "Aplicaciones instaladas"); `PRODUCTNAME = Miel`; modo por usuario | Ver abajo. |
| Directorio de instalación | `%LOCALAPPDATA%\Miel` | Depende del nombre de producto, no del identificador. |
| Claves de registro | `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Miel` y `HKCU\Software\bkode\Miel` | La primera por *nombre de producto*; la segunda por *fabricante derivado*. |

### Actualizaciones y desinstalación

- **Continuidad de actualizaciones**: el instalador localiza la instalación previa por la clave de
  desinstalación del *nombre de producto* (`…\Uninstall\Miel`) y compara versiones (avisa en una degradación).
  Mantener **`Miel` + `lat.bkode.miel`** estables garantiza que una versión futura se instale *sobre* la anterior
  y conserve `%APPDATA%\lat.bkode.miel\settings.json`. Cambiar otra vez el identificador reabriría los
  problemas habituales: preferencias "perdidas", datos de WebView2 duplicados, otra identidad de instancia
  única, y (si cambia el 2.º segmento) otro *Publisher*/clave de fabricante. **No volver a cambiarlo.**
- **Desinstalación**: el desinstalador ofrece la casilla *"Eliminar los datos de la aplicación"*. **Si se
  marca**, borra `%APPDATA%\lat.bkode.miel` (**incluidas las preferencias**) y `%LOCALAPPDATA%\lat.bkode.miel`
  (WebView2 y registros). **Sin marcarla**, no borra datos de usuario. Nunca toca los directorios del
  identificador anterior (`com.example.miel`).
- **Residuos de desarrollo**: `%LOCALAPPDATA%\com.example.miel` (perfil de WebView2 de las pruebas) puede
  seguir existiendo en equipos de desarrollo; no pertenece a la versión definitiva y puede borrarse a mano.
- **Publisher**: Windows mostrará `bkode` (en minúsculas, derivado). Si se desea `BKODE`, se puede fijar
  `bundle.publisher` en `tauri.conf.json`; **no se hizo** por no estar entre los cambios solicitados.
