<#
.SYNOPSIS
  Mide CPU y memoria de Miel (miel.exe + SUS procesos WebView2) de forma reproducible.

.DESCRIPTION
  - Lanza el ejecutable indicado (o se adjunta a uno ya en marcha con -AttachPid).
  - Identifica el ARBOL de procesos de Miel (miel.exe y descendientes por PPID). NO suma los
    msedgewebview2.exe del equipo que pertenezcan a otras aplicaciones.
  - Cada -IntervalSec registra por proceso: PID, PPID, CPU acumulada, Working Set y Private Bytes.
  - Escribe dos CSV: <Out>-procs.csv (una fila por proceso y muestra) y <Out>-samples.csv (agregado).
  - NO toca el cursor, el teclado, la resolucion ni la configuracion del equipo.

  Aviso: sumar Working Sets cuenta varias veces las paginas compartidas entre procesos WebView2
  (DLL de Chromium, etc.); la suma NO es la memoria fisica exclusiva de Miel. Private Bytes
  (memoria comprometida privada de cada proceso) es la cifra mas fiable para detectar fugas.

.EXAMPLE
  .\tools\measure.ps1 -Exe .\src-tauri\target\release\miel.exe -Seconds 600 -IntervalSec 5 -Out .\measure\idle
  # con diagnostico (build --features diag): marcas de tiempo del motor en <Out>-engine.log
  $env:MIEL_FAST='1'; .\tools\measure.ps1 -Exe ... -Seconds 600 -Out .\measure\fast
#>
param(
  [string]$Exe,
  [int]$AttachPid = 0,
  [int]$Seconds = 300,
  [int]$IntervalSec = 5,
  [int]$WarmupSec = 20,
  [Parameter(Mandatory = $true)][string]$Out,
  [switch]$KeepRunning
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Out) | Out-Null
$cores = [Environment]::ProcessorCount

function Get-MielTree([int]$RootPid) {
  $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name
  $byParent = $all | Group-Object ParentProcessId -AsHashTable -AsString
  $ids = New-Object System.Collections.Generic.List[int]
  $queue = New-Object System.Collections.Generic.Queue[int]; $queue.Enqueue($RootPid); $ids.Add($RootPid)
  while ($queue.Count -gt 0) {
    $p = $queue.Dequeue()
    foreach ($c in @($byParent["$p"])) { if ($c -and -not $ids.Contains([int]$c.ProcessId)) { $ids.Add([int]$c.ProcessId); $queue.Enqueue([int]$c.ProcessId) } }
  }
  $ppid = @{}; foreach ($a in $all) { $ppid[[int]$a.ProcessId] = [int]$a.ParentProcessId }
  foreach ($i in $ids) { [pscustomobject]@{ Pid = $i; Ppid = $ppid[$i] } }
}

$proc = $null
if ($AttachPid -gt 0) { $proc = Get-Process -Id $AttachPid }
else {
  if (-not (Test-Path $Exe)) { throw "No existe $Exe" }
  $proc = Start-Process -FilePath $Exe -PassThru -RedirectStandardError "$Out-engine.log" -RedirectStandardOutput "$Out-stdout.log" -WindowStyle Hidden
}
"Miel PID $($proc.Id); nucleos logicos: $cores; calentamiento $WarmupSec s; muestreo cada $IntervalSec s durante $Seconds s"
Start-Sleep -Seconds $WarmupSec

$procRows = New-Object System.Collections.Generic.List[object]
$sampleRows = New-Object System.Collections.Generic.List[object]
$prev = @{}; $prevT = $null
$t0 = [DateTimeOffset]::UtcNow
while (([DateTimeOffset]::UtcNow - $t0).TotalSeconds -lt $Seconds) {
  if ($proc.HasExited) { "Miel termino durante la medicion (codigo $($proc.ExitCode))"; break }
  $now = [DateTimeOffset]::UtcNow; $tMs = $now.ToUnixTimeMilliseconds()
  $tree = @(Get-MielTree $proc.Id)
  $cpuNow = @{}; $sumWs = 0.0; $sumPriv = 0.0; $webviews = 0
  foreach ($n in $tree) {
    try { $p = Get-Process -Id $n.Pid -ErrorAction Stop } catch { continue }
    $role = if ($n.Pid -eq $proc.Id) { 'miel' } else { 'webview2' }
    if ($role -eq 'webview2') { $webviews++ }
    $cpuMs = $p.TotalProcessorTime.TotalMilliseconds
    $ws = $p.WorkingSet64 / 1MB; $priv = $p.PrivateMemorySize64 / 1MB
    $cpuNow[$n.Pid] = $cpuMs; $sumWs += $ws; $sumPriv += $priv
    $pct = $null
    if ($prev.ContainsKey($n.Pid) -and $prevT) { $dt = ($now - $prevT).TotalMilliseconds; if ($dt -gt 0) { $pct = [math]::Round(($cpuMs - $prev[$n.Pid]) / $dt * 100, 3) } }
    $procRows.Add([pscustomobject]@{ t_ms = $tMs; role = $role; pid = $n.Pid; ppid = $n.Ppid; name = $p.ProcessName; cpu_ms = [math]::Round($cpuMs, 1); cpu_pct_core = $pct; ws_mb = [math]::Round($ws, 1); private_mb = [math]::Round($priv, 1) })
  }
  $totPct = $null
  if ($prevT) { $dt = ($now - $prevT).TotalMilliseconds; $d = 0.0; foreach ($k in $cpuNow.Keys) { if ($prev.ContainsKey($k)) { $d += $cpuNow[$k] - $prev[$k] } }; if ($dt -gt 0) { $totPct = [math]::Round($d / $dt * 100, 3) } }
  $sampleRows.Add([pscustomobject]@{ t_ms = $tMs; processes = $tree.Count; webview2_procs = $webviews; cpu_pct_of_one_core = $totPct; ws_sum_mb = [math]::Round($sumWs, 1); private_sum_mb = [math]::Round($sumPriv, 1) })
  $prev = $cpuNow; $prevT = $now
  Start-Sleep -Seconds $IntervalSec
}
$procRows | Export-Csv -NoTypeInformation -Path "$Out-procs.csv"
$sampleRows | Export-Csv -NoTypeInformation -Path "$Out-samples.csv"
"Muestras: $($sampleRows.Count) -> $Out-samples.csv / $Out-procs.csv"
if (-not $KeepRunning -and $AttachPid -eq 0 -and -not $proc.HasExited) {
  # cierre controlado de la ventana principal; si no responde, se termina el proceso
  $null = $proc.CloseMainWindow(); if (-not $proc.WaitForExit(3000)) { Stop-Process -Id $proc.Id -Force }
}
