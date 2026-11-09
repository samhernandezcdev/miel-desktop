<#
.SYNOPSIS
  Comprueba, DESPUÉS de instalar o de desinstalar Miel, que los avisos de licencia (LICENSE-MIT, LICENSE-APACHE,
  LICENSE-ASSETS, NOTICE y THIRD_PARTY_LICENSES.txt) están (o ya no están).

.DESCRIPTION
  Solo LEE el sistema de archivos y el registro del usuario; no instala, no desinstala ni modifica nada.
  Ejecútalo a mano en el equipo de prueba:
    1) tras instalar:       .\tools\check_install.ps1 -Phase Installed   -RepoRoot .
    2) tras desinstalar:    .\tools\check_install.ps1 -Phase Uninstalled

.PARAMETER Phase
  Installed   -> exige miel.exe y los 5 archivos de licencia en la carpeta de instalación, no vacíos y, con -RepoRoot,
                 con el mismo contenido (SHA-256) que los del repositorio (-LicensesFile/-ExpectedSha256 siguen
                 aceptándose para THIRD_PARTY_LICENSES.txt).
  Uninstalled -> exige que los 5 archivos de licencia y miel.exe ya no estén y que la carpeta no exista o esté vacía.
.PARAMETER InstallDir
  Carpeta de instalación (por defecto %LOCALAPPDATA%\Miel, instalación por usuario).
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][ValidateSet('Installed', 'Uninstalled')][string]$Phase,
  [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Miel'),
  [string]$LicensesFile = '',
  [string]$ExpectedSha256 = '',
  [string]$RepoRoot = ''
)
$ErrorActionPreference = 'Stop'
$fail = 0
function Pass([string]$m) { Write-Host "  [OK]    $m" }
function Fail([string]$m) { Write-Host "  [FALLO] $m"; $script:fail++ }
function Info([string]$m) { Write-Host "  [info]  $m" }

$licFiles = @('LICENSE-MIT', 'LICENSE-APACHE', 'LICENSE-ASSETS', 'NOTICE', 'THIRD_PARTY_LICENSES.txt')
$exe = Join-Path $InstallDir 'miel.exe'
Write-Host "Fase: $Phase   Carpeta: $InstallDir"

if ($Phase -eq 'Installed') {
  if (Test-Path $exe) { Pass 'miel.exe presente' } else { Fail 'miel.exe NO está' }
  foreach ($n in $licFiles) {
    $f = Join-Path $InstallDir $n
    if (-not (Test-Path $f)) { Fail "$n NO está en la carpeta de instalación"; continue }
    $size = (Get-Item $f).Length
    if ($size -gt 0) { Pass ("$n presente ({0:n0} bytes)" -f $size) } else { Fail "$n está vacío" }
    $want = ''
    if ($RepoRoot -and (Test-Path (Join-Path $RepoRoot $n))) { $want = (Get-FileHash (Join-Path $RepoRoot $n) -Algorithm SHA256).Hash }
    if ($n -eq 'THIRD_PARTY_LICENSES.txt') {
      if (-not $want -and $ExpectedSha256) { $want = $ExpectedSha256 }
      if (-not $want -and $LicensesFile) { $want = (Get-FileHash $LicensesFile -Algorithm SHA256).Hash }
    }
    if ($want) {
      $got = (Get-FileHash $f -Algorithm SHA256).Hash
      if ($got -ieq $want) { Pass "$n`: SHA-256 coincide ($($got.Substring(0,16))...)" } else { Fail "$n`: SHA-256 distinto: instalado $got, esperado $want" }
    } else { Info "$n`: sin referencia (-RepoRoot/-LicensesFile/-ExpectedSha256): no se comparó el contenido" }
  }
  $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Miel'
  if (Test-Path $key) { Info ("Entrada de desinstalación: {0} {1}" -f (Get-ItemProperty $key).DisplayName, (Get-ItemProperty $key).DisplayVersion) } else { Info 'No se encontró la clave de desinstalación HKCU\...\Uninstall\Miel (revisa Configuración > Aplicaciones)' }
} else {
  foreach ($n in $licFiles) {
    if (Test-Path (Join-Path $InstallDir $n)) { Fail "$n SIGUE en la carpeta de instalación" } else { Pass "$n eliminado" }
  }
  if (Test-Path $exe) { Fail 'miel.exe SIGUE en la carpeta de instalación' } else { Pass 'miel.exe eliminado' }
  if (Test-Path $InstallDir) {
    $left = @(Get-ChildItem $InstallDir -Force -Recurse -ErrorAction SilentlyContinue)
    if ($left.Count -eq 0) { Pass 'La carpeta de instalación está vacía' } else { Fail ("Quedan {0} elementos en {1}: {2}" -f $left.Count, $InstallDir, (($left | Select-Object -First 5 -ExpandProperty Name) -join ', ')) }
  } else { Pass 'La carpeta de instalación ya no existe' }
  # Informativo: dependen de la casilla "Eliminar los datos de la aplicación"
  foreach ($d in @((Join-Path $env:APPDATA 'lat.bkode.miel'), (Join-Path $env:LOCALAPPDATA 'lat.bkode.miel'))) {
    if (Test-Path $d) { Info "Existen datos de la aplicación (esperado si NO marcaste la casilla): $d" } else { Info "No hay datos en $d (esperado si marcaste la casilla)" }
  }
}
Write-Host ""
if ($fail -eq 0) { Write-Host "RESULTADO: OK"; exit 0 } else { Write-Host "RESULTADO: $fail fallo(s)"; exit 1 }
