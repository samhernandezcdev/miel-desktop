# Actualizar dependencias y regenerar los avisos de licencias

`THIRD_PARTY_LICENSES.txt` se instala con Miel y se adjunta a cada release. Debe corresponder **exactamente** a
`src-tauri/Cargo.lock` y `package-lock.json`. El job `licenses` de la CI (check **obligatorio** de `main`) falla si alguno
de los dos lockfiles cambió sin regenerar el archivo: registra el SHA-256 de ambos en su cabecera y lo compara
(con saltos de línea normalizados, así que CRLF/LF no cuentan como cambio).

**Regla:** los lockfiles, `THIRD_PARTY_LICENSES.txt` y `docs/THIRD_PARTY.md` cambian **en el mismo commit / pull request**.

## Una sola vez: preparar la herramienta

```powershell
cargo install cargo-about --locked --features cli --version 0.9.3   # versión fijada en tools/gen_third_party_licenses.py
```

Sin `--features cli` el comando termina bien pero **no instala el binario**. Necesitas también Python 3 y `npm ci` hecho.

## Cada vez que cambien `Cargo.lock` o `package-lock.json`

1. Actualiza lo que quieras (`cargo update -p <crate>`, `npm update <paquete>` o editando versiones) y vuelve a instalar limpio:

   ```powershell
   npm ci
   ```

2. Regenera los avisos y la lista de dependencias:

   ```powershell
   python -I tools/gen_third_party_licenses.py     # reescribe THIRD_PARTY_LICENSES.txt (y actualiza los hashes de su cabecera)
   python -I tools/third_party.py                  # reescribe docs/THIRD_PARTY.md
   ```

3. Comprueba (es lo mismo que hará la CI):

   ```powershell
   npm run licenses:check                          # = python -I tools/gen_third_party_licenses.py --check
   python -I tools/gen_third_party_licenses.py --verify   # opcional: regenera en memoria y compara byte a byte
   ```

4. **Revisa el diff** de `THIRD_PARTY_LICENSES.txt` y `docs/THIRD_PARTY.md`:
   - ¿Aparece una licencia nueva? Si no estaba en `accepted` de `tools/about.toml`, `cargo-about` **falla a propósito**.
     Léela antes de añadirla; si no es compatible con lo que quieres redistribuir, no actualices esa dependencia.
   - ¿Cambió una de las 5 crates MPL-2.0 (`cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors`)?
     El script vuelve a comprobar que no están modificadas y actualiza los enlaces a su código fuente; revisa que sigan siendo correctos.
   - ¿Falta alguna crate enlazada? El script falla si una crate de `cargo tree -e normal,no-proc-macro` no tiene aviso.

5. Pasa la batería completa (`tsc`, `npm test`, `npm run build`, `cargo test --locked`, `cargo clippy -- -D warnings`) y sube todo junto.

### Si la CI falla en `licenses`

El mensaje dice qué lockfile cambió (`Cargo.lock` o `package-lock.json`). Haz los pasos 1–5 en tu rama y vuelve a subir.
No edites `THIRD_PARTY_LICENSES.txt` a mano: la cabecera con los hashes se rompería y el contenido dejaría de ser reproducible.

### Si cambias la versión de `cargo-about`

Cámbiala en `ABOUT_VERSION` (`tools/gen_third_party_licenses.py`), instala esa versión, regenera y revisa el diff completo
(cambian la plantilla y el agrupado de textos).

## Dependabot: NO está activado

No hay `.github/dependabot.yml`. La propuesta quedó guardada como `docs/proposed/dependabot.yml.proposed` (inactiva). Motivo:
cada PR de Dependabot que toque `Cargo.lock` o `package-lock.json` haría fallar `licenses` hasta que una persona regenere
los avisos, y la CI **no** regenera nada (solo tiene permiso de lectura: es una decisión de mínimo privilegio; hacerlo
requeriría permisos de escritura y ejecutar `cargo-about` en la CI).

Antes de activar nada, comprueba también en *Settings → Code security* que **Dependabot security updates** y
**version updates** estén desactivados: los de seguridad abren PR por su cuenta aunque no exista el archivo.
(Las *alertas* de Dependabot son pasivas, no abren PR; decide tú si las quieres.)

### Estrategia acordada para activarlo más adelante (propuesta, aún sin aplicar)

| Fase | Qué se activa | Regeneración de avisos |
|---|---|---|
| 0 (v0.1.0) | Nada. Actualizaciones manuales con los pasos de arriba. Auditoría manual (`npm audit`, y `cargo audit` si se instala). | La hace quien actualiza |
| 1 | Dependabot **solo** para `github-actions` (no toca lockfiles, así que no afecta a `licenses`). | No aplica |
| 2 | `npm` y `cargo`, **agrupando** en un PR mensual y con límite de PR abiertos bajo. | Una persona mantenedora hace checkout de la rama del PR, ejecuta los pasos 1–5 y sube el commit de regeneración antes de fusionar. Las actualizaciones de seguridad urgentes siguen el mismo camino, sin saltarse el check. |

El job `licenses` **sigue siendo obligatorio** en todas las fases.

## Qué cubre y qué no

- Cubre: crates de Rust compiladas para Windows x64 (enlazadas más macros de compilación) y paquetes npm de **producción**
  (`react`, `react-dom`, `scheduler`, `@tauri-apps/api`). Las dependencias de desarrollo no se redistribuyen y no están en el archivo.
- No cubre: la licencia del propio Miel, el runtime WebView2 (no se incluye) ni una revisión legal. Ver `docs/THIRD_PARTY_NOTES.md`.
