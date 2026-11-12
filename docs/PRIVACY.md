# Privacidad de Miel / Miel privacy

**[Español](#español) · [English](#english)**

> Este documento describe **qué consulta Miel y qué no**, y cómo se comprueba. Distingue lo que **ya hace v0.1.0** de lo **planificado para v0.2.0**; cada señal indica su **estado**: «Activo» si ya está implementada (hoy, No molestar automático y Sueño) y «Planificado» si aún no. No es asesoría legal.

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
| Tiempo desde la última entrada (`GetLastInputInfo` + `GetTickCount`) | Dormir tras un rato sin usar el equipo | **Activo** (v0.2.0, E5): un número (cuánto tiempo), solo con «Sueño» activado y Miel visible y sin pausar; sin sondeo periódico mientras está despierta (calcula cuándo mirar) y cada ~2 s mientras duerme | Sueño (se aplica al instante) |
| Estado de notificaciones del usuario (`SHQueryUserNotificationState`) | No molestar con pantalla completa, juego Direct3D o modo presentación | **Activo** (v0.2.0, E4b): cada ~2,5 s, solo con el interruptor activado y sin que la persona haya ocultado o pausado a Miel | No molestar automático (se aplica al instante) |
| Bloqueo/desbloqueo de sesión y suspensión/reanudación (mensajes de Windows) | Dormir al instante al bloquear/suspender y despertar al volver | **Activo** (v0.2.0, E5): Windows avisa a la ventana de Miel; sin sondeo | (con Sueño) |
| Hora local del equipo (`GetLocalTime`, solo la hora) | Mensajes, saludo y ritmo de descanso según la franja del día | **Planificado** (v0.2.0, E6b) | Hora del día |

### Lo que NO lee ni hace
Miel **no**: lee lo que escribes (ni teclas ni pulsaciones), registra clics, hace capturas de pantalla, lee títulos o contenido de ventanas, lee el contenido de tus archivos ni conoce qué aplicaciones usas. Tampoco manipula tu cursor ni genera entradas.

## Listas de la comprobación automática
**Permitido** (lista exacta en [`tools/privacy_policy.json`](../tools/privacy_policy.json)): `GetLastInputInfo`, `GetTickCount`/`GetTickCount64`, `GetLocalTime` (E6b), `SHQueryUserNotificationState`, notificaciones de sesión (`WTSRegisterSessionNotification`) y de energía (`WM_POWERBROADCAST`), y la geometría, monitores, DPI y ventanas que Miel ya usa.

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
- **Qué recibe Miel y qué no (E5, activo):** un único número (milisegundos desde la última entrada de la sesión). **No sabe qué teclas** se pulsaron, ni qué ratón ni qué ventana, ni qué haces; no mira la ventana en primer plano. El número no se guarda ni se envía: solo decide si Miel se duerme o despierta.
- **El límite de las entradas simuladas:** la inactividad de Windows también cuenta como uso lo que otros programas **simulen** (escritorio remoto, accesibilidad, automatización, algunos juegos o reproductores). Un programa así mantiene despierta a Miel aunque no estés ahí; Miel no puede distinguirlo.
- **Cuándo consulta:** solo con «Sueño» activado y con Miel visible y sin pausar (ni oculta a mano, ni suprimida por No molestar). Despierta **no hay sondeo periódico**: Miel calcula cuándo tocaría dormirse (tiempo elegido − inactividad actual, con un mínimo de 1 s) y vuelve a mirar entonces; si hubo actividad, recalcula. Dormida consulta cada ~2 s solo para detectar actividad. Si la lectura falla, **nunca se duerme** por ello: reintenta con espera creciente (5 s, 10 s… hasta 1 min), y dormida, tres errores seguidos la despiertan.
- **Bloqueo y suspensión (sin leer nada):** Windows avisa a la ventana de Miel de que **esta** sesión se bloqueó o desbloqueó y de que el equipo se suspende o se reanuda (`WTSRegisterSessionNotification` con `NOTIFY_FOR_THIS_SESSION`, `WM_WTSSESSION_CHANGE`, `WM_POWERBROADCAST`). Solo se distinguen esos cuatro avisos; no se consulta quién es el usuario ni otras sesiones. Al bloquear o suspender Miel duerme al instante; al desbloquear o reanudar despierta con una pequeña sorpresa. Tras reanudar o desbloquear se trata como actividad reciente aunque el contador aún no lo refleje.
- **Despertar desde la bandeja:** mostrar, reanudar, «Mostrar de todos modos» o activar «Seguir cursor» desde la bandeja la despiertan al instante (ocultada o pausada no se consulta nada). Apagar el interruptor «Sueño» la despierta y detiene todo el sondeo.

**`GetLocalTime`** (hora del día, E6b)
- Miel lee **solo la hora** (0–23) del reloj local del equipo, para elegir entre cuatro franjas (madrugada 00:00–05:59, mañana 06:00–11:59, tarde 12:00–18:59, noche 19:00–23:59). No usa fecha, zona horaria ni ubicación, no lo guarda ni lo envía.
- **No tiene sondeo propio:** Miel consulta la hora solo cuando ya despierta por otra razón (decidir qué hacer, elegir una frase, volver de una pausa); si está pausada, oculta, dormida o suprimida, no la consulta.
- Si la hora del equipo es incorrecta, las frases corresponderán a esa hora. Se apaga con el interruptor «Hora del día» (usa solo la hora local de tu equipo).

**`SHQueryUserNotificationState`** (No molestar)
- Es una **heurística de Windows**, no un detector de pantalla completa ni del asistente de concentración (*Focus Assist*). Puede dar **falsos positivos** (p. ej. vídeo a pantalla completa) y **falsos negativos** (juegos sin bordes, algunas apps).
- Miel solo suprime con tres estados: pantalla completa/ocupado, juego Direct3D exclusivo y modo presentación. `QUNS_QUIET_TIME` y `QUNS_APP` **no** suprimen, y un error de la consulta nunca oculta a Miel.
- Si la consulta falla repetidamente, Miel vuelve a mostrarse.
- **Qué recibe Miel y qué no (E4b, activo):** un único número de estado. Miel **no sabe qué aplicación** está a pantalla completa, qué muestra ni en qué monitor; no mira la ventana en primer plano, ni procesos, ni títulos. El estado no se guarda ni se envía: solo decide, en el momento, si Miel se oculta o reaparece.
- **Cuándo consulta:** cada ~2,5 s mientras el interruptor «No molestar automático» está activado y la persona no ha **ocultado ni pausado** a Miel (se consulta también mientras está oculta por No molestar, para saber cuándo termina). Si la oculta o la pausa a mano, o apaga el interruptor, el sondeo se detiene por completo (apagar el interruptor actúa al instante y termina cualquier supresión en curso).
- **Dos lecturas seguidas** para empezar y para terminar (así un parpadeo no la oculta), y **tres errores seguidos** de la consulta terminan la supresión. Desde la bandeja, «Mostrar de todos modos» la anula durante la supresión en curso; no apaga el ajuste.
- **No es el asistente de concentración ni «No molestar» de Windows**: Miel no sabe si esos modos están activos. Un vídeo a pantalla completa en el navegador (F11) puede ocultarla; un juego «sin bordes» puede no hacerlo.
- Solo en compilaciones de diagnóstico (no en el instalador) una variable de entorno puede sustituir esta consulta por una secuencia programada para probar el comportamiento sin tocar el sistema.

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
| Time since the last input (`GetLastInputInfo` + `GetTickCount`) | Fall asleep after a while without using the computer | **Active** (v0.2.0, E5): one number (how long), only with "Sleep" on and Miel visible and not paused; no periodic polling while awake (it computes when to look) and every ~2 s while asleep | Sleep (applies immediately) |
| User notification state (`SHQueryUserNotificationState`) | Do not disturb during full screen, exclusive Direct3D or presentation mode | **Active** (v0.2.0, E4b): every ~2.5 s, only while the switch is on and the person has not hidden or paused Miel | Automatic Do Not Disturb (applies immediately) |
| Session lock/unlock and suspend/resume (Windows messages) | Fall asleep at once on lock/suspend and wake on return | **Active** (v0.2.0, E5): Windows notifies Miel's window; no polling | (with Sleep) |
| Local computer time (`GetLocalTime`, hour only) | Messages, greeting and rest rhythm by time of day | **Planned** (v0.2.0, E6b) | Time of day |

### What it does NOT read or do
Miel does **not**: read what you type (no keys, no keystrokes), record clicks, take screenshots, read window titles or contents, read the contents of your files, or know which applications you use. It does not move your cursor or generate input.

## Lists enforced by the automatic check
**Allowed** (exact list in [`tools/privacy_policy.json`](../tools/privacy_policy.json)): `GetLastInputInfo`, `GetTickCount`/`GetTickCount64`, `GetLocalTime` (E6b), `SHQueryUserNotificationState`, session notifications (`WTSRegisterSessionNotification`) and power notifications (`WM_POWERBROADCAST`), plus the geometry, monitor, DPI and window APIs Miel already uses.

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
- **What Miel receives and what it does not (E5, active):** a single number (milliseconds since the session's last input). It **does not know which keys** were pressed, nor which mouse or window, nor what you are doing; it does not look at the foreground window. The number is neither stored nor sent: it only decides whether Miel falls asleep or wakes up.
- **The synthetic-input limit:** Windows' idle time also counts as use whatever other programs **simulate** (remote desktop, accessibility, automation, some games or players). Such a program keeps Miel awake even if you are not there; Miel cannot tell the difference.
- **When it asks:** only with "Sleep" on and Miel visible and not paused (not hidden by hand, not suppressed by Do Not Disturb). While awake there is **no periodic polling**: Miel computes when it would be time to fall asleep (chosen time − current idle time, with a 1 s minimum) and looks again then; if there was activity, it recomputes. While asleep it asks every ~2 s only to detect activity. If the reading fails it **never falls asleep** because of it: it retries with growing waits (5 s, 10 s… up to 1 min), and while asleep three consecutive errors wake it.
- **Lock and suspend (reading nothing):** Windows notifies Miel's window that **this** session was locked or unlocked and that the computer suspends or resumes (`WTSRegisterSessionNotification` with `NOTIFY_FOR_THIS_SESSION`, `WM_WTSSESSION_CHANGE`, `WM_POWERBROADCAST`). Only those four notices are distinguished; it does not ask who the user is nor about other sessions. On lock or suspend Miel falls asleep at once; on unlock or resume it wakes with a small surprise. After resume or unlock it is treated as recent activity even if the counter does not show it yet.
- **Waking from the tray:** show, resume, "Mostrar de todos modos" or turning "Follow cursor" on from the tray wake it at once (while hidden or paused nothing is queried). Turning the "Sleep" switch off wakes it and stops all polling.

**`GetLocalTime`** (time of day, E6b)
- Miel reads **only the hour** (0–23) of the computer's local clock, to pick one of four bands (early morning 00:00–05:59, morning 06:00–11:59, afternoon 12:00–18:59, evening 19:00–23:59). It uses no date, time zone or location, and neither stores nor sends it.
- **It has no polling of its own:** Miel looks at the time only when it already wakes for another reason (deciding what to do, picking a phrase, coming back from a pause); while paused, hidden, asleep or suppressed it does not read it.
- If the computer's clock is wrong, the phrases will match that wrong time. It is turned off with the "Time of day" switch (it uses only your computer's local time).

**`SHQueryUserNotificationState`** (Do Not Disturb)
- It is a **Windows heuristic**, not a full-screen detector nor a *Focus Assist* detector. It can give **false positives** (e.g. full-screen video) and **false negatives** (borderless games, some apps).
- Miel only suppresses on three states: busy/full screen, exclusive Direct3D and presentation mode. `QUNS_QUIET_TIME` and `QUNS_APP` do **not** suppress, and a query error never hides Miel.
- If the query keeps failing, Miel shows itself again.
- **What Miel receives and what it does not (E4b, active):** a single status number. Miel **does not know which application** is full screen, what it shows or on which monitor; it does not look at the foreground window, processes or titles. The state is neither stored nor sent: it only decides, at that moment, whether Miel hides or comes back.
- **When it asks:** every ~2.5 s while the "Automatic Do Not Disturb" switch is on and the person has not **hidden or paused** Miel (it also asks while hidden by Do Not Disturb, to know when it ends). If the person hides or pauses it by hand, or turns the switch off, polling stops completely (turning the switch off acts immediately and ends any suppression in progress).
- **Two consecutive readings** to start and to end (so a blip does not hide it), and **three consecutive errors** from the query end the suppression. From the tray, "Mostrar de todos modos" (show anyway) overrides the current suppression; it does not turn the setting off.
- **It is not Windows focus assist or Do Not Disturb**: Miel does not know whether those modes are on. Full-screen video in the browser (F11) may hide it; a "borderless" game may not.
- Only in diagnostic builds (not in the installer) an environment variable can replace this query with a scripted sequence, to test the behaviour without touching the system.

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
