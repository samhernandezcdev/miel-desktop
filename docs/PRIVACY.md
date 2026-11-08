# Privacidad de Miel / Miel privacy

**[Español](#español) · [English](#english)**

> Este documento describe **qué consulta Miel y qué no**, y cómo se comprueba. Distingue lo que **ya hace v0.1.0** de lo **planificado para v0.2.0** (aún sin implementar). No es asesoría legal.

---

# Español

## Resumen
- Miel **no usa red, IA, servicios externos ni telemetría**. No envía nada y no guarda historial de tu actividad.
- Solo consulta señales básicas del sistema, cada una con su interruptor (en v0.2.0, en la ventana de ajustes).
- Lo único que Miel guarda en tu equipo son sus **preferencias** (`%APPDATA%\lat.bkode.miel\settings.json`): tamaño, velocidad, seguimiento, mensajes, movimiento reducido y, en v0.2.0, los interruptores nuevos.

## Qué consulta Miel

| Señal | Para qué | Estado | Interruptor |
|---|---|---|---|
| Posición del cursor | Seguirlo (v0.1.0) y reaccionar si se acerca (v0.2.0) | v0.1.0: solo con "Seguir cursor"; v0.2.0: proximidad **planificada** | Seguimiento / Proximidad |
| Monitores, área de trabajo y DPI | Colocar la ventana y escalar el tamaño | v0.1.0 | — |
| Tiempo desde la última entrada (`GetLastInputInfo` + `GetTickCount`) | Dormir tras un rato sin usar el equipo | **Planificado** (v0.2.0) | Sueño |
| Estado de notificaciones del usuario (`SHQueryUserNotificationState`) | No molestar con pantalla completa, juego Direct3D o modo presentación | **Planificado** (v0.2.0) | No molestar automático |
| Bloqueo/desbloqueo de sesión y suspensión/reanudación (mensajes de Windows) | Recalcular el estado al volver | **Planificado** (v0.2.0) | (con Sueño) |

### Lo que NO lee ni hace
Miel **no**: lee lo que escribes (ni teclas ni pulsaciones), registra clics, hace capturas de pantalla, lee títulos o contenido de ventanas, lee el contenido de tus archivos ni conoce qué aplicaciones usas. Tampoco manipula tu cursor ni genera entradas.

## Listas de la comprobación automática
**Permitido** (lista exacta en [`tools/privacy_policy.json`](../tools/privacy_policy.json)): `GetLastInputInfo`, `GetTickCount`/`GetTickCount64`, `SHQueryUserNotificationState`, notificaciones de sesión (`WTSRegisterSessionNotification`) y de energía (`WM_POWERBROADCAST`), y la geometría, monitores, DPI y ventanas que Miel ya usa.

**Prohibido** (la comprobación falla si aparece en `src-tauri/src/**` o `src/**`):
- Teclado y ratón globales: `SetWindowsHookEx*`, `RegisterRawInputDevices`, `GetRawInputData`, `GetAsyncKeyState`, `GetKeyState`, `GetKeyboardState`.
- Captura de pantalla: `BitBlt`, `PrintWindow`, Desktop Duplication, Graphics Capture.
- Lectura de ventanas: `GetWindowText*`, `InternalGetWindowText`, UI Automation.
- En el frontend: `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource`, `sendBeacon`, `getDisplayMedia`.
- Permisos de aplicación (capabilities) distintos de la lista aprobada (p. ej. `http`, `fs`, `shell`) y APIs de la crate `windows` no aprobadas.

## Limitaciones de las señales

**`GetLastInputInfo`** (tiempo de inactividad)
- Es un contador de **toda la sesión**: no distingue dispositivo ni aplicación, y no dice **qué** entrada fue. Miel solo lee cuánto tiempo pasó.
- Cuenta también entradas generadas por programas (p. ej. escritorio remoto, accesibilidad o herramientas de automatización): un programa que simule actividad mantiene despierta a Miel.
- Se calcula con `GetTickCount`, un contador de 32 bits que **da la vuelta cada ≈ 49,7 días**; Miel resta con aritmética `wrapping` y trata lecturas anómalas con prudencia (no despierta ni duerme por una lectura imposible).
- Puede comportarse distinto con sesiones remotas, con el equipo bloqueado o en un cambio de usuario.

**`SHQueryUserNotificationState`** (No molestar)
- Es una **heurística de Windows**, no un detector de pantalla completa ni del asistente de concentración (*Focus Assist*). Puede dar **falsos positivos** (p. ej. vídeo a pantalla completa) y **falsos negativos** (juegos sin bordes, algunas apps).
- Miel solo suprime con tres estados: pantalla completa/ocupado, juego Direct3D exclusivo y modo presentación. `QUNS_QUIET_TIME` y `QUNS_APP` **no** suprimen, y un error de la consulta nunca oculta a Miel.
- Si la consulta falla repetidamente, Miel vuelve a mostrarse.

## Red y telemetría
- La compilación por defecto no incluye código de red ni telemetría.
- **WebView2 está fuera del control de Miel.** El runtime de WebView2 es un componente de Microsoft con su **propio comportamiento de red y de diagnóstico** (por ejemplo, actualizaciones del runtime y los datos de diagnóstico que Microsoft o Windows tengan configurados). Se rige por su configuración y la de Windows, y Miel no puede ni auditarlo ni desactivarlo. Miel no añade conexiones por su cuenta, pero **no puede garantizar qué hace el runtime**. El **instalador** descarga el *bootstrapper* de WebView2 desde Microsoft solo si el runtime falta.
- Dependencias con capacidad de red presentes **de forma transitiva** (se documentan, no se ocultan; línea base en [`tools/privacy_baseline.json`](../tools/privacy_baseline.json)): `tokio`, `mio`, `socket2`, `http`, `url` y `tauri-plugin-single-instance` (para la instancia única). Que existan en el árbol de dependencias no significa que Miel abra conexiones: no se ha verificado de forma exhaustiva en tiempo de ejecución, y esta comprobación no lo demuestra.
- El CSP de producción solo permite `ipc:` y `http://ipc.localhost` (el canal interno entre la interfaz y Rust).

## Cómo se comprueba (controles preventivos)
```
python -I tools/check_privacy.py              # comprobación completa (necesita cargo y `npm ci`)
python -I tools/check_privacy.py --self-test  # prueba negativa: debe FALLAR ante entradas indebidas
python -I tools/check_privacy.py --update-baseline   # tras revisar una dependencia nueva
python -I tools/check_dco.py --self-test      # DCO (casos con y sin firma)
```
La CI ejecuta el trabajo `privacy` (autotest y comprobación) en cada push y pull request.

**Aviso importante:** son controles **preventivos**. Detectan símbolos y configuraciones concretos, **no demuestran** la ausencia de acceso a datos ni sustituyen la revisión de código: alguien podría acceder a un dato por una vía que la lista no contempla. Las limitaciones conocidas del análisis: busca nombres de símbolos (no el comportamiento), trata como código las cadenas de texto y no analiza dependencias de terceros más allá de la línea base de crates y paquetes.

---

# English

## Summary
- Miel **uses no network, AI, external services or telemetry**. It sends nothing and keeps no history of your activity.
- It only queries basic system signals, each with its own switch (in v0.2.0, in the settings window).
- The only thing Miel stores on your machine is its **preferences** (`%APPDATA%\lat.bkode.miel\settings.json`): size, speed, follow, messages, reduced motion and, in v0.2.0, the new switches.

## What Miel queries

| Signal | Purpose | Status | Switch |
|---|---|---|---|
| Cursor position | Follow it (v0.1.0) and react when it gets close (v0.2.0) | v0.1.0: only with "Follow cursor"; v0.2.0: proximity **planned** | Follow / Proximity |
| Monitors, work area and DPI | Place the window and scale its size | v0.1.0 | — |
| Time since the last input (`GetLastInputInfo` + `GetTickCount`) | Fall asleep after a while without using the computer | **Planned** (v0.2.0) | Sleep |
| User notification state (`SHQueryUserNotificationState`) | Do not disturb during full screen, exclusive Direct3D or presentation mode | **Planned** (v0.2.0) | Automatic Do Not Disturb |
| Session lock/unlock and suspend/resume (Windows messages) | Recompute state on return | **Planned** (v0.2.0) | (with Sleep) |

### What it does NOT read or do
Miel does **not**: read what you type (no keys, no keystrokes), record clicks, take screenshots, read window titles or contents, read the contents of your files, or know which applications you use. It does not move your cursor or generate input.

## Lists enforced by the automatic check
**Allowed** (exact list in [`tools/privacy_policy.json`](../tools/privacy_policy.json)): `GetLastInputInfo`, `GetTickCount`/`GetTickCount64`, `SHQueryUserNotificationState`, session notifications (`WTSRegisterSessionNotification`) and power notifications (`WM_POWERBROADCAST`), plus the geometry, monitor, DPI and window APIs Miel already uses.

**Forbidden** (the check fails if they appear in `src-tauri/src/**` or `src/**`):
- Global keyboard and mouse: `SetWindowsHookEx*`, `RegisterRawInputDevices`, `GetRawInputData`, `GetAsyncKeyState`, `GetKeyState`, `GetKeyboardState`.
- Screen capture: `BitBlt`, `PrintWindow`, Desktop Duplication, Graphics Capture.
- Window reading: `GetWindowText*`, `InternalGetWindowText`, UI Automation.
- In the frontend: `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource`, `sendBeacon`, `getDisplayMedia`.
- Application permissions (capabilities) other than the approved list (e.g. `http`, `fs`, `shell`) and unapproved `windows` crate APIs.

## Limitations of the signals

**`GetLastInputInfo`** (idle time)
- It is a **session-wide** counter: it does not tell the device or the application, nor **what** input it was. Miel only reads how much time has passed.
- It also counts input generated by programs (e.g. remote desktop, accessibility or automation tools): a program that simulates activity keeps Miel awake.
- It is computed with `GetTickCount`, a 32-bit counter that **wraps every ≈ 49.7 days**; Miel subtracts with wrapping arithmetic and handles anomalous readings defensively (an impossible reading neither wakes nor sleeps Miel).
- It may behave differently with remote sessions, a locked machine or a user switch.

**`SHQueryUserNotificationState`** (Do Not Disturb)
- It is a **Windows heuristic**, not a full-screen detector nor a *Focus Assist* detector. It can give **false positives** (e.g. full-screen video) and **false negatives** (borderless games, some apps).
- Miel only suppresses on three states: busy/full screen, exclusive Direct3D and presentation mode. `QUNS_QUIET_TIME` and `QUNS_APP` do **not** suppress, and a query error never hides Miel.
- If the query keeps failing, Miel shows itself again.

## Network and telemetry
- The default build contains no network code and no telemetry.
- **WebView2 is outside Miel's control.** The WebView2 runtime is a Microsoft component with its **own network and diagnostics behaviour** (for example runtime updates and whatever diagnostic data Microsoft or Windows have configured). It is governed by its and Windows' settings, and Miel can neither audit nor disable it. Miel adds no connections of its own, but **cannot guarantee what the runtime does**. The **installer** downloads the WebView2 bootstrapper from Microsoft only if the runtime is missing.
- Network-capable dependencies present **transitively** (documented, not hidden; baseline in [`tools/privacy_baseline.json`](../tools/privacy_baseline.json)): `tokio`, `mio`, `socket2`, `http`, `url` and `tauri-plugin-single-instance` (single instance). Being in the dependency tree does not mean Miel opens connections: this has not been exhaustively verified at runtime, and this check does not prove it.
- The production CSP only allows `ipc:` and `http://ipc.localhost` (the internal channel between the UI and Rust).

## How it is checked (preventive controls)
```
python -I tools/check_privacy.py              # full check (needs cargo and `npm ci`)
python -I tools/check_privacy.py --self-test  # negative test: must FAIL on bad inputs
python -I tools/check_privacy.py --update-baseline   # after reviewing a new dependency
python -I tools/check_dco.py --self-test      # DCO (cases with and without sign-off)
```
CI runs the `privacy` job (self-test and check) on every push and pull request.

**Important:** these are **preventive** controls. They detect specific symbols and settings; they **do not prove** the absence of data access and do not replace code review: data could be reached through a path the list does not cover. Known limits of the analysis: it matches symbol names (not behaviour), treats string literals as code and does not inspect third-party dependencies beyond the baseline of crates and packages.
