# Checklist de publicación: Miel v0.1.0 (pre-release)

Todo lo de este documento lo hace **la persona titular, a mano**. No se ha ejecutado nada de Git: no hay commits, tags, remoto ni
workflows creados, y **nada está publicado**. Los comandos de Git son solo guía. Marca cada casilla al hacerla.

La release v0.1.0 se prepara como **pre-release** y en **borrador**: no se publica hasta que tú lo decidas.

## 0. Decisiones y datos que faltan (bloquean la publicación)

- [ ] **Titular del copyright y año** (ver `docs/LICENSING.md`, "Por confirmar" 1 y 2: titularidad y autoría humana/IA).
- [ ] **Licencias definitivas** (propuesta: código MIT, recursos CC BY 4.0, marca aparte). Crear `LICENSE` (desde `docs/legal-drafts/LICENSE-MIT.draft.txt`), `LICENSE-ASSETS` (con el texto oficial de CC BY 4.0) y la política de marca final (`TRADEMARK.md`, desde `docs/legal-drafts/TRADEMARK_POLICY.draft.md`).
- [ ] **Nombre "Miel"**: búsqueda en registros de marcas (México y países de distribución); y uso de "BKODE".
- [ ] **Dominio `bkode.lat`**: confirmar que lo controlas (de él sale el identificador `lat.bkode.miel`, que **no debe cambiar** después de publicar).
- [ ] **Usuario u organización de GitHub** y nombre del repositorio.
- [ ] **Contactos** (conducta y seguridad) y política de contribuciones (¿CLA o entrada = salida?).
- [ ] **Capturas reales** de la ventana de Miel sobre un fondo neutro (las actuales de `docs/img/` son renders de los componentes). Revísalas antes: sin contenido del escritorio, rutas ni nombres.

## 1. Rellenar marcadores

```powershell
Select-String -Path (Get-ChildItem -Recurse -File -Include *.md,*.yml,*.txt | Where-Object { $_.FullName -notmatch 'node_modules|target|\\dist\\' }) -Pattern 'OWNER|USUARIO|\[CONTACTO|\[CONDUCT|\[SECURITY|\[AÑO|\[TITULAR|FECHA POR DEFINIR|\[SHA256|\[Licen'
```

| Marcador | Dónde | Qué poner |
|---|---|---|
| `OWNER` | `.github/ISSUE_TEMPLATE/config.yml` | Tu usuario/organización de GitHub |
| `[CONTACTO DE CONDUCTA…]`, `[CONDUCT CONTACT…]` | `CODE_OF_CONDUCT.md` | Un contacto real |
| `[CONTACTO DE SEGURIDAD…]`, `[SECURITY CONTACT…]` | `SECURITY.md` | Un contacto real (o quita el correo si solo usas el reporte privado) |
| `FECHA POR DEFINIR` | `CHANGELOG.md` | Fecha de la release |
| `[SHA256 …]`, `[Licencias…]` | `docs/RELEASE_NOTES_v0.1.0.md` | Hash real (sección 5) y licencia definitiva |
| `[AÑO]`, `[TITULAR…]` | `docs/legal-drafts/*` | Al crear los archivos de licencia |

Actualiza también el README (secciones *Licencia* y *Limitaciones*) y `ROADMAP.md` (Hito 0) cuando decidas las licencias. Declara la licencia en `package.json` y `src-tauri/Cargo.toml` solo **después** de confirmarla.

## 2. Revisión de lo que se va a versionar (antes del primer commit)

`.gitignore` ya excluye `node_modules`, `dist`, `src-tauri/target`, `src-tauri/target-diag`, `src-tauri/gen`, instaladores (`*.exe`, `*.msi`),
`SHA256SUMS.txt`, perfiles WebView2 (`EBWebView/`), `settings.json`, claves/certificados y `.env*`. Se comprobó (simulando sus reglas **sin usar Git**)
que **todos los archivos necesarios para compilar son versionables**: de 150 archivos, la copia limpia con la que se compiló antes es exactamente ese conjunto.

Justo antes del primer commit repite la revisión con Git (solo lectura):

```powershell
git status --short --ignored          # nada inesperado
git add -A --dry-run | Select-String -Pattern 'EBWebView|settings\.json|\.env|\.pem|\.pfx|\.key|node_modules|target|dist/|\.exe|\.claude'   # no debe salir nada
git add -A --dry-run | Select-String -Pattern '\.(png|jpg|jpeg|gif|webp)$'   # revisa a ojo cada imagen (debe haber solo renders del personaje e iconos)
```

Sí es intencional (y no contiene datos personales): `docs/measurements/*` (PID y marcas de tiempo; **cifras invalidadas**, ver su README).
Los instaladores y `SHA256SUMS.txt` **no se versionan**: se adjuntan a la release.

## 3. Commit y repositorio privado (comandos de Git, a mano)

El directorio ya tiene un repositorio Git vacío (rama `main`, sin commits). Crea primero el repositorio remoto como **privado**.

```powershell
git config user.name  "TU NOMBRE"
git config user.email "TU_ID+TU_USUARIO@users.noreply.github.com"   # el correo noreply de GitHub evita publicar tu correo real
git add -A
git status                                    # revisa qué entra
git commit -m "Miel Desktop v0.1.0: primera versión pública"
# En github.com: crear el repositorio PRIVADO, SIN README, .gitignore ni licencia generados. Luego:
git remote add origin https://github.com/USUARIO/miel-desktop.git
git push -u origin main
```

- [ ] La pestaña **Actions** muestra la CI en verde: jobs **`licenses`**, **`frontend`** y **`rust`**. Si falla, corrige antes de seguir.

## 4. Configurar el repositorio (aún privado)

- [ ] **Settings → Branches**: proteger `main` (pull request obligatorio) y marcar como **checks obligatorios** `licenses`, `frontend` y `rust`.
      **`licenses` debe seguir siendo obligatorio**: es lo que impide que cambien los lockfiles sin regenerar `THIRD_PARTY_LICENSES.txt`
      (cómo regenerarlo: `docs/DEPENDENCY_UPDATES.md`).
- [ ] **Settings → Actions → General → Workflow permissions**: *Read repository contents permissions*; exigir aprobación para PR de colaboradores externos primerizos.
- [ ] **Settings → Code security**: activar *Private vulnerability reporting* y *Secret scanning / Push protection*.
      **NO actives Dependabot** (ni *security updates* ni *version updates*) hasta definir la estrategia (`docs/DEPENDENCY_UPDATES.md`): sus PR
      harían fallar `licenses` hasta regenerar los avisos a mano. No hay `.github/dependabot.yml` (la propuesta está en `docs/proposed/`).
- [ ] Descripción corta, *topics* (`tauri`, `rust`, `react`, `typescript`, `desktop-pet`, `windows`, `mascot`) e imagen social limpia (opcional).
- [ ] Activar *Issues*; *Discussions* es opcional.
- [ ] Crear las **etiquetas** de `.github/labels.yml` (bug, enhancement, documentation, good first issue, help wanted, roadmap y las demás), a mano en *Issues → Labels* o con `gh` (si lo instalas):

```powershell
gh label create "bug" --color d73a4a --description "Algo no funciona / Something is broken" --force
gh label create "enhancement" --color a2eeef --description "Mejora o funcionalidad nueva" --force
gh label create "documentation" --color 0075ca --description "Documentación" --force
gh label create "good first issue" --color 7057ff --description "Bueno para empezar" --force
gh label create "help wanted" --color 008672 --description "Se agradece ayuda" --force
gh label create "roadmap" --color fbca04 --description "Relacionado con ROADMAP.md" --force
# El resto: ver .github/labels.yml
```

## 5. Validar un clon limpio y generar el instalador definitivo

Sigue **`docs/CLEAN_CLONE_VALIDATION.md`** (script `tools/validate_clean_clone.ps1` o los mismos pasos a mano): clon limpio → `npm ci` →
pruebas completas → `npm run tauri build` → SHA-256. Todo debe pasar. **Ese** instalador (y su hash) es el que se publica.

- [ ] Resultado: `vitest` 57/57 · `cargo test` 156 unitarias + 5 de integración · `clippy` sin avisos · `check:dist` y `licenses:check` OK.
- [ ] El commit validado es el mismo que tiene la CI en verde. Si haces otro commit, **repite el paso**.
- [ ] **No uses** el instalador provisional `C:\dev\miel-release-artifacts\v0.1.0-PROVISIONAL-no-publicar\` (salió de una copia de archivos, no de un commit).
- [ ] Copia el hash a `docs/RELEASE_NOTES_v0.1.0.md` y haz commit de ese cambio **antes** de etiquetar (si solo cambian las notas, el instalador no cambia, pero la etiqueta debe apuntar al commit final).

`SHA256SUMS.txt` ya sale generado en `out\` (si lo haces a mano: `"$hash  Miel_0.1.0_x64-setup.exe"` y `"$hash  THIRD_PARTY_LICENSES.txt"`, en minúsculas y con dos espacios).

## 6. Prueba manual con el instalador definitivo

Haz **`docs/MANUAL_TEST_CHECKLIST.md`** (instalación, animaciones, un solo rostro, bandeja, click-through, burbuja, desinstalación) y anota el resultado en `docs/TEST_LOG.md`.

- [ ] **Tras instalar:** `.\tools\check_install.ps1 -Phase Installed -LicensesFile .\THIRD_PARTY_LICENSES.txt` → `THIRD_PARTY_LICENSES.txt` en `%LOCALAPPDATA%\Miel` con el mismo SHA-256.
- [ ] **Tras desinstalar:** `.\tools\check_install.ps1 -Phase Uninstalled` → el archivo y `miel.exe` ya no están.
- Evidencia **estática** ya obtenida (no sustituye a lo anterior): el script NSIS empaqueta `THIRD_PARTY_LICENSES.txt` (235 964 bytes, log de `makensis -V4`) y lo borra al desinstalar. **Que aparezca al instalar de verdad y se elimine al desinstalar está sin comprobar hasta que hagas esta prueba.**
- [ ] Si una prueba falla, **no publiques**; corrige y vuelve a la sección 5.

## 7. Hacer público el repositorio

- [ ] Actualiza `CHANGELOG.md` (fecha de 0.1.0). Revisa el README y las notas: las limitaciones declaradas siguen siendo ciertas (sección 9).
- [ ] Cambia el repositorio a **público** (*Settings → Danger zone → Change visibility*) cuando todo lo anterior esté listo.
- [ ] Crea unos pocos issues iniciales (`good first issue`, `help wanted`, `roadmap`): capturas reales de la ventana, traducción al inglés de `ROADMAP.md`, prueba en varios monitores, nuevas frases, probar el instalador, repetir mediciones.

## 8. Etiqueta y GitHub Release v0.1.0 como PRE-RELEASE (borrador)

```powershell
git tag -a v0.1.0 -m "Miel v0.1.0"          # sobre el commit validado en la sección 5
git push origin v0.1.0
```

En GitHub → *Releases → Draft a new release*:

- [ ] Etiqueta `v0.1.0`, título `Miel v0.1.0`.
- [ ] Marca **"Set as a pre-release"** (instalador sin firmar y pruebas manuales pendientes). **No marques "Set as the latest release".**
- [ ] Cuerpo: `docs/RELEASE_NOTES_v0.1.0.md`, con el **SHA-256 real** del instalador generado desde el clon limpio.
- [ ] Adjunta `Miel_0.1.0_x64-setup.exe`, `THIRD_PARTY_LICENSES.txt` y `SHA256SUMS.txt` (los de `out\`).
- [ ] Guárdala como **borrador** (*Save draft*), revísala y publícala solo cuando decidas. Con `gh` (alternativa): `gh release create v0.1.0 <out>\Miel_0.1.0_x64-setup.exe <out>\THIRD_PARTY_LICENSES.txt <out>\SHA256SUMS.txt --title "Miel v0.1.0" --notes-file docs\RELEASE_NOTES_v0.1.0.md --draft --prerelease`
- [ ] **Antes de publicar el borrador:** descarga el instalador desde el propio borrador y verifica que su SHA-256 coincide con `SHA256SUMS.txt` y con las notas.

## 9. Limitaciones que se mantienen declaradas (no se presentan como probadas)

README, notas de release y CHANGELOG deben seguir diciendo que están **pendientes**:

- [ ] Prueba prolongada de **8 horas**.
- [ ] **Varios monitores** y **cambios de DPI/resolución** reales.
- [ ] **Nuevas mediciones de rendimiento** con la build corregida (las de `docs/PERFORMANCE.md` están **invalidadas**; no se cita ninguna cifra).
- [ ] Instalador **sin firmar** (SmartScreen avisará) y firma de código sin resolver.
- [ ] Pruebas manuales no incluidas en la checklist reducida (ver `docs/TEST_LOG.md`).

## 10. Después de publicar

- [ ] Revisar el primer issue/PR externo con calma (ver `CONTRIBUTING.md`).
- [ ] Abrir issues para las limitaciones y pruebas pendientes.
- [ ] Investigar firma de código (ver `ROADMAP.md`, Comunidad) y auditoría de dependencias (`cargo audit`/`cargo deny`, `npm audit`) en la CI.
- [ ] Decidir la estrategia de Dependabot (`docs/DEPENDENCY_UPDATES.md`) antes de activarlo.
- [ ] **No empezar el Hito A** hasta cerrar el Hito 0.

## Inventario de etiquetas iniciales

| Etiqueta | Uso |
|---|---|
| `bug` | Algo no funciona |
| `enhancement` | Mejora o funcionalidad nueva |
| `documentation` | Documentación |
| `good first issue` | Para empezar |
| `help wanted` | Se agradece ayuda |
| `roadmap` | Relacionado con `ROADMAP.md` |
| `privacy`, `security`, `performance` | Temas transversales |
| `character`, `messages`, `translation` | Personaje, frases, traducciones |
| `windows`, `installer` | Específicos de Windows / instalador |
| `blocked`, `wontfix`, `duplicate`, `question` | Gestión |

Colores y descripciones: `.github/labels.yml`.
