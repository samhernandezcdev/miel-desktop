# Dependencias de terceros: notas y obligaciones

Complementa [`THIRD_PARTY.md`](THIRD_PARTY.md), que se **genera** con `python -I tools/third_party.py` (229 crates de Rust de
ejecución y 3 paquetes npm de ejecución; se comprobó que la lista vigente coincide con la regenerada a partir de
`Cargo.lock` y `node_modules`). Esa lista y sus avisos **se conservan sin cambios**; este documento añade lo que el
generador no cubre. **No es asesoría legal ni sustituye a los textos de licencia originales.**

## Qué entra en el instalador

| Componente | Dónde | Licencia | Nota |
|---|---|---|---|
| Crates de Rust (229) | Enlazados estáticamente en `miel.exe` | Ver `THIRD_PARTY.md` (predominan MIT, Apache-2.0 y dobles) | Se redistribuyen en el instalador |
| `react`, `react-dom`, `@tauri-apps/api` | Empaquetados en el JS del frontend | MIT / MIT / Apache-2.0 OR MIT | Se redistribuyen en el instalador |
| NSIS 3.13 (Nullsoft Scriptable Install System) y plugin `nsis_tauri_utils` | Parte del instalador generado por Tauri (compresión LZMA sólida; verificado) | NSIS: zlib/libpng y **CPL 1.0** para el módulo LZMA (bzip2 no incrustado). `nsis_tauri_utils`: Apache-2.0 OR MIT | Avisos aún incompletos en `THIRD_PARTY_LICENSES.txt`; ver `LICENSING.md` (sección NSIS) |
| Runtime **WebView2** | **No** se incluye; lo aporta Windows o el *bootstrapper* de Microsoft | Licencia de Microsoft | Miel no lo redistribuye |
| Fuentes | **No** se incluyen; la burbuja usa fuentes del sistema (`Segoe UI`) | n/a | |

## Dependencias de desarrollo (no se redistribuyen)

No forman parte del instalador; se usan para compilar y probar.

| Paquete | Versión | Licencia |
|---|---|---|
| @tauri-apps/cli | 2.12.4 | Apache-2.0 OR MIT |
| @testing-library/dom | 10.4.2 | MIT |
| @testing-library/react | 16.3.3 | MIT |
| @types/node | 26.6.6 | MIT |
| @types/react | 19.3.0 | MIT |
| @types/react-dom | 19.3.0 | MIT |
| @vitejs/plugin-react | 6.1.2 | MIT |
| jsdom | 30.1.3 | MIT |
| typescript | 5.9.3 | Apache-2.0 |
| vite | 8.3.5 | MIT |
| vite-plugin-svgr | 5.2.0 | MIT |
| vitest | 5.0.5 | MIT |

También hay dependencias transitivas de npm (solo de desarrollo) y de Cargo de desarrollo/compilación (`tauri-build`,
`tauri` con la característica `test`); no se listan una a una. Las versiones exactas están en `package-lock.json` y
`src-tauri/Cargo.lock`.

## Licencias que requieren atención al redistribuir

- **MPL-2.0** (5 crates: `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors`): copyleft **a nivel de archivo**.
  Se usan **sin modificar** como dependencias; la obligación práctica es conservar los avisos y poder indicar dónde obtener su
  código fuente (los repositorios de cada crate en crates.io). Si algún día se modifican esos archivos, los cambios en
  ellos deben publicarse bajo MPL-2.0. Es compatible con publicar el resto del código bajo MIT.
- **BSD-3-Clause** (`alloc-no-stdlib`, `alloc-stdlib`, `brotli`, `brotli-decompressor`, entre otras): al distribuir binarios hay que
  **reproducir el aviso de copyright y el texto de la licencia** en la documentación o materiales que acompañen al binario.
- **MIT**, **Apache-2.0**, **Unicode-3.0**, **Zlib**, **0BSD**, **Unlicense/CC0** y las dobles (`MIT OR Apache-2.0`…): MIT, Apache-2.0
  y Unicode-3.0 exigen conservar los avisos de copyright y el texto de licencia en las redistribuciones; Apache-2.0 además exige
  conservar archivos `NOTICE` cuando existan. Las dobles se pueden acoger a cualquiera de las dos alternativas.
- Si alguna dependencia tiene el campo `license` vacío o ambiguo, `THIRD_PARTY.md` lo marca como "(sin campo license)": **no hay ninguna** en la lista actual.

## Avisos de licencia dentro del instalador: `THIRD_PARTY_LICENSES.txt`

Hasta ahora el instalador contenía código de las dependencias anteriores sin sus textos de licencia ni avisos de copyright
(`THIRD_PARTY.md` solo lista nombre, versión y licencia). La brecha se cierra con `THIRD_PARTY_LICENSES.txt` (raíz del repositorio):

- **Qué contiene:** el texto completo de licencia y los avisos de copyright de las crates de Rust compiladas para Windows x64
  (209: las 135 realmente enlazadas en `miel.exe` más las macros de compilación, por prudencia) y de los 4 paquetes npm del
  frontend de producción (`react`, `react-dom`, `scheduler`, `@tauri-apps/api`), la sección de código fuente de las 5 crates
  MPL-2.0 y los otros componentes (NSIS, WebView2 no incluido).
- **Cómo se genera:** `python -I tools/gen_third_party_licenses.py` (Rust con `cargo-about` 0.9.3, configuración en
  `tools/about.toml` y `tools/about.hbs`; npm leyendo los archivos de licencia de `node_modules`). Es reproducible: `--verify`
  regenera en memoria y compara byte a byte. Requisitos: `cargo install cargo-about --locked --features cli --version 0.9.3` y `npm ci`.
- **Comprobaciones del propio script:** todas las crates enlazadas según `cargo tree -e normal,no-proc-macro` aparecen en la salida;
  cada paquete npm tiene archivo de licencia; las crates MPL-2.0 se verifican como no modificadas (ver abajo).
- **CI:** el job `licenses` ejecuta `--check`, que falla si `Cargo.lock` o `package-lock.json` cambian sin regenerar el archivo
  (el archivo registra el SHA-256 de ambos, con saltos de línea normalizados). Cualquier actualización de dependencias debe regenerarlo (pasos en `docs/DEPENDENCY_UPDATES.md`; Dependabot no está activado).
- **Instalador:** se incluye como recurso de Tauri (`bundle.resources` en `src-tauri/tauri.conf.json`), por lo que se instala junto a
  `miel.exe` en `%LOCALAPPDATA%\Miel\THIRD_PARTY_LICENSES.txt`. También se adjunta a cada release.

### Crates MPL-2.0 (código fuente y no modificación)

`cssparser` 0.37.0, `cssparser-macros` 0.7.1, `dtoa-short` 0.3.5, `option-ext` 0.2.0 y `selectors` 0.38.0. Los enlaces al código
fuente (`.crate` en static.crates.io, página de crates.io y repositorio) están en la primera sección de `THIRD_PARTY_LICENSES.txt`.
**No están modificadas**, comprobado al generar el archivo: el `.crate` descargado coincide con el checksum de `Cargo.lock`, el código
extraído coincide con el contenido del `.crate`, y `src-tauri/Cargo.toml` no tiene `[patch]`, `[replace]` ni dependencias por ruta.
Esto demuestra que lo compilado partió de lo publicado en crates.io; no prueba nada sobre el binario final más allá de eso.

### Lo que sigue sin hacer o sin garantizar

- `THIRD_PARTY.md` (229 crates, generado por `tools/third_party.py`) es un **superconjunto** del archivo anterior: usa el grafo de
  `cargo metadata`, que unifica las características de desarrollo (p. ej. `tauri/test`) e incluye crates que no se compilan en el binario
  (`chrono`, `defmt`, `bs58`, `hashbrown 0.12.3`, `indexmap 1.9.3`…). `THIRD_PARTY_LICENSES.txt` es la referencia de lo que se redistribuye.
- La salida de `cargo-about` depende de los archivos de licencia que cada crate publica; no se revisó a mano cada texto. Sigue siendo
  una ayuda, **no asesoría legal**.
- La licencia del propio Miel no está en ese archivo (aún sin decidir; ver `LICENSING.md`).

## Mantenimiento

- Regenerar `THIRD_PARTY.md` (`python -I tools/third_party.py`) y `THIRD_PARTY_LICENSES.txt` (`python -I tools/gen_third_party_licenses.py`) en cada versión y siempre que cambien los lockfiles.
- Revisar licencias nuevas al actualizar dependencias (ver `docs/DEPENDENCY_UPDATES.md`).
- Pendiente: auditoría de vulnerabilidades (`cargo audit` / `cargo deny`, `npm audit`): **no se ha ejecutado** (ver `docs/TEST_LOG.md`).
