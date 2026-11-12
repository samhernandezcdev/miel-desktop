<#
E4a (SOLO verificacion): equivalencia de comportamiento entre dos binarios `diag` con la MISMA semilla de RNG
(MIEL_SEED, que el parche de la prueba anade a ambos; no existe en el repositorio) y el MISMO guion de ordenes.
Ejecuta cada variante `-Reps` veces, intercaladas, guarda el registro (stderr) y no toca el cursor ni el teclado.
Respalda y restaura settings.json (queda ausente si estaba ausente) y no deja procesos en marcha.
Cada ejecucion empieza SIN settings.json (las ordenes `pause`/`follow_*` lo escriben): mismo punto de partida.
Uso: -Variants "nombre=ruta.exe;nombre2=ruta2.exe" -Out <carpeta de registros>
#>
param(
  [Parameter(Mandatory)][string]$Variants,
  [Parameter(Mandatory)][string]$Out,
  [int]$Reps = 3, [int]$Seconds = 125, [int]$Seed = 42,
  [string]$Script = 'pause@20,resume@28,hide@36,show@44,suppress@52,override@60,unsuppress@68,follow_off@76,follow_on@84,suppress@92,unsuppress@100'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')   # Write-Utf8NoBom / Clear-PersonalPaths
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$dir = Split-Path $settings
$bak = Join-Path $env:TEMP 'miel-settings-eq.bak'
$existed = Test-Path $settings
$hash0 = if ($existed) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'AUSENTE' }
$dirExisted = Test-Path $dir
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion; cierralo (no se toca).' }
if ($existed) { Copy-Item $settings $bak -Force }
New-Item -ItemType Directory -Force $Out | Out-Null
$spec = @{}
foreach ($v in ($Variants -split ';')) { $n, $p = $v -split '=', 2; $spec[$n] = $p }
$names = [string[]]$spec.Keys
try {
  $env:MIEL_FAST = '1'; $env:MIEL_SEED = "$Seed"; $env:MIEL_DIAG_SCRIPT = $Script
  for ($r = 1; $r -le $Reps; $r++) {
    $order = [string[]]($names | Get-Random -Count $names.Count -SetSeed (900 + $r))
    foreach ($v in $order) {
      Remove-Item $settings -Force -ErrorAction SilentlyContinue
      $log = Join-Path $Out "$v-r$r.log"
      $p = Start-Process $spec[$v] -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out"
      Start-Sleep -Seconds $Seconds
      foreach ($id in @(Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $p.Id } | ForEach-Object ProcessId)) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
      Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
      Start-Sleep -Seconds 3
      Write-Output ("{0} rep {1} ok" -f $v, $r)
    }
  }
}
finally {
  Clear-PersonalPathsInDir $Out   # saneado en el origen: ninguna ruta personal llega a los registros
  Get-Process miel -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item Env:\MIEL_FAST, Env:\MIEL_SEED, Env:\MIEL_DIAG_SCRIPT -ErrorAction SilentlyContinue
  if ($existed) { Copy-Item $bak $settings -Force; Remove-Item $bak -Force } else {
    Remove-Item $settings -Force -ErrorAction SilentlyContinue
    if (-not $dirExisted) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
  }
}
if ($existed) { "settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $hash0)" } else { "settings ausentes como al inicio: $(-not (Test-Path $settings))" }
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
