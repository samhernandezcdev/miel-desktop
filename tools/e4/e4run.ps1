<#
E4b (SOLO medicion): coste de No molestar automatico MIDIENDO miel.exe SOLO (el proceso raiz; sin los procesos de WebView2),
con el mismo metodo de E1a/E4a: binario de PRODUCCION con `diag` (solo anade registro y guiones), MIEL_FAST=1,
follow_off@1, muestreo de ~1 s del CPU acumulado, repeticiones intercaladas. No toca el cursor ni el teclado.
Respalda y restaura settings.json (queda ausente si estaba ausente) y no deja procesos. Saneado de rutas en el origen.

Variantes (todas con el MISMO binario; solo cambian el ajuste sembrado y el guion):
  dnd_on      dnd_auto_enabled = true  (sondeo de No molestar activo, estados Idle/Rest normales)
  dnd_off     dnd_auto_enabled = false (sin sondeo)
  suppressed  dnd_auto_enabled = true  + `suppress@20` (diag): oculta por supresion; el sondeo REAL de Windows sigue (recuperacion)
  hidden      dnd_auto_enabled = true  + `hide@20`: oculta por la persona; el sondeo debe estar detenido
  paused      dnd_auto_enabled = true  + `pause@20`: pausada por la persona; el sondeo debe estar detenido
Salida: <Out>-samples.csv (variant, rep, t_ms, dt_ms, cpu_pct, procs) y <Out>-logs\<variante>-r<N>.log
Analisis: python -I tools/e4/analyze_e4m.py <Out>
#>
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Out,
  [int]$Reps = 4, [int]$Seconds = 200, [int]$WarmupSec = 12, [int]$Seed = 2026,
  [string]$Variants = 'dnd_on,dnd_off,suppressed,hidden,paused'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$dir = Split-Path $settings
$existed = Test-Path $settings
$hash0 = if ($existed) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'AUSENTE' }
$dirExisted = Test-Path $dir
$bak = Join-Path $env:TEMP 'miel-settings-e4-measure.bak'
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion; cierralo (no se toca).' }
if ($existed) { Copy-Item $settings $bak -Force } else { New-Item -ItemType Directory -Force $dir | Out-Null }
$logs = "$Out-logs"; New-Item -ItemType Directory -Force $logs | Out-Null
$csv = "$Out-samples.csv"
$names = [string[]]($Variants -split ',')
$spec = @{
  dnd_on     = @{ dnd = 'true';  script = 'follow_off@1' }
  dnd_off    = @{ dnd = 'false'; script = 'follow_off@1' }
  suppressed = @{ dnd = 'true';  script = 'follow_off@1,suppress@20' }
  hidden     = @{ dnd = 'true';  script = 'follow_off@1,hide@20' }
  paused     = @{ dnd = 'true';  script = 'follow_off@1,pause@20' }
}

function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
  $set = [System.Collections.Generic.HashSet[int]]::new(); [void]$set.Add($root)
  do { $n = $set.Count; foreach ($p in $all) { if ($set.Contains([int]$p.ParentProcessId)) { [void]$set.Add([int]$p.ProcessId) } } } while ($set.Count -ne $n)
  @($set)
}

try {
  $env:MIEL_FAST = '1'
  $run = 0; $total = $Reps * $names.Count
  for ($r = 1; $r -le $Reps; $r++) {
    $order = [string[]]($names | Get-Random -Count $names.Count -SetSeed ($Seed + $r))
    foreach ($v in $order) {
      $run++
      Write-Utf8NoBom $settings ('{{"schema_version":2,"messages_enabled":false,"follow_enabled":false,"reduced_motion":false,"paused":false,"pet_size":96,"dnd_auto_enabled":{0}}}' -f $spec[$v].dnd)
      $env:MIEL_DIAG_SCRIPT = $spec[$v].script
      $log = Join-Path $logs "$v-r$r.log"
      $p = Start-Process $Exe -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out"
      Start-Sleep -Seconds $WarmupSec
      $prev = $null; $prevT = $null
      $t0 = [DateTimeOffset]::UtcNow
      while (([DateTimeOffset]::UtcNow - $t0).TotalSeconds -lt ($Seconds - $WarmupSec)) {
        if ($p.HasExited) { break }
        $now = [DateTimeOffset]::UtcNow; $tMs = $now.ToUnixTimeMilliseconds()
        try { $cur = (Get-Process -Id $p.Id -ErrorAction Stop).TotalProcessorTime.TotalMilliseconds } catch { break }
        if ($null -ne $prev) {
          $dt = ($now - $prevT).TotalMilliseconds
          [pscustomobject]@{ variant = $v; rep = $r; t_ms = $tMs; dt_ms = [math]::Round($dt, 1); cpu_pct = [math]::Round(($cur - $prev) / $dt * 100, 3); procs = @(Get-Tree $p.Id).Count } |
            Export-Csv -Append -NoTypeInformation -Encoding UTF8 $csv
        }
        $prev = $cur; $prevT = $now
        Start-Sleep -Milliseconds 900
      }
      foreach ($id in ((Get-Tree $p.Id) | Sort-Object -Descending)) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
      Start-Sleep -Seconds 3
      Write-Output ("run {0}/{1} {2} rep {3} ok" -f $run, $total, $v, $r)
    }
  }
}
finally {
  Get-Process miel -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item Env:\MIEL_FAST, Env:\MIEL_DIAG_SCRIPT -ErrorAction SilentlyContinue
  Clear-PersonalPathsInDir $logs   # saneado en el origen
  if (Test-Path $csv) { Clear-PersonalPaths -Path $csv }
  if ($existed) { Copy-Item $bak $settings -Force; Remove-Item $bak -Force } else {
    Remove-Item $settings -Force -ErrorAction SilentlyContinue
    if (-not $dirExisted) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
  }
}
if ($existed) { "settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $hash0)" } else { "settings ausentes como al inicio: $(-not (Test-Path $settings))" }
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
