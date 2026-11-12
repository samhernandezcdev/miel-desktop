<#
E5 (SOLO medicion): coste del planificador con «Sueno» activado y apagado, MIDIENDO miel.exe SOLO (el proceso raiz; sin los
procesos de WebView2), con el metodo de E1a/E4b: binario de PRODUCCION con `diag` (solo anade registro y guiones), MIEL_FAST=1,
follow_off@1, muestreo de ~1 s del CPU acumulado, repeticiones intercaladas. La inactividad la inyecta la sonda programada
(MIEL_DIAG_IDLE=0@0: crece desde el arranque; nunca se consulta a Windows) para que el resultado no dependa de si la persona esta
delante del equipo. No toca el cursor ni el teclado. Respalda y restaura settings.json y no deja procesos. Saneado en el origen.
Mas fiable que el CPU para cifras tan bajas: el contador de DESPERTARES del planificador (linea `despertar <estado>` del registro
de diag, una por cada vez que el hilo del motor despierta), que analyze_e5.py reduce a despertares por minuto y estado.

Variantes (el MISMO binario; solo cambian el ajuste sembrado y el guion). `_on`: sleep_enabled = true; `_off`: false.
  awake_on / awake_off   Idle, Rest y Reaction normales (se separan por estado en el analisis)
  sleeping               duerme (umbral de diag de 20 s) y sigue durmiendo: sondeo ligero
  suppressed_on/_off     `suppress@20` (diag): oculta por supresion (el sueno no sondea)
  hidden_on/_off         `hide@20`: oculta por la persona
  paused_on/_off         `pause@20`: pausada por la persona
dnd_auto_enabled = false en todas (No molestar se midio en E4b).
Salida: <Out>-samples.csv (variant, rep, t_ms, dt_ms, cpu_pct, procs) y <Out>-logs\<variante>-r<N>.log
Analisis: python -I tools/e5/analyze_e5.py <Out>
#>
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Out,
  [int]$Reps = 4, [int]$Seconds = 150, [int]$WarmupSec = 12, [int]$Seed = 2026,
  [string]$Variants = 'awake_on,awake_off,sleeping,suppressed_on,suppressed_off,hidden_on,hidden_off,paused_on,paused_off'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$dir = Split-Path $settings
$existed = Test-Path $settings
$hash0 = if ($existed) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'AUSENTE' }
$dirExisted = Test-Path $dir
$bak = Join-Path $env:TEMP 'miel-settings-e5-measure.bak'
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion; cierralo (no se toca).' }
if ($existed) { Copy-Item $settings $bak -Force } else { New-Item -ItemType Directory -Force $dir | Out-Null }
$logs = "$Out-logs"; New-Item -ItemType Directory -Force $logs | Out-Null
$csv = "$Out-samples.csv"
$names = [string[]]($Variants -split ',')
$spec = @{
  awake_on       = @{ sleep = 'true';  script = 'follow_off@1';                secs = '' }
  awake_off      = @{ sleep = 'false'; script = 'follow_off@1';                secs = '' }
  sleeping       = @{ sleep = 'true';  script = 'follow_off@1';                secs = '20' }
  suppressed_on  = @{ sleep = 'true';  script = 'follow_off@1,suppress@20';    secs = '' }
  suppressed_off = @{ sleep = 'false'; script = 'follow_off@1,suppress@20';    secs = '' }
  hidden_on      = @{ sleep = 'true';  script = 'follow_off@1,hide@20';        secs = '' }
  hidden_off     = @{ sleep = 'false'; script = 'follow_off@1,hide@20';        secs = '' }
  paused_on      = @{ sleep = 'true';  script = 'follow_off@1,pause@20';       secs = '' }
  paused_off     = @{ sleep = 'false'; script = 'follow_off@1,pause@20';       secs = '' }
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
      Write-Utf8NoBom $settings ('{{"schema_version":2,"messages_enabled":false,"follow_enabled":false,"reduced_motion":false,"paused":false,"pet_size":96,"dnd_auto_enabled":false,"sleep_enabled":{0}}}' -f $spec[$v].sleep)
      $env:MIEL_DIAG_SCRIPT = $spec[$v].script
      $env:MIEL_DIAG_IDLE = '0@0'
      if ($spec[$v].secs -ne '') { $env:MIEL_DIAG_SLEEP_SECS = $spec[$v].secs } else { Remove-Item Env:\MIEL_DIAG_SLEEP_SECS -ErrorAction SilentlyContinue }
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
  Remove-Item Env:\MIEL_FAST, Env:\MIEL_DIAG_SCRIPT, Env:\MIEL_DIAG_IDLE, Env:\MIEL_DIAG_SLEEP_SECS -ErrorAction SilentlyContinue
  Clear-PersonalPathsInDir $logs   # saneado en el origen
  if (Test-Path $csv) { Clear-PersonalPaths -Path $csv }
  if ($existed) { Copy-Item $bak $settings -Force; Remove-Item $bak -Force } else {
    Remove-Item $settings -Force -ErrorAction SilentlyContinue
    if (-not $dirExisted) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
  }
}
if ($existed) { "settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $hash0)" } else { "settings ausentes como al inicio: $(-not (Test-Path $settings))" }
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
