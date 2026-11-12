<#
E3 (SOLO medicion): costo de la ventana de ajustes con el MISMO metodo que tools/e0b/prodrun.ps1 (binarios de PRODUCCION
con custom-protocol + diag, MIEL_FAST=1, guion follow_off@1, muestreo de ~1 s del CPU acumulado de TODO el arbol miel.exe +
WebView2, repeticiones intercaladas). No toca el cursor ni el teclado. Respalda y restaura settings.json (SHA-256) y no deja
procesos en marcha.

Variantes (-Variants "nombre=ruta.exe[:ui]"): con ':ui' la variante recibe MIEL_DIAG_UI_SCRIPT = abrir ajustes en -OpenAt y
cerrarlos en -CloseAt; sin ':ui' la ventana nunca se abre.
  Fases: A = sin ventana (antes de abrir) · B = ventana abierta · C = tras cerrarla.
Salida: <Out>-samples.csv (variant, rep, t_ms, dt_ms, cpu_pct, procs, private_mb, ws_mb) y <Out>-logs\<variante>-r<N>.log
#>
param(
  [Parameter(Mandatory)][string]$Variants,   # 'nombre=ruta[:ui];nombre=ruta...' (separadas por ;)
  [Parameter(Mandatory)][string]$Out,
  [int]$Reps = 4, [int]$Seconds = 230, [int]$WarmupSec = 12, [int]$OpenAt = 65, [int]$CloseAt = 125, [int]$Seed = 2026
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')   # Write-Utf8NoBom / Clear-PersonalPaths
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$bak = Join-Path $env:TEMP 'miel-settings-e3-measure.bak'
$existed = Test-Path $settings
$hash0 = if ($existed) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'AUSENTE' }
$dirExisted = Test-Path (Split-Path $settings)
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion; cierralo (no se toca).' }
if ($existed) { Copy-Item $settings $bak -Force } else { New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null; Write-Utf8NoBom $bak '{"schema_version":2,"messages_enabled":true,"follow_enabled":true,"reduced_motion":false,"paused":false,"pet_size":96}' }
$logs = "$Out-logs"; New-Item -ItemType Directory -Force $logs | Out-Null
$csv = "$Out-samples.csv"

$spec = @{}
foreach ($v in ($Variants -split ';')) {
  $name, $rest = $v -split '=', 2
  $ui = $rest.EndsWith(':ui'); $exe = if ($ui) { $rest.Substring(0, $rest.Length - 3) } else { $rest }
  $spec[$name] = @{ exe = $exe; ui = $ui }
}
$names = [string[]]$spec.Keys

function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
  $set = [System.Collections.Generic.HashSet[int]]::new(); [void]$set.Add($root)
  do { $n = $set.Count; foreach ($p in $all) { if ($set.Contains([int]$p.ParentProcessId)) { [void]$set.Add([int]$p.ProcessId) } } } while ($set.Count -ne $n)
  @($set)
}

try {
  $s = Get-Content $bak -Raw | ConvertFrom-Json
  $s.messages_enabled = $false; $s.follow_enabled = $false; $s.reduced_motion = $false; $s.paused = $false; $s.pet_size = 96
  Write-Utf8NoBom $settings ($s | ConvertTo-Json)
  $env:MIEL_FAST = '1'
  $env:MIEL_DIAG_SCRIPT = 'follow_off@1'
  $run = 0; $total = $Reps * $names.Count
  for ($r = 1; $r -le $Reps; $r++) {
    $order = [string[]]($names | Get-Random -Count $names.Count -SetSeed ($Seed + $r))
    foreach ($v in $order) {
      $run++
      if ($spec[$v].ui) { $env:MIEL_DIAG_UI_SCRIPT = "open_settings@$OpenAt,close_settings@$CloseAt" } else { Remove-Item Env:\MIEL_DIAG_UI_SCRIPT -ErrorAction SilentlyContinue }
      $log = Join-Path $logs "$v-r$r.log"
      $p = Start-Process $spec[$v].exe -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out"
      Start-Sleep -Seconds $WarmupSec
      $prev = @{}; $prevT = $null; $ids = @(Get-Tree $p.Id); $tick = 0
      $t0 = [DateTimeOffset]::UtcNow
      while (([DateTimeOffset]::UtcNow - $t0).TotalSeconds -lt ($Seconds - $WarmupSec)) {
        if ($p.HasExited) { break }
        $now = [DateTimeOffset]::UtcNow; $tMs = $now.ToUnixTimeMilliseconds()
        if ($tick % 3 -eq 0) { $ids = @(Get-Tree $p.Id) }   # el arbol cambia al abrir/cerrar: se relee cada 3 muestras
        $cur = @{}; $priv = 0.0; $ws = 0.0
        foreach ($id in $ids) { try { $q = Get-Process -Id $id -ErrorAction Stop; $cur[$id] = $q.TotalProcessorTime.TotalMilliseconds; $priv += $q.PrivateMemorySize64; $ws += $q.WorkingSet64 } catch {} }
        if ($prevT) {
          $dt = ($now - $prevT).TotalMilliseconds; $d = 0.0
          foreach ($k in $cur.Keys) { if ($prev.ContainsKey($k)) { $d += $cur[$k] - $prev[$k] } }
          [pscustomobject]@{ variant = $v; rep = $r; t_ms = $tMs; dt_ms = [math]::Round($dt, 1); cpu_pct = [math]::Round($d / $dt * 100, 3); procs = $cur.Count; private_mb = [math]::Round($priv / 1MB, 1); ws_mb = [math]::Round($ws / 1MB, 1) } |
            Export-Csv -Append -NoTypeInformation -Encoding UTF8 $csv
        }
        $prev = $cur; $prevT = $now; $tick++
        Start-Sleep -Milliseconds 900
      }
      foreach ($id in ((Get-Tree $p.Id) | Sort-Object -Descending)) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
      Start-Sleep -Seconds 3
      Write-Output ("run {0}/{1} {2} rep {3} ok" -f $run, $total, $v, $r)
    }
  }
}
finally {
  Clear-PersonalPathsInDir $logs   # saneado en el origen: ninguna ruta personal llega a los registros
  Get-Process miel -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  if ($existed) { Copy-Item $bak $settings -Force } else {
    Remove-Item $settings -Force -ErrorAction SilentlyContinue
    if (-not $dirExisted) { Remove-Item (Split-Path $settings) -Recurse -Force -ErrorAction SilentlyContinue }
  }
  Remove-Item Env:\MIEL_FAST, Env:\MIEL_DIAG_SCRIPT, Env:\MIEL_DIAG_UI_SCRIPT -ErrorAction SilentlyContinue
}
if ($existed) { "settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $hash0)" } else { "settings ausentes como al inicio: $(-not (Test-Path $settings))" }
Remove-Item $bak -Force
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
