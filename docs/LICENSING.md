# Licencias de Miel

> No es asesoría legal. Titular: **Copyright © 2026 Samuel David Hernández Contreras (BKODE)**. Decisiones aprobadas por la persona titular el 2026-11-08.
> Quedan decisiones abiertas (ver "Pendiente"); ninguna impide la publicación según esta documentación, pero conviene una revisión jurídica.

## Resumen

| Parte | Licencia | Archivo |
|---|---|---|
| Código (Rust, TypeScript, React, Python, PowerShell), herramientas, CSS funcional | **MIT OR Apache-2.0** (a elección de quien lo recibe) | [`LICENSE-MIT`](../LICENSE-MIT), [`LICENSE-APACHE`](../LICENSE-APACHE) |
| Arte, personaje, *stickers* y documentación original | **CC BY 4.0** | [`LICENSE-ASSETS`](../LICENSE-ASSETS) |
| Nombre "Miel", "BKODE", logotipo, iconos de la aplicación e identidad oficial | **Sin licenciar**; política de marca | [`TRADEMARK_POLICY.md`](../TRADEMARK_POLICY.md) |
| Dependencias y componentes de terceros (incluido NSIS) | Las suyas | [`THIRD_PARTY_LICENSES.txt`](../THIRD_PARTY_LICENSES.txt), [`THIRD_PARTY.md`](THIRD_PARTY.md) |
| Contribuciones | DCO (`Signed-off-by`), sin CLA | [`DCO.md`](../DCO.md), [`CONTRIBUTING.md`](../CONTRIBUTING.md) |

`NOTICE` en la raíz resume el reparto y los archivos mixtos.

## Delimitación por componente

| Componente | Licencia | Observación |
|---|---|---|
| `src/**/*.ts(x)`, `src-tauri/src/`, `src-tauri/tests/`, `src-tauri/build.rs`, `tests/`, `vite.config.ts`, `tsconfig.json`, `index.html` | MIT OR Apache-2.0 | Código propio. |
| `tools/*` | MIT OR Apache-2.0 | `tools/build_bee.py` es código; **su salida SVG** es arte. |
| CSS funcional: `src/styles/app.css`, `src/components/**/*.css`, `bee.css` | MIT OR Apache-2.0 | `bee.css` (animaciones y tabla de estados) lo genera `build_bee.py`. |
| `src/assets/bee/*.svg`, `palette.json` | CC BY 4.0 | Arte original. `bee-rig.nostyle.svg` deriva de `bee-rig.svg` (misma licencia). |
| `bee-rig.svg` (y `.nostyle`) | CC BY 4.0, archivo completo | Su `<style>` embebido (5 115 B) es idéntico a `bee.css`; dentro del SVG se distribuye bajo CC BY 4.0, como archivo independiente es código. No se añaden comentarios a los SVG: la atribución vive en `NOTICE` y `LICENSE-ASSETS`. |
| `miel-bee-mascot/assets/bee/*` | Igual que `src/assets/bee/*` | Copia idéntica (verificada con `diff`), salvo `bee-rig.nostyle.svg`. |
| `miel-bee-mascot/tools/*` | MIT OR Apache-2.0 | Copias de herramientas. |
| `miel-bee-mascot/preview/index.html` | Mixto | SVG: CC BY 4.0; código y CSS de la página: MIT OR Apache-2.0. Es un archivo generado. |
| `miel-bee-mascot/docs/*`, `docs/sheet-*.png`, `docs/img/*` | CC BY 4.0 | `docs/img/*` son capturas de la aplicación. |
| **`src-tauri/icons/*`** | **Sin licenciar** (identidad) | Ver "Iconos". |
| Documentación original (`README`, `docs/*.md`, `ROADMAP`, `CHANGELOG`...) | CC BY 4.0 | Excepto fragmentos de código (licencia de código) y textos de terceros. |
| `CODE_OF_CONDUCT.md` | Derivado del Contributor Covenant 2.1 (CC BY 4.0) | Ya declara su origen; conservar la atribución. |
| `THIRD_PARTY_LICENSES.txt`, `docs/THIRD_PARTY.md` | Avisos de terceros | Generados; no se relicencian. |
| `LICENSE-*`, `DCO.md` | Textos oficiales | Copiados sin modificar de apache.org, creativecommons.org y developercertificate.org (consultados el 2026-11-08). |

## Compatibilidad con terceros (verificada)

- `npm run licenses:check` pasa. El copyleft de terceros que hay es **MPL-2.0** (por archivo; compatible con MIT/Apache-2.0, código fuente enlazado en el archivo de avisos, sin modificaciones por parte de Miel). No hay GPL, AGPL ni CC BY-SA.
- No se incluyen fuentes tipográficas ni se redistribuye WebView2.
- Con Apache-2.0 como opción, la concesión expresa de patentes cubre el código de Miel.

## Instalador y NSIS (verificado)

El instalador se genera con **NSIS 3.13**, compresión **LZMA sólida** (`SetCompressor /SOLID "lzma"`; el stub `lzma_solid-x86-unicode` coincide 48/48 con el instalador y el registro `makensis -V4` de la sesión del 2026-11-01 confirma `Using lzma (compress whole)`). Plugins incrustados: `System`, `nsDialogs`, `NSISdl` y `nsis_tauri_utils` (MIT OR Apache-2.0, © Tauri Programme within The Commons Conservancy).

| Componente del instalador | Aviso | Dónde |
|---|---|---|
| NSIS: código, plugins y cabeceras (zlib/libpng) | Reproducido | `THIRD_PARTY_LICENSES.txt`, sección 4 |
| NSIS: módulo LZMA (Common Public License 1.0, con la excepción especial de LZMA) | Reproducido | idem |
| NSIS: módulo bzip2 | Reproducido por prudencia (no está incrustado con compresión LZMA) | idem |
| `nsis_tauri_utils` (MIT OR Apache-2.0) | Reproducido (texto MIT) | idem |
| WebView2 | No se redistribuye; `NSISdl` lo descarga de Microsoft si falta | idem |

El texto de NSIS proviene de su archivo `COPYING` (© 1999-2026 Contributors) y está en `tools/nsis-notices.txt`; lo incorpora `tools/gen_third_party_licenses.py`.
Limitación: el log `makensis` disponible es de una sesión anterior y su tamaño (1 276 934 B) es cercano pero no idéntico al del instalador provisional (1 276 357 B); para el instalador definitivo hay que generar y conservar un log propio.

## Iconos (`src-tauri/icons/*`)

Evidencia revisada: seis archivos añadidos en el commit inicial, sin metadatos de herramienta ni de autor; muestran la abeja de Miel con los colores exactos de `palette.json`; ningún script del repositorio los genera. Son un derivado visual del personaje, pero **no hay registro de la herramienta ni del proceso**. Decisión adoptada: se tratan como **logotipo/identidad** (sin licenciar, regidos por la política de marca). El arte del personaje en sí (SVG) sí va bajo CC BY 4.0.

## Asistencia de IA

Parte del código, las pruebas y la documentación se desarrolló con asistencia de herramientas de IA generativa (Claude, de Anthropic); el personaje se genera con un programa propio (`tools/build_bee.py`). Según la jurisdicción, el material generado íntegramente por IA puede no estar protegido por derechos de autor. Por eso las licencias se aplican **en la medida en que existan derechos** y **no se afirma titularidad exclusiva** sobre resultados íntegramente generados por IA. Si la persona titular quiere describir su aportación creativa (dirección, selección, edición, escritura propia), puede añadirse a esta sección y a `NOTICE`.

## Contribuciones

Sin CLA. Cada contribución se firma con el *Developer Certificate of Origin* 1.1 (`git commit -s` → `Signed-off-by`) y se publica bajo la licencia del componente que modifica: código y herramientas → MIT OR Apache-2.0; arte y documentación → CC BY 4.0. Consecuencia: cambiar de licencia en el futuro exigiría el consentimiento de cada contribuyente. Las contribuciones con material generado por IA deben declararlo en el PR. Ver [`CONTRIBUTING.md`](../CONTRIBUTING.md) y [`DCO.md`](../DCO.md).

## Pendiente (decisiones de la persona titular)

- [ ] **Marca:** búsqueda de anterioridades de "Miel" y "BKODE" y decisión sobre registrarlas; mientras tanto no se afirma ningún registro. Confirmar el control de `bkode.lat`.
- [ ] **Contacto de marca:** hoy remite a los *issues* del repositorio; decidir si se publica un correo.
- [ ] **Iconos:** confirmar que se consideran logotipo (sin licenciar) y, si se quiere, documentar su origen.
- [ ] **Originalidad del personaje:** buscar similitudes con mascotas existentes antes de promocionarlo ([`BEE_GUIDE.md`](BEE_GUIDE.md) §8 afirma construcción desde cero, lo que no garantiza ausencia de conflictos).
- [ ] **Licencia de Miel dentro del instalador:** hoy el instalador solo lleva `THIRD_PARTY_LICENSES.txt`; decidir si también `LICENSE-MIT`/`LICENSE-APACHE` (cambia `tauri.conf.json`).
- [ ] **DCO:** decidir si se verifica automáticamente (aplicación de GitHub o comprobación en CI).
- [ ] **`ROADMAP.md` y `CHANGELOG.md`:** reflejar las licencias en el Hito 0 y en la sección 0.1.0 (no se tocaron porque tienen cambios de v0.2.0 sin confirmar).
- [ ] Revisión jurídica.

## Identificador de aplicación

**`lat.bkode.miel`** (dominio de referencia `bkode.lat`; ver `docs/ARCHITECTURE.md`). Pendiente confirmar el control de `bkode.lat`.
