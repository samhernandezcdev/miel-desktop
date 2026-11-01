# Licencias: propuesta revisada (NO aplicada)

> Este documento es una **propuesta** para que la persona titular decida. **No se ha añadido ningún archivo `LICENSE`**
> ni se ha declarado una licencia definitiva en `package.json`, `Cargo.toml` o el README. No es asesoría legal:
> conviene que una persona con formación jurídica revise los puntos de "Por confirmar" antes de publicar.

## Propuesta para v0.1.0 (Hito 0)

| Parte | Licencia propuesta | Estado |
|---|---|---|
| **Código** (`src/`, `src-tauri/src/`, `src-tauri/tests/`, `tools/`, `tests/`, configuración) | **MIT** | Por confirmar titularidad |
| **Recursos originales de Miel** (SVG y CSS del personaje, paleta, hojas de estados, imágenes de `docs/`, iconos de la aplicación) | **CC BY 4.0** | Por confirmar titularidad y alcance |
| **Nombre "Miel", logotipo e identidad visual** | **Política de marca** separada (borrador: [`legal-drafts/TRADEMARK_POLICY.draft.md`](legal-drafts/TRADEMARK_POLICY.draft.md)) | Por decidir; nombre sin revisar en registros |
| **Dependencias de terceros** | Las suyas (ver [`THIRD_PARTY.md`](THIRD_PARTY.md)) | Mantener avisos |
| **Documentación** (README, docs/*.md) | Sin decidir (opciones: misma licencia que el código, o CC BY 4.0) | Por decidir |

Cambio respecto a la propuesta anterior: antes se sugería `MIT OR Apache-2.0`. Con **MIT** a secas se gana simplicidad
y se pierde la concesión expresa de patentes de Apache-2.0; es una decisión aceptable, pero es una decisión.

### Cómo encajan las tres capas

- **MIT (código)** exige conservar el aviso de copyright y el texto de la licencia en las copias del código.
- **CC BY 4.0 (recursos)** permite usar, modificar y vender el personaje (también con fines comerciales) citando autoría, indicando si hubo cambios y enlazando la licencia. Máxima difusión, **sin control sobre usos comerciales** ni derivados. Creative Commons no recomienda sus licencias para software, pero para ilustraciones sí es el uso previsto.
- **Las licencias abiertas no conceden derechos de marca.** CC BY 4.0 lo dice expresamente (no se licencian marcas ni derechos de personalidad) y MIT no menciona marcas. Por eso el nombre y la identidad visual van con una política aparte: se puede copiar y modificar el código y el dibujo, pero no **hacerse pasar por** el proyecto oficial ni sugerir su respaldo.
- Fronteras a definir: `tools/build_bee.py` es **código** (MIT) pero su **salida** (los SVG) son **recursos** (CC BY 4.0); `miel-bee-mascot/preview/index.html` y `tools/preview_template.html` mezclan ambos; `src-tauri/icons/` es el personaje convertido a iconos (recurso, pero también parte de la identidad: **marca**).

## Por confirmar antes de declarar nada definitivo

1. **Titularidad (quién tiene el copyright).** Decidir persona o entidad y el año. No se ha inventado ninguno. Si alguna parte se desarrolló bajo una relación laboral, con recursos de un empleador o por encargo, confirmar que no hay derechos de terceros.
2. **Autoría humana y herramientas de IA.** El proyecto se desarrolló con asistencia de herramientas de IA (código y generación del personaje desde `tools/build_bee.py`). Según la jurisdicción, la protección por derechos de autor de material generado o asistido por IA puede ser limitada o incierta, y eso afecta a lo que realmente se puede licenciar. Conviene dejar constancia de la contribución creativa humana (decisiones de diseño, dirección, edición) y valorar si se menciona la asistencia en la documentación.
3. **Originalidad del personaje.** [`BEE_GUIDE.md`](BEE_GUIDE.md) §8 indica que se construyó desde cero, pero **eso no garantiza la ausencia de conflictos**. Buscar similitudes con mascotas existentes antes de promocionarla.
4. **Nombre "Miel".** Es una palabra común (miel en español); revisar registros de marca en México y en los países de distribución, y el uso de "bkode"/"BKODE". Pendiente además confirmar que se controla `bkode.lat`, de donde sale el identificador `lat.bkode.miel`.
5. **Atribución del recurso (CC BY 4.0).** Definir el texto exacto de atribución (por ejemplo "Miel, por <titular>, CC BY 4.0") y cómo se pide cuando alguien reutiliza el personaje.
6. **Contribuciones externas.** Se propone que la contribución se publique bajo las licencias del proyecto (entrada = salida), sin cesión de derechos. Si se quiere poder cambiar de licencia en el futuro hace falta un acuerdo de contribución (CLA) o el consentimiento de cada autora o autor. Decidirlo **antes** de aceptar la primera contribución.
7. **Fuentes tipográficas y otros recursos.** El proyecto no incluye fuentes: la burbuja usa fuentes del sistema (`Segoe UI Variable Text`, `Segoe UI`). No se encontraron otros recursos de terceros en el repositorio; revisar de nuevo si se añaden.
8. **Texto íntegro de las licencias.** Para publicar hacen falta los textos oficiales: MIT (corto, ver borrador) y el *legal code* de CC BY 4.0 (descargarlo de creativecommons.org; no se reproduce aquí).

## Qué hay en el repositorio (inventario)

| Parte | Origen | Observaciones |
|---|---|---|
| Código de la aplicación (`src/`, `src-tauri/`, `tools/`, `tests/`) | Escrito para este proyecto | Libre de elegir licencia (ver "Por confirmar" 1 y 2) |
| Personaje Miel (`src/assets/bee/*`, `miel-bee-mascot/`, `docs/sheet-*.png`, `docs/img/*`, `src-tauri/icons/*`) | Diseño original generado desde `tools/build_bee.py` | `src/assets/bee/*` y `miel-bee-mascot/assets/bee/*` son copias idénticas (verificado con `cmp`), salvo `bee-rig.nostyle.svg`, derivado solo de `src/` |
| Dependencias de Rust y npm | Terceros | Ver [`THIRD_PARTY.md`](THIRD_PARTY.md) |
| Runtime WebView2 | Microsoft, **no se redistribuye** con el instalador | Se descarga del sistema o con el *bootstrapper* de Microsoft |
| NSIS (empaquetado del instalador) | Terceros (Nullsoft Scriptable Install System) | El instalador generado incluye componentes de NSIS; revisar su licencia al redistribuir |

## Antes de publicar

- Resolver los puntos de "Por confirmar".
- Añadir `LICENSE` (código) y `LICENSE-ASSETS` (recursos) con el texto oficial, un `NOTICE` o sección de atribuciones, y la política de marca final (`TRADEMARK.md`). Hasta entonces, no declarar licencia en `package.json`/`Cargo.toml`.
- Los borradores están en [`legal-drafts/`](legal-drafts/); **no son definitivos** y llevan marcadores `[...]` por rellenar.
- Mantener [`THIRD_PARTY.md`](THIRD_PARTY.md) al día (`python -I tools/third_party.py`) en cada versión.
- El instalador NSIS incluye el ejecutable con sus dependencias enlazadas estáticamente: al **redistribuirlo** hay que cumplir los avisos de atribución de todas ellas (ver [`THIRD_PARTY_NOTES.md`](THIRD_PARTY_NOTES.md)).

## Identificador de aplicación

Definitivo: **`lat.bkode.miel`** (dominio de referencia `bkode.lat`, marca BKODE). Ver `docs/ARCHITECTURE.md`,
*Identidad de la aplicación*, para su impacto en preferencias, WebView2, instancia única, instalador y
actualizaciones. **Pendiente** confirmar el control de `bkode.lat`. El titular del copyright y la licencia siguen
**sin decidir** (esta propuesta no los fija).
