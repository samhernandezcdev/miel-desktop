<#
Utilidades compartidas por los scripts de medicion/verificacion (se cargan con punto: `. "$PSScriptRoot\..\common.ps1"`).

- Write-Utf8NoBom: escribe texto en UTF-8 SIN BOM (`Set-Content -Encoding UTF8` de Windows PowerShell 5 anade BOM; Miel
  ya lo tolera desde E4a, pero los scripts no deben sembrar archivos con BOM).
- Clear-PersonalPaths: sustituye en un archivo de texto las rutas de la maquina actual (perfil de usuario, %APPDATA%, %LOCALAPPDATA%,
  %TEMP%, nombre de usuario y de equipo) por marcadores. Se llama sobre los registros AL TERMINAR cada ejecucion, para que
  nada personal llegue a docs/measurements/. Los valores se leen del entorno en ejecucion; nunca se escriben en el repositorio.
#>

function Write-Utf8NoBom {
  param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Text)
  [System.IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

function Clear-PersonalPaths {
  param([Parameter(Mandatory)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return }
  $text = [System.IO.File]::ReadAllText($Path)
  $orig = $text
  # De la mas especifica a la menos: AppData\Roaming, AppData\Local\Temp, AppData\Local, perfil.
  $pairs = @(
    @($env:APPDATA, '%APPDATA%'),
    @($env:TEMP, '%TEMP%'),
    @($env:LOCALAPPDATA, '%LOCALAPPDATA%'),
    @($env:USERPROFILE, '%USERPROFILE%')
  )
  foreach ($p in $pairs) {
    if ([string]::IsNullOrEmpty($p[0])) { continue }
    $plain = $p[0]
    $escaped = $plain.Replace('\', '\\')       # forma que sale en cadenas JSON / Debug de Rust
    $text = $text.Replace($escaped, $p[1]).Replace($plain, $p[1])
  }
  foreach ($pair in @(@($env:USERNAME, '<usuario>'), @($env:COMPUTERNAME, '<equipo>'))) {
    if ([string]::IsNullOrEmpty($pair[0]) -or $pair[0].Length -lt 3) { continue }
    $text = [regex]::Replace($text, [regex]::Escape($pair[0]), $pair[1], 'IgnoreCase')
  }
  if ($text -ne $orig) { [System.IO.File]::WriteAllText($Path, $text, (New-Object System.Text.UTF8Encoding($false))) }
}

function Clear-PersonalPathsInDir {
  param([Parameter(Mandatory)][string]$Dir)
  if (-not (Test-Path -LiteralPath $Dir)) { return }
  Get-ChildItem -LiteralPath $Dir -File -Recurse | ForEach-Object { Clear-PersonalPaths -Path $_.FullName }
}
