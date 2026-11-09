# Changelog

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/); el proyecto sigue [versionado semántico](https://semver.org/lang/es/) en la medida de lo posible (mientras sea 0.x, puede haber cambios incompatibles entre versiones menores). / Based on Keep a Changelog; follows semantic versioning as far as possible (breaking changes may occur between 0.x minor versions).

## [Sin publicar / Unreleased]

### Añadido
- Preparación para la publicación open source (Hito 0): `ROADMAP.md`, README bilingüe, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, plantillas de issues y PR, inventario de etiquetas, CI en GitHub Actions, notas de release y checklist de publicación.
- `THIRD_PARTY_LICENSES.txt` con los textos de licencia y avisos de copyright de las dependencias de terceros (Rust y npm de producción) y el código fuente de las crates MPL-2.0; se instala con Miel, se adjunta a la release y la CI comprueba que esté al día con `Cargo.lock` y `package-lock.json` (`tools/gen_third_party_licenses.py`).

- Validación desde clon limpio (`tools/validate_clean_clone.ps1`, `docs/CLEAN_CLONE_VALIDATION.md`), comprobación post-instalación (`tools/check_install.ps1`), checklist manual reducida y estrategia de actualización de dependencias (`docs/DEPENDENCY_UPDATES.md`). La release se prepara como pre-release.

### Cambiado
- Dependabot **no está activado** (propuesta en `docs/proposed/`); el job `licenses` de la CI es obligatorio.
- Las cifras de rendimiento de `docs/PERFORMANCE.md` se marcaron como invalidadas.
- `publish = false` en `src-tauri/Cargo.toml`.
- Se retiraron de `docs/img/` cuatro capturas que mostraban contenido del escritorio del equipo de desarrollo.

## [0.2.0] - PLANIFICADA / PLANNED (no publicada; nada de esto está implementado)

> Esta sección describe lo **planificado** en [`docs/plan/v0.2.0.md`](docs/plan/v0.2.0.md); se actualizará al implementarse cada entrega.

### Planificado / Planned
- **Ajustes bajo demanda:** ventana propia que existe solo mientras está abierta; interruptores para las capacidades automáticas.
- **No molestar automático (activado por defecto):** Miel se **oculta** y cancela sus mensajes mientras Windows indica pantalla completa de una app, juego Direct3D exclusivo o modo presentación, y vuelve sola al terminar. Es una heurística de Windows: puede ocultarla sin necesidad (p. ej. vídeo a pantalla completa) o no detectar juegos sin bordes. La bandeja lo indica y permite "Mostrar de todos modos" (solo para ese episodio) o apagar la función. Si ocultaste a Miel tú mismo, no reaparece al terminar la pantalla completa.
- **Dormir por inactividad (activado por defecto, 10 min):** tras un rato sin usar el equipo Miel se queda dormida (sigue visible, sin mensajes) y despierta al volver. Solo usa un contador de inactividad del sistema; no registra teclas ni clics ni guarda historial.
- **Reacción al cursor (activada por defecto):** Miel reacciona si acercas el cursor cuando está tranquila, respetando los tiempos de espera existentes. Se medirá su costo antes de publicar.
- **Mensajes y comportamiento por hora del día (A6 mínima, E6b; activado por defecto):** según la hora local del equipo (madrugada 00:00–05:59, mañana 06:00–11:59, tarde 12:00–18:59, noche 19:00–23:59) Miel tiene mensajes propios de cada franja, descansa algo más de madrugada y saluda una vez la primera vez que habla tras arrancar o desbloquear (respetando el tiempo de espera, No molestar, el sueño y la pausa). Usa solo la hora local, sin sondeo nuevo, sin expresiones nuevas y sin guardar nada; se apaga con un interruptor en ajustes («usa solo la hora local de tu equipo»).
- **Actualizar desde v0.1.0 activa estos comportamientos** porque son los valores por defecto; cada uno se apaga desde ajustes.
- **Privacidad verificable (E0, implementado en el repositorio; CI remota pendiente de ver en verde):** nuevo [`docs/PRIVACY.md`](docs/PRIVACY.md) y `tools/check_privacy.py` (símbolos prohibidos, permisos exactos, API de `windows` permitida y línea base de dependencias con capacidad de red, con prueba negativa `--self-test`) como trabajo `privacy` de la CI. Es un control preventivo, no una demostración.
- **DCO en los pull requests (E0):** trabajo `dco` de la CI con `tools/check_dco.py` (cada commit con `Signed-off-by`; sin acciones de terceros).
- **Menos CPU en reposo (E0b, aprobado):** medido con el binario de producción, Idle 10,78 → 5,30 % de un núcleo (−51 %) y Rest 9,86 → 2,98 % (−70 %), solo con CSS (los SVG no cambian): los rostros ocultos ya no animan y el movimiento visible usa `steps()` alineados a 30 fps (Idle, aleteo estándar) y 20 fps (Rest/Sleep). Paused y Hidden ya estaban en el suelo. Límites: un solo equipo y monitor a 165 Hz (a otra frecuencia no medido), mediana de 5 repeticiones; los estados de reacción no se han cuantizado. Detalle y reglas en [`docs/ANIMATION_PERFORMANCE.md`](docs/ANIMATION_PERFORMANCE.md).
- **Licencias dentro del instalador (E0c):** el instalador de Windows incluye ahora `LICENSE-MIT`, `LICENSE-APACHE`, `LICENSE-ASSETS`, `NOTICE` y `THIRD_PARTY_LICENSES.txt` en la carpeta de instalación (recursos en `tauri.conf.json`); `tools/check_install.ps1` comprueba su presencia y su contenido tras instalar y que desaparecen al desinstalar. Sin cambios en el código de la aplicación ni en la versión.
- **Reloj y sonda del sistema inyectables (E1a, interno):** el motor mide el tiempo con un `Clock` y se añade una abstracción `SystemProbe` (inactividad y estado de notificaciones) con su implementación de Windows, **sin usarla todavía**: sin cambios de comportamiento observables (medición de no regresión dentro del ruido de ≈ ±10 %, 165 Hz, un equipo).
- **Compatibilidad:** `settings.json` pasa a `schema_version` 2 (incluye el interruptor de la hora del día). Volver a v0.1.0 conserva lo común, **pierde** la configuración nueva (v0.1.0 la reescribe como versión 1) y al regresar los interruptores vuelven a sus valores por defecto.

## [0.1.0] - FECHA POR DEFINIR (al crear la release) / DATE TBD

Primera versión pública (MVP).

### Añadido
- Mascota de escritorio para Windows 11 x64: ventana transparente, sin marco, siempre visible, *click-through* y sin robar el foco.
- Abeja original en SVG con 8 expresiones (idle, happy, curious, thinking, sleepy, surprised, talking, resting) y animaciones por CSS.
- Comportamientos Idle, Follow (seguimiento ocasional del cursor), Rest y Reaction, con máquina de estados pura en Rust.
- Mensajes locales en burbuja (catálogo de 16 frases, 12 minutos de cooldown por defecto).
- Bandeja del sistema: Mostrar/Ocultar, Seguir cursor, Pausar/Reanudar, Cerrar.
- Preferencias persistentes en JSON, instancia única, soporte de varios monitores, DPI y coordenadas negativas.
- Instalador NSIS por usuario (sin privilegios de administrador), **sin firma de código**.
- Identificador de aplicación `lat.bkode.miel`.

### Corregido
- En el binario de producción la CSP bloqueaba el `<style>` embebido del rig SVG (Tauri añade un nonce que anula `'unsafe-inline'`), de modo que se veían los 8 rostros apilados y sin animación. Ahora el rig se sirve sin `<style>` (`bee-rig.nostyle.svg`) y el CSS como archivo propio (`bee.css`); `npm run build` ejecuta `check:dist` como red de seguridad.

### Limitaciones conocidas
- Instalador sin firmar (SmartScreen avisará).
- Pruebas manuales pendientes: instalación/desinstalación, varios monitores y DPI reales, uso prolongado.
- Sin cifras de rendimiento: las mediciones anteriores de CPU/RAM están invalidadas (build sin animaciones CSS) y deben repetirse.
- Miel aparece encima de apps a pantalla completa.

[Sin publicar / Unreleased]: ../../compare/v0.1.0...HEAD
[0.1.0]: ../../releases/tag/v0.1.0
