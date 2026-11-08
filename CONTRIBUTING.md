# Contribuir a Miel / Contributing to Miel

**[Español](#español) · [English](#english)**

---

# Español

¡Gracias por querer ayudar! Miel es una abeja de escritorio local y discreta. Cualquier persona puede contribuir; no hace falta experiencia previa con Rust ni con Tauri para mejorar documentación, frases, traducciones o el diseño del personaje.

Al participar aceptas el [Código de conducta](CODE_OF_CONDUCT.md). Las **vulnerabilidades** se reportan en privado (ver [`SECURITY.md`](SECURITY.md)), nunca en un issue público.

## Antes de empezar

1. Lee [`ROADMAP.md`](ROADMAP.md), sobre todo los **principios**: local primero, cada capacidad se activa por separado, sin telemetría, sin leer lo que escribes ni capturar la pantalla, discreta y sin manipulación. **Una contribución que rompa esos principios no se acepta, aunque funcione.**
2. Busca si ya existe un issue. Para algo más que una corrección pequeña, **abre un issue primero** y espera conformidad antes de invertir tiempo.
3. Si quieres algo para empezar, mira los issues con `good first issue` y `help wanted`.

Qué no entra en la v0.1.x: funcionalidades nuevas (el Hito A del roadmap empieza después del lanzamiento) ni cambios de arquitectura sin discusión previa.

## Preparar el entorno

Requisitos: Windows 11 x64, Node.js 22+, Rust estable (≥ 1.82) con `x86_64-pc-windows-msvc`, Visual Studio Build Tools (MSVC), WebView2 y, solo si tocas el personaje o las licencias, Python 3.

```powershell
npm install
npm run tauri dev
```

## Antes de abrir un pull request

Todo esto debe pasar (la CI lo ejecuta en Windows):

```powershell
npx tsc --noEmit
npm test
npm run build            # incluye check:dist
cargo test   --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Reglas del proyecto:

- **Rust es la única fuente de verdad** del comportamiento (FSM, movimiento, mensajes, bandeja, preferencias); React solo dibuja. La lógica va en funciones puras con **reloj y azar inyectados**, para probarla sin interfaz. Ver [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
- **Cada cambio llega con sus pruebas** y, si es una característica o corrige algo de comportamiento real, con su nota en [`docs/TEST_LOG.md`](docs/TEST_LOG.md). **No marques como completada una prueba manual que no hiciste.**
- **CSP estricta**: la de producción no permite estilos ni scripts en línea y es más estricta que la de `tauri dev`; jsdom y Vitest tampoco la aplican. Si tocas estilos o SVG, comprueba también el binario de producción (`npm run tauri build`) y deja pasar `check:dist`. No añadas `'unsafe-inline'` ni `'unsafe-eval'`.
- **Sin red y sin telemetría** en la compilación por defecto. Cualquier dependencia nueva se justifica en el PR (licencia compatible, tamaño, mantenimiento) y se refleja en [`docs/THIRD_PARTY.md`](docs/THIRD_PARTY.md) (`python -I tools/third_party.py`). **Si cambia `Cargo.lock` o `package-lock.json` (cualquier actualización de dependencias), regenera `THIRD_PARTY_LICENSES.txt`** con `python -I tools/gen_third_party_licenses.py` (necesita `cargo-about` y `npm ci`; ver la cabecera del script): la CI falla si no coincide con los lockfiles. Pasos completos en [`docs/DEPENDENCY_UPDATES.md`](docs/DEPENDENCY_UPDATES.md).
- **Capabilities mínimas** del frontend (`src-tauri/capabilities/default.json`): no se amplían sin una razón explicada.
- **Rendimiento**: Miel debe poder dormir en reposo. Nada de sondeos o animaciones que gasten CPU con la ventana pausada u oculta sin medirlo.

## Personaje (SVG)

**No edites a mano** los SVG de `src/assets/bee/`: se generan desde [`tools/build_bee.py`](tools/build_bee.py), la fuente única de verdad, para que los 8 estados no se desvíen. Para cambiar el personaje:

```powershell
python -I tools/build_bee.py
python -I tools/make_nostyle_rig.py        # regenera bee-rig.nostyle.svg
python -I tools/make_nostyle_rig.py --check
```

Sigue [`docs/BEE_GUIDE.md`](docs/BEE_GUIDE.md) (paleta, ids, estructura y `data-state`). Un rostro nuevo debe ser **original**: no se aceptan copias ni derivados de personajes, ilustraciones o marcas de terceros.

## Frases

El catálogo está en `src-tauri/src/mascot/messages.rs`. Tono: tierno, discreto, **sin regañar, sin presionar y sin suponer lo que hace la persona**; **máximo 64 caracteres**; cada frase declara en qué comportamientos puede decirse y con qué expresión. Añade una prueba si cambias la selección.

## Capturas e imágenes

**No subas capturas de tu escritorio** ni imágenes con rutas, nombres de usuario, ventanas de otras apps o contenido privado. Si necesitas una captura, usa un fondo liso o recorta solo la ventana de Miel y revísala antes de adjuntarla (también en issues).

## Commits y pull requests

- Cambios pequeños y enfocados; un PR, un propósito.
- Mensajes de commit claros en español o inglés, en imperativo y con el porqué.
- Rellena la plantilla del PR y actualiza [`CHANGELOG.md`](CHANGELOG.md) (sección *Sin publicar*).
- Una persona mantenedora revisará el PR; puede pedir cambios. Sé paciente: es un proyecto comunitario.

## Licencia de las contribuciones y DCO

No hay CLA. Cada commit debe llevar la línea `Signed-off-by: Nombre <correo>` (con `git commit -s`), con la que certificas el [Developer Certificate of Origin 1.1](DCO.md): que tienes derecho a aportar el cambio bajo la licencia del archivo.

Tu contribución se publica bajo la licencia del componente que modificas:
- código, herramientas y CSS funcional → **MIT OR Apache-2.0** (a elección de quien lo reciba);
- arte, personaje, stickers y documentación → **CC BY 4.0**;
- el **nombre y la identidad visual** de Miel (nombre, BKODE, logotipo e iconos) no se licencian y se rigen por la [política de marca](TRADEMARK_POLICY.md).

Si tu aporte incluye material generado con IA, indícalo en el PR. Detalles en [`docs/LICENSING.md`](docs/LICENSING.md).

La CI comprueba el DCO en cada pull request (`tools/check_dco.py`, sin acciones de terceros): si falta la firma en algún commit, arréglalo con `git commit --amend -s` (o `git rebase --signoff` para varios) y vuelve a subir. Para probarlo en local: `python -I tools/check_dco.py --self-test`.

### Comprobación de privacidad
Miel no lee teclas, pantallas, títulos de ventana ni archivos, y no usa red. La CI ejecuta `tools/check_privacy.py`, que falla si aparece una API prohibida, un permiso nuevo en `capabilities/`, una API de `windows` no aprobada o una dependencia nueva con capacidad de red. Es un control **preventivo** (ver [`docs/PRIVACY.md`](docs/PRIVACY.md)). En local:
```
python -I tools/check_privacy.py              # completa (necesita cargo y `npm ci`)
python -I tools/check_privacy.py --self-test  # prueba negativa: debe fallar ante entradas indebidas
```
Si tu cambio necesita legítimamente un permiso, una API o una dependencia nueva, explícalo en el PR: las listas (`tools/privacy_policy.json`, `tools/privacy_baseline.json`) solo se amplían tras revisión de privacidad.

---

# English

Thanks for wanting to help! Miel is a local, discreet desktop bee. Anyone can contribute; you do not need prior Rust or Tauri experience to improve docs, phrases, translations or the character design.

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md). Report **vulnerabilities** privately (see [`SECURITY.md`](SECURITY.md)), never in a public issue.

## Before you start

1. Read [`ROADMAP.md`](ROADMAP.md) (in Spanish), especially the **principles**: local first, every capability opt-in, no telemetry, never reading what you type or capturing the screen, discreet and non-manipulative. **A contribution that breaks them is not accepted, even if it works.**
2. Search for an existing issue. For anything beyond a small fix, **open an issue first** and wait for agreement.
3. Look for `good first issue` and `help wanted`.

Out of scope for v0.1.x: new features (Milestone A starts after the launch) and architecture changes without prior discussion.

## Setup

Windows 11 x64, Node.js 22+, stable Rust (≥ 1.82) with `x86_64-pc-windows-msvc`, Visual Studio Build Tools (MSVC), WebView2, and Python 3 only if you touch the character or licenses.

```powershell
npm install
npm run tauri dev
```

## Before opening a pull request

All of this must pass (CI runs it on Windows):

```powershell
npx tsc --noEmit
npm test
npm run build            # includes check:dist
cargo test   --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Project rules:

- **Rust is the single source of truth** for behavior; React only draws. Logic lives in pure functions with **injected clock and randomness**. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
- **Every change ships with tests** and, for behavior changes, a note in [`docs/TEST_LOG.md`](docs/TEST_LOG.md). **Never mark a manual test as done if you did not do it.**
- **Strict CSP**: production forbids inline styles/scripts and is stricter than `tauri dev`; jsdom/Vitest do not apply it either. If you touch styles or SVG, also check the production binary and keep `check:dist` green. Do not add `'unsafe-inline'` or `'unsafe-eval'`.
- **No network, no telemetry** in the default build. New dependencies must be justified in the PR (license, size, upkeep) and reflected in [`docs/THIRD_PARTY.md`](docs/THIRD_PARTY.md). **If `Cargo.lock` or `package-lock.json` changes (any dependency update), regenerate `THIRD_PARTY_LICENSES.txt`** with `python -I tools/gen_third_party_licenses.py` (needs `cargo-about` and `npm ci`); CI fails if it does not match the lockfiles.
- **Minimal frontend capabilities**; do not widen them without a stated reason.
- **Performance**: Miel must be able to sleep when idle. No polling or animations that burn CPU while paused or hidden without measuring.

## Character (SVG)

**Do not hand-edit** the SVGs in `src/assets/bee/`; they are generated from [`tools/build_bee.py`](tools/build_bee.py), the single source of truth. After changing the generator run `python -I tools/build_bee.py` and `python -I tools/make_nostyle_rig.py`. Follow [`docs/BEE_GUIDE.md`](docs/BEE_GUIDE.md). New faces must be **original**: no copies or derivatives of third-party characters, artwork or trademarks.

## Phrases

The catalog lives in `src-tauri/src/mascot/messages.rs`. Tone: tender, discreet, **never scolding, pressuring, or assuming what the person is doing**; **at most 64 characters**. Phrases are currently Spanish-only; translations are welcome once phrases become data (see ROADMAP).

## Screenshots and images

**Do not upload screenshots of your desktop** or images showing paths, user names, other apps' windows or private content. Use a plain background or crop only Miel's window, and review before attaching (also in issues).

## Commits and pull requests

Small, focused changes; one PR, one purpose. Clear commit messages (Spanish or English), imperative, with the why. Fill in the PR template and update [`CHANGELOG.md`](CHANGELOG.md). A maintainer will review; please be patient.

## Contribution licensing and DCO

There is no CLA. Every commit must carry a `Signed-off-by: Name <email>` line (use `git commit -s`), certifying the [Developer Certificate of Origin 1.1](DCO.md): that you have the right to submit the change under the file's license.

Your contribution is published under the license of the component you modify:
- code, tooling and functional CSS → **MIT OR Apache-2.0** (at the recipient's option);
- art, character, stickers and documentation → **CC BY 4.0**;
- Miel's **name and visual identity** (name, BKODE, logo and icons) are not licensed and are governed by the [trademark policy](TRADEMARK_POLICY.md).

If your contribution includes AI-generated material, say so in the PR. Details in [`docs/LICENSING.md`](docs/LICENSING.md).

CI checks the DCO on every pull request (`tools/check_dco.py`, no third-party actions): if a commit lacks the sign-off, fix it with `git commit --amend -s` (or `git rebase --signoff` for several) and push again. To try it locally: `python -I tools/check_dco.py --self-test`.

### Privacy check
Miel does not read keys, screens, window titles or files, and uses no network. CI runs `tools/check_privacy.py`, which fails on a forbidden API, a new permission in `capabilities/`, an unapproved `windows` API or a new network-capable dependency. It is a **preventive** control (see [`docs/PRIVACY.md`](docs/PRIVACY.md)). Locally:
```
python -I tools/check_privacy.py              # full (needs cargo and `npm ci`)
python -I tools/check_privacy.py --self-test  # negative test: must fail on bad inputs
```
If your change legitimately needs a new permission, API or dependency, explain it in the PR: the lists (`tools/privacy_policy.json`, `tools/privacy_baseline.json`) are only extended after a privacy review.
