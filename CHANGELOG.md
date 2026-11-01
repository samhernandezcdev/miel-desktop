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
