# Licencias: arquitectura aprobada, titularidad pendiente

> **Estado: NO DEFINITIVO.** La arquitectura de licencias fue aprobada el 2026-11-03, pero **no se ha añadido ningún
> `LICENSE`**, ni se declara una licencia en `package.json`, `Cargo.toml` o el README, porque **la persona o entidad
> titular del copyright no está confirmada** y no se ha verificado que existan derechos suficientes sobre cada recurso.
> Este documento no es asesoría legal; conviene una revisión jurídica antes de publicar.
> Los textos preparados están en [`legal-drafts/`](legal-drafts/) y llevan marcadores `[...]` por rellenar.

## Decisiones aprobadas (2026-11-03)

| # | Decisión |
|---|---|
| 1 | Año inicial del aviso de copyright: **2026**. |
| 2 | **Titular jurídico: pendiente de confirmar expresamente.** No se sustituye por BKODE ni por un nombre supuesto. |
| 3 | Código (Rust, TypeScript, React, Python), herramientas y CSS funcional: **MIT**. |
| 4 | Recursos visuales originales **sobre los que haya derechos suficientes**: **CC BY 4.0**. |
| 5 | Documentación original: **CC BY 4.0**, salvo fragmentos de código (MIT) y materiales de terceros. |
| 6 | Nombre e identidad oficial de **Miel** y de **BKODE**: política de marca separada. **No se afirma ningún registro de marca** que no esté comprobado. |
| 7 | Contribuciones: **sin CLA** inicialmente. Cada contribución se acepta bajo la licencia del componente que modifica. |
| 8 | Conservar `THIRD_PARTY_LICENSES.txt` y verificar los avisos de NSIS (ver más abajo). |
| 9 | Delimitar `miel-bee-mascot/`, `src-tauri/icons/` y los archivos generados (ver tabla). |
| 10 | Documentar con precisión la asistencia de IA, sin atribuir derechos exclusivos sobre resultados íntegramente generados por IA. |

## Delimitación por componente

| Componente | Licencia propuesta | Condición / observación |
|---|---|---|
| `src/**/*.ts`, `*.tsx`, `src-tauri/src/`, `src-tauri/tests/`, `src-tauri/build.rs`, `tests/`, `vite.config.ts`, `tsconfig.json`, `index.html` | MIT | Código propio. |
| `tools/*` (Python, PowerShell, Node, plantillas `*.hbs`/`*.toml`) | MIT | `tools/build_bee.py` es código; **su salida** (SVG) es recurso. |
| CSS funcional: `src/styles/app.css`, `src/components/**/*.css`, y `bee.css` | MIT | `bee.css` (tabla de estados y animaciones) es funcional; se genera con `build_bee.py`. Si el CSS va **embebido dentro de un SVG** (`bee-rig.svg`), el archivo SVG completo se rige por CC BY 4.0. |
| `src/assets/bee/*.svg`, `palette.json` | CC BY 4.0 | Recursos originales. Incluye `bee-rig.nostyle.svg` (derivado de `bee-rig.svg` por `make_nostyle_rig.py`; misma licencia que su origen). |
| `miel-bee-mascot/assets/bee/*` | Igual que `src/assets/bee/*` | Copia idéntica (verificada con `diff`), salvo `bee-rig.nostyle.svg`. |
| `miel-bee-mascot/tools/*` | MIT | Copias de herramientas. |
| `miel-bee-mascot/preview/index.html` | **Mixto** | Archivo generado que incrusta SVG (CC BY 4.0) y código/CSS (MIT). Se documenta como mixto en `NOTICE`. |
| `miel-bee-mascot/docs/*`, `docs/sheet-*.png`, `docs/img/*` | CC BY 4.0 | Imágenes generadas con el personaje; `docs/img/*` son capturas de la aplicación: confirmar que no muestran contenido de terceros. |
| `src-tauri/icons/*` | **Pendiente** | Aparentan derivar del personaje (colores idénticos a `palette.json`), pero **sin evidencia de herramienta, autor ni proceso**; ver la sección de procedencia. Además son identidad (marca). |
| Archivos generados: `THIRD_PARTY_LICENSES.txt`, `docs/THIRD_PARTY.md` | Avisos de terceros | No se relicencian: reproducen licencias ajenas. No editar a mano. |
| Salida de compilación (`dist/`, `src-tauri/target*/`, instaladores) | n/a | No se versionan. El instalador contiene código MIT, dependencias de terceros y recursos CC BY. |
| Documentación original (`README`, `docs/*.md`, `ROADMAP`, `CHANGELOG`, etc.) | CC BY 4.0 | Excepto fragmentos de código (MIT), textos de licencia (los de sus autores) y avisos de terceros. |
| `CODE_OF_CONDUCT.md` | Derivado de terceros (Contributor Covenant 2.1, CC BY 4.0) | Ya declara su origen y licencia en la línea 3; conservar esa atribución y no relicenciarlo como obra propia. |
| Nombre "Miel", "BKODE", logotipo, identidad visual | Política de marca | Borrador en [`legal-drafts/TRADEMARK_POLICY.draft.md`](legal-drafts/TRADEMARK_POLICY.draft.md). Sin afirmar registros. |
| Dependencias, NSIS, WebView2 | Las suyas | Ver más abajo y [`THIRD_PARTY_NOTES.md`](THIRD_PARTY_NOTES.md). |

## Cómo encajan las capas

- **MIT** exige conservar el aviso de copyright y el texto de la licencia en las copias del código. No dice nada de marcas.
- **CC BY 4.0** permite usar, modificar y vender el personaje (incluso con fines comerciales) citando autoría, indicando si hubo
  cambios y enlazando la licencia. No licencia marcas ni derechos de personalidad. **Sin control sobre usos comerciales.**
- **Marca**: se puede copiar el código y el dibujo, pero no **hacerse pasar por** el proyecto oficial ni sugerir su respaldo.
- Creative Commons no recomienda sus licencias para software: por eso el código es MIT y CC BY queda para los recursos y la documentación.

## Compatibilidad con los avisos de terceros (verificado)

- `npm run licenses:check` pasa: `THIRD_PARTY_LICENSES.txt` coincide con `Cargo.lock` y `package-lock.json`. El archivo declara que **no** incluye la licencia de Miel.
- El único copyleft encontrado en la lista de terceros es **MPL-2.0** (p. ej. `cssparser`, `selectors`, `dtoa-short`, `option-ext`): su copyleft es por archivo, es compatible con un proyecto MIT y el archivo de avisos ya enlaza el código fuente (Miel no los modifica). No se encontró GPL, AGPL ni CC BY-SA.
- No hay fuentes tipográficas incluidas (la burbuja usa fuentes del sistema). WebView2 no se redistribuye.
- Ninguna de las licencias propuestas contradice las de las dependencias redistribuidas. **No es asesoría legal.**

## NSIS y avisos obligatorios del instalador (verificado 2026-11-03)

**Comprobado en esta máquina:**
- `tauri.conf.json` no fija compresión; el script generado (`src-tauri/target/release/nsis/x64/installer.nsi`) usa `SetCompressor /SOLID "lzma"`.
- El instalador de `target/release/bundle/nsis/Miel_0.1.0_x64-setup.exe` contiene la cadena `Nullsoft Install System v3.13`. Comparando bloques de 64 bytes de los *stubs* de NSIS con el instalador, coincide el `lzma_solid-x86-unicode` (48/48) y mucho menos los de bzip2 (10-11/48) y zlib (7/48): **el módulo de descompresión incrustado es LZMA**.
- Plugins que invoca el script: `System`, `nsDialogs`, `nsis_tauri_utils` (extra, de Tauri) y `NSISdl` (descarga del bootstrapper de WebView2 desde `go.microsoft.com` si falta el runtime).
- `COPYING` de NSIS (© 1999-2026 Contributors): zlib/libpng para código, plug-ins, cabeceras y gráficos; bzip2 y **CPL 1.0** (LZMA) para sus módulos.
- `nsis-tauri-utils` (repositorio de `tauri-apps`): contiene `LICENSE_APACHE-2.0` y `LICENSE_MIT` (doble licencia según los archivos del repositorio; la API de GitHub lo clasifica como Apache-2.0).
- **Registro de `makensis -V4` encontrado** (tras una búsqueda en segundo plano): `makensis-verbose.log`, de una sesión anterior (2026-11-01 08:39, en el directorio temporal de esa sesión; no versionado). Confirma: `MakeNSIS v3.13`, `SetCompressor: /SOLID lzma`, `Using lzma (compress whole) compression`, cabecera EXE de 38 912 bytes (= tamaño del stub `lzma_solid-x86-unicode`) y plugins incrustados `System.dll`, `nsis_tauri_utils.dll` y `NSISdl.dll` (más `nsDialogs`). Tamaño total 1 276 934 B, próximo al instalador provisional (1 276 357 B; **no idéntico**: no se puede asegurar que el log sea de esa compilación exacta) y distinto del de `target/` (1 258 510 B). Para el instalador definitivo hay que generar y conservar un log propio.

**Obligaciones identificadas para distribuir ese instalador** (no es asesoría legal):

| Componente | Obligación | Estado |
|---|---|---|
| Crates y paquetes npm enlazados en `miel.exe` (MIT, Apache-2.0, etc.) | Reproducir avisos de copyright y textos de licencia | Cubierto por `THIRD_PARTY_LICENSES.txt` (se instala con Miel; `licenses:check` OK) |
| Crates MPL-2.0 | Indicar dónde está el código fuente | Cubierto (enlaces en la sección 1 del archivo) |
| NSIS: código y módulo zlib (zlib/libpng) | En binarios no se exige aviso; se agradece reconocimiento | Hoy solo hay una línea con enlace; **recomendable** nombrar la licencia |
| NSIS: módulo **LZMA** (CPL 1.0) | Distribuido en forma objeto; la CPL 1.0 exige indicar que el código fuente está disponible y cómo obtenerlo, y no ofrecer garantías en nombre de otros | **No cubierto**: el aviso actual no nombra CPL 1.0. Pendiente de revisión jurídica |
| NSIS: módulo bzip2 | Su aviso aplica si se redistribuye | **No incrustado** en este instalador (stub LZMA); no exigible mientras siga `lzma` |
| `nsis_tauri_utils.dll` (Apache-2.0 OR MIT) | Conservar aviso y licencia al redistribuir el binario | **No cubierto**: no aparece en `THIRD_PARTY_LICENSES.txt` ni en `THIRD_PARTY.md` |
| WebView2 (Microsoft) | Ninguna: no se redistribuye; el instalador lo descarga de Microsoft si falta | Documentado. Nota: es la única conexión de red que hace el instalador |
| Licencia de Miel (MIT / CC BY) | Incluir `LICENSE` y avisos si se redistribuyen | Pendiente de titularidad |

Decisión pendiente: ampliar el generador de `THIRD_PARTY_LICENSES.txt` (sección 4) para nombrar CPL 1.0 (LZMA de NSIS), zlib/libpng y `nsis-tauri-utils` con sus textos. No se hace aquí porque implica cambiar una herramienta y regenerar un archivo con hash comprobado en CI.

## `src-tauri/icons/*`: procedencia (verificado 2026-11-03)

Evidencia:
- Seis archivos (`32x32`, `64x64`, `128x128`, `128x128@2x`, `icon.png` de 512 px, `icon.ico` con 6 imágenes de 16 a 256 px), todos añadidos en el único commit (`243fc91`).
- Los PNG no traen metadatos (solo `IHDR`, `IDAT`, `IEND`: sin `tEXt` de herramienta ni de autor).
- `icon.png` muestra la abeja de Miel (pose `idle`, contorno crema, antenas con hexágono y círculo, alas azules). Los píxeles opacos usan **exactamente** colores de `palette.json`: `#f7b538` (honey), `#3b2a21` (ink), `#fff1cc` (cream), `#fff7e0` (rim), `#86afcf` (wingStroke) y `#e8962a` (honeyShade).
- Ningún script del repositorio genera los iconos (`tools/*.py` no menciona `icons`), y ni `ARCHITECTURE.md` ni `BEE_GUIDE.md` ni `TEST_LOG.md` documentan cómo se hicieron.

Conclusión: **hay evidencia fuerte de que son un derivado visual del personaje original (mismos colores y misma geometría)**, pero **no hay evidencia de la herramienta, el autor ni el proceso**, ni de que sean una rasterización exacta de `bee-idle.svg` (no se comparó píxel a píxel: no hay `rsvg-convert` ni Pillow). No se presupone autoría ni derechos: la persona titular debe confirmar cómo y quién los generó. Hasta entonces: **licencia por confirmar**; además son identidad de marca.

## Frontera de licencia en `bee-rig.svg` y archivos mixtos

Comprobado: `bee-rig.svg` (19 854 B) contiene un único `<style>` de 5 115 B **idéntico a `bee.css`** (generado por `tools/build_bee.py`) y no tiene comentarios ni avisos de licencia. `bee-rig.nostyle.svg` es el mismo archivo sin `<style>`. `miel-bee-mascot/preview/index.html` (100 814 B) incrusta dos bloques `<style>` y un `<script>`, sin avisos.

Criterio propuesto:
- El **archivo SVG completo** (formas y CSS embebido) se licencia como **recurso, CC BY 4.0**: es un artefacto indivisible del personaje.
- El mismo CSS, **como archivo independiente** (`bee.css`) y dentro de la aplicación, es **MIT**. Quien lo extraiga puede usarlo bajo MIT; quien reutilice el SVG, bajo CC BY 4.0. Es el mismo texto bajo dos licencias según el envoltorio, y ambas permiten el uso libre.
- `preview/index.html` es mixto: SVG (CC BY 4.0) + código y CSS de la página (MIT).
- No se añaden comentarios a los SVG (arte original intacto). La atribución va en `NOTICE.md`, en la política de marca y en `LICENSE-ASSETS`, no dentro de los archivos.

Atribución propuesta (borrador): ver `legal-drafts/NOTICE.draft.md`.

## Asistencia de IA (declaración propuesta)

> Miel se desarrolló con asistencia de herramientas de IA generativa (Claude, de Anthropic) para escribir y revisar código,
> pruebas y documentación. El personaje se genera con un programa propio (`tools/build_bee.py`) cuyas decisiones de diseño,
> geometrías y parámetros fueron dirigidos y editados por [PERSONA TITULAR: describir su aportación creativa].

Notas:
- Según la jurisdicción, el material **generado íntegramente por IA** puede no estar protegido por derechos de autor, o estarlo de forma incierta.
  Por eso **no se afirma** que la persona titular tenga derechos exclusivos sobre resultados íntegramente generados por IA; se licencia
  **lo que se pueda licenciar** ("en la medida en que existan derechos").
- Debe dejarse constancia de la contribución humana (dirección, selección, edición, escritura propia). **Esa descripción la redacta la persona titular**;
  no se inventa aquí.
- Las licencias MIT y CC BY 4.0 pueden aplicarse a lo que sí esté protegido sin que eso suponga garantizar su protección.

## Contribuciones

- Sin CLA inicialmente. Quien contribuye confirma que es autora o autor (o tiene derecho a aportarlo) y que su aporte se publica bajo la licencia del componente que modifica: código y herramientas → MIT; recursos visuales → CC BY 4.0; documentación → CC BY 4.0.
- Consecuencia: sin CLA, **cambiar de licencia en el futuro exigiría el consentimiento de cada contribuyente**.
- Contribuciones de recursos generados con IA deben declararlo en el PR.

## Pendiente antes de declarar nada definitivo

- [ ] **Titular jurídico** (persona o entidad) confirmado expresamente por la persona responsable; relación laboral o recursos de un empleador descartados o resueltos.
- [ ] **Derechos suficientes** sobre cada recurso (personaje, iconos, capturas), incluida la descripción de la contribución humana.
- [ ] Origen de `src-tauri/icons/*`: investigado (derivado visual probable, proceso sin documentar); la persona titular debe confirmar cómo y quién los creó.
- [x] Origen de `CODE_OF_CONDUCT.md`: Contributor Covenant 2.1 (CC BY 4.0), ya atribuido en el archivo.
- [ ] Texto oficial de CC BY 4.0 (*legal code*, descargado de creativecommons.org) y texto de atribución definitivo.
- [ ] Búsqueda de anterioridades de "Miel" y "BKODE"; control de `bkode.lat`. **No se afirma ningún registro.**
- [ ] Revisión jurídica.
- [x] Módulos NSIS verificados (LZMA; ver sección).
- [ ] Decidir si se amplían los avisos (CPL 1.0, zlib/libpng, `nsis-tauri-utils`) y obtener revisión jurídica.
- [ ] Revisar de nuevo si se añaden fuentes u otros recursos de terceros.

## Cuando la titularidad esté confirmada

1. Rellenar `[TITULAR]` y crear `LICENSE` (desde `legal-drafts/LICENSE-MIT.draft.txt`), `LICENSE-ASSETS` (con el texto oficial de CC BY 4.0), `TRADEMARK.md` y `NOTICE.md` (desde `legal-drafts/`).
2. Actualizar README (sección Licencia), `PUBLISHING_CHECKLIST.md`, `RELEASE_NOTES_v0.1.0.md`, `CONTRIBUTING.md` y `ROADMAP.md`.
3. Declarar la licencia en `package.json`/`Cargo.toml` solo si se regeneran antes `package-lock.json` y `THIRD_PARTY_LICENSES.txt` (`npm run licenses`): el campo `license` aparece en el raíz del lockfile y cambia su hash.
4. Verificar la CI y crear el tag `v0.1.0` sobre el commit definitivo.

## Identificador de aplicación

Definitivo: **`lat.bkode.miel`** (dominio de referencia `bkode.lat`). Ver `docs/ARCHITECTURE.md`, *Identidad de la aplicación*.
**Pendiente** confirmar el control de `bkode.lat`. El titular del copyright **sigue sin decidirse**.

## Inventario de origen

| Parte | Origen | Observaciones |
|---|---|---|
| Código (`src/`, `src-tauri/`, `tools/`, `tests/`) | Escrito para este proyecto con asistencia de IA | Ver "Asistencia de IA" |
| Personaje (`src/assets/bee/*`, `miel-bee-mascot/`, `docs/sheet-*.png`, `docs/img/*`, `src-tauri/icons/*`) | Diseño original generado desde `tools/build_bee.py` ([`BEE_GUIDE.md`](BEE_GUIDE.md) §8: construido desde cero) | Eso **no garantiza** la ausencia de conflictos con obras existentes: buscar similitudes antes de promocionarlo |
| Dependencias Rust y npm | Terceros | [`THIRD_PARTY.md`](THIRD_PARTY.md), [`THIRD_PARTY_NOTES.md`](THIRD_PARTY_NOTES.md) |
| WebView2 | Microsoft; no se redistribuye | |
| NSIS | Terceros; parte del instalador | Ver sección NSIS |
