# Validar un clon limpio y generar el instalador definitivo

Se hace **después** de que la persona titular cree el repositorio privado y haga el primer commit. **Yo (el asistente) no
ejecuto operaciones de Git**: ni commit, ni push, ni tag, ni clonado. Estas instrucciones son para que las ejecutes tú.

**Por qué:** el instalador que se publica debe salir de código *versionado*, no de tu carpeta de trabajo (que puede tener
archivos sin versionar, `node_modules` viejos, `dist` antiguos…). Un clon limpio demuestra que el repositorio basta para compilar.
El SHA-256 de la release se calcula del instalador generado así, **no** del instalador provisional que ya existe.

## Antes de empezar

- Primer commit hecho (y, si quieres, subido al repositorio privado). Anota el hash: `git rev-parse HEAD`.
- Herramientas en el PATH: Git, Node.js 22+, Rust estable (≥ 1.82, MSVC), Python 3, PowerShell 7. Para regenerar avisos
  (no para validar): `cargo-about` 0.9.3 (ver `docs/DEPENDENCY_UPDATES.md`).
- Una carpeta de trabajo **vacía** fuera del repositorio. Tarda varios minutos (compilación completa de Rust con LTO).

## Opción A: script (recomendada)

```powershell
# Desde tu copia de trabajo (o desde cualquier sitio donde tengas el script):
.\tools\validate_clean_clone.ps1 -Repo https://github.com/USUARIO/miel-desktop.git
# o, con un repositorio local:
.\tools\validate_clean_clone.ps1 -Repo C:\ruta\a\miel-desktop
# opcional: -Ref <rama|tag|commit>   -WorkDir C:\trabajo\miel-limpio   -SkipBuild
```

El script, en una carpeta nueva: clona, anota el commit, comprueba que el árbol está limpio y que no hay archivos prohibidos
versionados; ejecuta `npm ci`, `tsc`, `vitest`, `npm run build` (con `check:dist`), `npm run licenses:check`,
`cargo test --locked`, `cargo clippy -D warnings` y `npm run tauri build`; comprueba que `installer.nsi` incluye
`THIRD_PARTY_LICENSES.txt` y su borrado al desinstalar; comprueba que el árbol sigue limpio tras compilar, y escribe en
`<carpeta>\out\`: el instalador, `THIRD_PARTY_LICENSES.txt`, `SHA256SUMS.txt` y `validation-report.txt`. Se detiene en el primer fallo.

## Opción B: a mano (los mismos pasos)

```powershell
git clone https://github.com/USUARIO/miel-desktop.git miel-limpio
cd miel-limpio
git rev-parse HEAD                      # anota el commit
git status --porcelain                  # debe salir vacío
npm ci
npx tsc --noEmit
npm test
npm run build                           # incluye check:dist
npm run licenses:check
cargo test   --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
npm run tauri build
git status --porcelain                  # debe seguir vacío (la compilación no debe modificar nada versionado)
Get-FileHash .\src-tauri\target\release\bundle\nsis\Miel_0.1.0_x64-setup.exe -Algorithm SHA256
```

## Resultado esperado (al preparar este documento)

| Prueba | Esperado |
|---|---|
| `vitest` | 57 de 57 |
| `cargo test` | 156 unitarias + 5 de integración |
| `clippy -D warnings` | 0 avisos |
| `check:dist`, `licenses:check` | OK |

Si añades pruebas, estas cifras cambian: actualízalas aquí. **Cualquier fallo detiene la publicación.**

## Después

1. Comprueba que la CI de GitHub está en verde **sobre el mismo commit** (`licenses`, `frontend`, `rust`).
2. Usa el instalador y `SHA256SUMS.txt` de `out\` para la release; copia el hash a `docs/RELEASE_NOTES_v0.1.0.md`.
3. Si haces **otro commit** antes de etiquetar, repite todo: el hash es del código exacto que se etiqueta.
4. Haz la [checklist manual](MANUAL_TEST_CHECKLIST.md) con **ese** instalador.

## Notas

- Las compilaciones **no son reproducibles bit a bit** (rutas y metadatos): el hash cambia aunque el código sea el mismo.
  Por eso el hash que publicas debe ser el del archivo que subes, calculado una sola vez.
- `-FromDirectory` es solo un ensayo sin Git: su instalador **no** vale para publicar.
- El instalador provisional del 2026-11-01 (`C:\dev\miel-release-artifacts\v0.1.0-PROVISIONAL-no-publicar\`) salió de una copia
  de archivos, no de un commit: **no lo publiques**.
- Este script y su rama de Git **no se han ejecutado contra un repositorio real** (aún no hay commits); solo se ensayó la rama
  `-FromDirectory`. Si falla algo en la parte de Git, corrígelo o usa la opción B.
