# BORRADOR - NO DEFINITIVO / DRAFT - NOT FINAL

> No renombrar a `NOTICE.md` hasta confirmar titularidad (ver `../LICENSING.md`). Titular: **[PENDIENTE]**. Año: 2026.

# Miel Desktop: avisos de licencia / License notices

| Qué | Dónde | Licencia propuesta |
|---|---|---|
| Código, herramientas y CSS funcional | `src/**/*.ts(x)`, `src-tauri/src`, `src-tauri/tests`, `tools/`, `tests/`, `*.css` de la aplicación, `bee.css` | MIT (`LICENSE`) |
| Recursos visuales originales | `src/assets/bee/*.svg`, `palette.json`, `miel-bee-mascot/assets/bee/*.svg`, `docs/sheet-*.png`, `docs/img/*` | CC BY 4.0 (`LICENSE-ASSETS`) |
| Archivos mixtos | `miel-bee-mascot/preview/index.html`, `bee-rig*.svg` con CSS embebido | SVG: CC BY 4.0; código incrustado: MIT |
| Iconos de la aplicación | `src-tauri/icons/*` | Por confirmar; además identidad de marca |
| Documentación original | `README`, `docs/*.md`, etc. | CC BY 4.0, salvo código (MIT) y terceros |
| Código de conducta | `CODE_OF_CONDUCT.md` | Adaptado de Contributor Covenant 2.1 (CC BY 4.0) |
| Nombre e identidad (Miel, BKODE) | n/a | Política de marca (`TRADEMARK.md`); sin registros afirmados |
| Dependencias, NSIS | `THIRD_PARTY_LICENSES.txt`, `docs/THIRD_PARTY.md` | Las suyas |

Asistencia de IA: ver `docs/LICENSING.md`, "Asistencia de IA" (texto por completar por la persona titular).

## Atribución propuesta para archivos mixtos (borrador)

- `bee-rig.svg`, `bee-rig.nostyle.svg`: «Miel, por [TITULAR], CC BY 4.0. El CSS embebido es el de `bee.css` (MIT); como parte de este SVG se distribuye bajo CC BY 4.0.»
- `miel-bee-mascot/preview/index.html`: «Página generada. SVG: CC BY 4.0 © 2026 [TITULAR]. Código y CSS de la página: MIT © 2026 [TITULAR].»
- No se modifican los SVG para añadir estos avisos; viven en este documento y en `LICENSE-ASSETS`.
- `src-tauri/icons/*`: sin licencia asignada hasta confirmar su procedencia.
- Pendiente (instalador): nombrar zlib/libpng y CPL 1.0 (NSIS 3.13, compresión LZMA) y `nsis_tauri_utils` (Apache-2.0 OR MIT) en `THIRD_PARTY_LICENSES.txt`.
