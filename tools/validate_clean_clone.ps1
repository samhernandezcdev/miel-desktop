<#
.SYNOPSIS
  Valida Miel desde un CLON LIMPIO y genera el instalador definitivo con su SHA-256.

.DESCRIPTION
  Pensado para ejecutarse DESPUÉS de que la persona titular cree el repositorio y haga el primer commit.
  Este script NO se ejecutó contra un repositorio Git real al prepararlo (aún no hay commits); la rama
  -FromDirectory (copia sin Git) sí se probó como ensayo, pero NO sirve para el instalador definitivo.

  Qué hace, en una carpeta de trabajo nueva y vacía:
    1. Clona el repositorio (git clone) y anota el commit exacto; comprueba que el árbol está limpio.
    2. Comprueba que no se versionó nada que no debe (node_modules, target, dist, EBWebView, settings.json, etc.).
    3. npm ci
    4. tsc --noEmit, vitest, npm run build (incluye check:dist), licenses:check
    5. cargo test --locked y cargo clippy --all-targets --locked -- -D warnings
    6. npm run tauri build   (instalador NSIS)
    7. Comprueba que THIRD_PARTY_LICENSES.txt va en el instalador (installer.nsi) y que el árbol sigue limpio.
    8. Calcula el SHA-256 del instalador y escribe SHA256SUMS.txt + validation-report.txt en <WorkDir>\out

  Se detiene en el primer fallo. No publica nada, no hace push, no crea tags.

.PARAMETER Repo
  URL del repositorio (https://github.com/<usuario>/miel-desktop.git) o ruta local de un clon/repositorio.
.PARAMETER Ref
  Rama, tag o commit a validar (por defecto, el HEAD del clon).
.PARAMETER WorkDir
  Carpeta de trabajo (debe no existir o estar vacía). Por defecto, una carpeta nueva en %TEMP%.
.PARAMETER FromDirectory
  SOLO ENSAYO: en vez de `git clone`, copia una carpeta (sin node_modules/target/dist/gen/.git). El resultado
  NO es válido como instalador definitivo (no sale de código versionado).
.PARAMETER SkipBuild
  No ejecuta `npm run tauri build` (solo pruebas).

.EXAMPLE
  .\tools\validate_clean_clone.ps1 -Repo https://github.com/USUARIO/miel-desktop.git
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Repo,
  [string]$Ref = '',
  [string]$WorkDir = '',
  [switch]$FromDirectory,
  [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not $WorkDir) { $WorkDir = Join-Path $env:TEMP ("miel-clean-" + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
if ((Test-Path $WorkDir) -and (Get-ChildItem $WorkDir -Force | Select-Object -First 1)) { throw "La carpeta de trabajo ya existe y no está vacía: $WorkDir" }
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
$src = Join-Path $WorkDir 'miel-desktop'
$out = Join-Path $WorkDir 'out'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$report = New-Object System.Collections.Generic.List[string]
function Log([string]$m) { Write-Host $m; $report.Add($m) }

function Step([string]$name, [scriptblock]$body) {
  Log ""
  Log "=== $name ==="
  $t0 = Get-Date
  $global:LASTEXITCODE = 0
  & $body
  if ($global:LASTEXITCODE -ne 0) { throw "FALLÓ: $name (código $global:LASTEXITCODE)" }
  Log ("    OK ({0:n0} s)" -f ((Get-Date) - $t0).TotalSeconds)
}
function Need([string]$cmd) { if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { throw "Falta '$cmd' en el PATH" } }

foreach ($c in 'node', 'npm', 'cargo', 'python') { Need $c }
if (-not $FromDirectory) { Need 'git' }

Log "Miel: validación desde clon limpio"
Log ("Fecha: {0}   Equipo: {1}   Windows: {2}" -f (Get-Date -Format 's'), $env:COMPUTERNAME, [Environment]::OSVersion.Version)
Log ("node {0} | npm {1} | {2}" -f (node -v), (npm -v), (cargo -V))
if ($FromDirectory) { Log "*** MODO ENSAYO (-FromDirectory): el resultado NO es el instalador definitivo ***" }

# 1) Obtener el código
if ($FromDirectory) {
  Step 'Copiar carpeta (ensayo, sin Git)' {
    New-Item -ItemType Directory -Force -Path $src | Out-Null
    $xd = @('node_modules', 'target', 'target-diag', 'dist', 'gen', '.git', '__pycache__')
    & robocopy.exe $Repo $src /E /XD @xd /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "robocopy falló ($LASTEXITCODE)" }
    $global:LASTEXITCODE = 0
  }
} else {
  Step "git clone $Repo" {
    git clone $Repo $src
    if ($LASTEXITCODE -ne 0) { return }
    if ($Ref) { git -C $src checkout --detach $Ref }
  }
  $commit = (git -C $src rev-parse HEAD).Trim()
  Log "Commit validado: $commit"
  Step 'Árbol limpio y sin archivos prohibidos' {
    $dirty = git -C $src status --porcelain
    if ($dirty) { throw "El clon no está limpio:`n$dirty" }
    $files = git -C $src ls-files
    $bad = $files | Where-Object { $_ -match '(^|/)(node_modules|target|target-diag|dist|EBWebView)/' -or $_ -match '(^|/)(settings(\.corrupt-\d+)?\.json|frontend-errors\.log|\.env.*)$' -or $_ -match '\.(exe|msi|pem|pfx|p12|key)$' }
    if ($bad) { throw "Archivos que no deben estar versionados:`n$($bad -join "`n")" }
    Log ("    {0} archivos versionados" -f @($files).Count)
    $global:LASTEXITCODE = 0
  }
}

Push-Location $src
try {
  Step 'npm ci'                          { npm ci }
  Step 'tsc --noEmit'                    { npx tsc --noEmit }
  Step 'vitest'                          { npx vitest run 2>&1 | Tee-Object -Variable vt | Out-Host; ($vt | Select-String 'Tests ').Line | ForEach-Object { Log "    $_" } }
  Step 'npm run build (incluye check:dist)' { npm run build }
  Step 'licenses:check (avisos de terceros al día)' { npm run licenses:check }
  Step 'cargo test --locked' {
    cargo test --manifest-path src-tauri/Cargo.toml --locked 2>&1 | Tee-Object -Variable ct | Out-Host
    $ct | Select-String 'test result:' | ForEach-Object { Log "    $($_.Line)" }
  }
  Step 'cargo clippy -D warnings' { cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings }

  if (-not $SkipBuild) {
    Step 'npm run tauri build' { npm run tauri build }
    $setup = Join-Path $src 'src-tauri\target\release\bundle\nsis\Miel_0.1.0_x64-setup.exe'
    if (-not (Test-Path $setup)) { throw "No se generó el instalador: $setup" }
    $nsi = Get-Content (Join-Path $src 'src-tauri\target\release\nsis\x64\installer.nsi') -Raw
    if ($nsi -notmatch 'File /a "/oname=THIRD_PARTY_LICENSES\.txt"') { throw 'installer.nsi NO incluye THIRD_PARTY_LICENSES.txt' }
    if ($nsi -notmatch 'Delete "\$INSTDIR\\THIRD_PARTY_LICENSES\.txt"') { throw 'installer.nsi NO borra THIRD_PARTY_LICENSES.txt al desinstalar' }
    Log 'installer.nsi: incluye THIRD_PARTY_LICENSES.txt y lo borra al desinstalar'
    $a = (Get-FileHash (Join-Path $src 'src-tauri\target\release\THIRD_PARTY_LICENSES.txt') -Algorithm SHA256).Hash
    $b = (Get-FileHash (Join-Path $src 'THIRD_PARTY_LICENSES.txt') -Algorithm SHA256).Hash
    if ($a -ne $b) { throw 'El recurso empaquetado difiere de THIRD_PARTY_LICENSES.txt del repositorio' }
    Log 'Recurso empaquetado idéntico al archivo del repositorio'
    if (-not $FromDirectory) {
      $dirty = git -C $src status --porcelain
      if ($dirty) { throw "La compilación modificó archivos versionados:`n$dirty" }
      Log 'Árbol sigue limpio tras compilar'
    }
    Copy-Item $setup $out
    Copy-Item (Join-Path $src 'THIRD_PARTY_LICENSES.txt') $out
    $h1 = (Get-FileHash (Join-Path $out 'Miel_0.1.0_x64-setup.exe') -Algorithm SHA256).Hash.ToLower()
    $h2 = (Get-FileHash (Join-Path $out 'THIRD_PARTY_LICENSES.txt') -Algorithm SHA256).Hash.ToLower()
    @("$h1  Miel_0.1.0_x64-setup.exe", "$h2  THIRD_PARTY_LICENSES.txt") | Set-Content -Encoding ascii (Join-Path $out 'SHA256SUMS.txt')
    Log ""
    Log "SHA-256 instalador : $h1"
    Log "SHA-256 avisos     : $h2"
    Log ("Tamaño instalador  : {0:n0} bytes" -f (Get-Item (Join-Path $out 'Miel_0.1.0_x64-setup.exe')).Length)
    if ($FromDirectory) { Log '*** ENSAYO: no usar este hash para la release ***' }
  }
} finally {
  Pop-Location
  $report | Set-Content -Encoding utf8 (Join-Path $out 'validation-report.txt')
}
Log ""
Log "Listo. Resultados en: $out"
