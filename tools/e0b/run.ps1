<#
E0b Fase 1 (SOLO medicion): ejecuta el plan intercalado de variantes CSS contra el build diag (devUrl) y
registra CPU por proceso (miel.exe + arbol WebView2), Private Bytes y GPU Engine por PID. No toca el cursor ni el
teclado. Respalda y restaura settings.json (se comprueba por SHA-256). No deja procesos en marcha.
#>
param(
  [Parameter(Mandatory)][string]$Work,      # carpeta con server.py, css\, plan.json
  [Parameter(Mandatory)][string]$Exe,       # build diag SIN custom-protocol
  [Parameter(Mandatory)][string]$Dist,
  [Parameter(Mandatory)][string]$Out,       # prefijo de salida: docs\measurements\v020-e0b-<nombre>
  [int]$Skip = 0, [int]$Max = 100000,
  [int]$IdleFrom = 12, [int]$IdleTo = 37, [int]$SwitchMs = 40000, [int]$RestFrom = 52, [int]$RestTo = 77,
  [switch]$Fps
)
$ErrorActionPreference = 'Stop'
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$bak = Join-Path $env:TEMP 'miel-settings-e0b.bak'
$hash0 = (Get-FileHash $settings -Algorithm SHA256).Hash
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion; cierralo (no se toca).' }
Copy-Item $settings $bak -Force
$plan = Get-Content (Join-Path $Work 'plan.json') -Raw | ConvertFrom-Json
$stateFile = Join-Path $Work 'state.json'
$srvOut = Join-Path $Work 'srv'
New-Item -ItemType Directory -Force $srvOut | Out-Null
$csvS = "$Out-samples.csv"; $csvP = "$Out-procs.csv"

function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, CommandLine
  $set = [System.Collections.Generic.HashSet[int]]::new(); [void]$set.Add($root)
  do { $n = $set.Count; foreach ($p in $all) { if ($set.Contains([int]$p.ParentProcessId)) { [void]$set.Add([int]$p.ProcessId) } } } while ($set.Count -ne $n)
  $all | Where-Object { $set.Contains([int]$_.ProcessId) }
}
function Role($t) {
  if ($t.Name -eq 'miel.exe') { return 'miel' }
  if ($t.CommandLine -match '--type=([\w-]+)') { return $Matches[1] }
  return 'wv2-browser'
}
function Snap([int]$root) {
  $ts = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() / 1000.0
  $h = @{}
  foreach ($t in (Get-Tree $root)) {
    $p = Get-Process -Id $t.ProcessId -ErrorAction SilentlyContinue
    if ($p) { $h[[int]$t.ProcessId] = @{ cpu = $p.TotalProcessorTime.TotalSeconds; priv = $p.PrivateMemorySize64; role = (Role $t) } }
  }
  @{ ts = $ts; procs = $h }
}
function Window([int]$root, [string]$cfg, [int]$rep, [string]$state, [double]$startAt, [double]$endAt) {
  while (([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() / 1000.0) -lt $startAt) { Start-Sleep -Milliseconds 100 }
  $a = Snap $root
  $n = [int][math]::Floor(($endAt - $a.ts) / 5)
  $gpu = 0.0
  try {
    $c = Get-Counter '\GPU Engine(*)\Utilization Percentage' -SampleInterval 5 -MaxSamples $n -ErrorAction Stop
    $ids = @($a.procs.Keys)
    $per = foreach ($s in $c) { ($s.CounterSamples | Where-Object { $_.InstanceName -match 'pid_(\d+)_' -and ($ids -contains [int]$Matches[1]) } | Measure-Object CookedValue -Sum).Sum }
    $gpu = ($per | Measure-Object -Average).Average
    if ($null -eq $gpu) { $gpu = 0.0 }
  } catch { $gpu = -1 }
  while (([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() / 1000.0) -lt $endAt) { Start-Sleep -Milliseconds 100 }
  $b = Snap $root
  $secs = $b.ts - $a.ts
  $agg = @{ miel = 0.0; 'wv2-browser' = 0.0; renderer = 0.0; 'gpu-process' = 0.0; utility = 0.0; other = 0.0 }
  $priv = 0.0
  foreach ($k in $b.procs.Keys) {
    $c0 = 0.0; if ($a.procs.ContainsKey($k)) { $c0 = $a.procs[$k].cpu }
    $pct = ($b.procs[$k].cpu - $c0) / $secs * 100
    $role = $b.procs[$k].role; $key = if ($agg.ContainsKey($role)) { $role } else { 'other' }
    $agg[$key] += $pct; $priv += $b.procs[$k].priv
    [pscustomobject]@{ cfg = $cfg; rep = $rep; state = $state; pid = $k; role = $role; cpu_pct = [math]::Round($pct, 3); private_mb = [math]::Round($b.procs[$k].priv / 1MB, 1) } |
      Export-Csv -Append -NoTypeInformation -Encoding UTF8 $csvP
  }
  $tot = ($agg.Values | Measure-Object -Sum).Sum
  [pscustomobject]@{ cfg = $cfg; rep = $rep; state = $state; seconds = [math]::Round($secs, 1)
    cpu_total = [math]::Round($tot, 3); cpu_miel = [math]::Round($agg['miel'], 3); cpu_wv2_browser = [math]::Round($agg['wv2-browser'], 3)
    cpu_renderer = [math]::Round($agg['renderer'], 3); cpu_gpu_process = [math]::Round($agg['gpu-process'], 3)
    cpu_utility = [math]::Round($agg['utility'], 3); cpu_other = [math]::Round($agg['other'], 3)
    private_mb = [math]::Round($priv / 1MB, 1); gpu_pct = [math]::Round($gpu, 3); procs = $b.procs.Count } |
    Export-Csv -Append -NoTypeInformation -Encoding UTF8 $csvS
}

$srv = $null
try {
  # ajustes de PRUEBA (mensajes y seguimiento apagados, tamano por defecto); el original ya esta respaldado
  $s = Get-Content $bak -Raw | ConvertFrom-Json
  $s.messages_enabled = $false; $s.follow_enabled = $false; $s.reduced_motion = $false; $s.paused = $false; $s.pet_size = 96
  ($s | ConvertTo-Json) | Set-Content $settings -Encoding UTF8

  $srv = Start-Process python -ArgumentList @('-I', (Join-Path $Work 'server.py'), $Dist, (Join-Path $Work 'css'), $stateFile, $srvOut) -WindowStyle Hidden -PassThru
  Start-Sleep -Seconds 2
  $i = 0
  foreach ($it in $plan) {
    $i++; if ($i -le $Skip) { continue }; if (($i - $Skip) -gt $Max) { break }
    $cfg = $it.cfg; $rep = [int]$it.rep
    @{ cfg = $cfg; first = 'idle'; second = 'resting'; switch_ms = $SwitchMs; fps = [bool]$Fps } | ConvertTo-Json | Set-Content $stateFile -Encoding UTF8
    Remove-Item (Join-Path $srvOut 'pagestart.txt') -ErrorAction SilentlyContinue
    $env:MIEL_DIAG_SCRIPT = 'follow_off@1'
    $p = Start-Process $Exe -PassThru
    $t0 = $null
    for ($w = 0; $w -lt 60; $w++) { Start-Sleep -Milliseconds 500; $f = Join-Path $srvOut 'pagestart.txt'; if (Test-Path $f) { $t0 = [double](Get-Content $f -Raw); break } }
    if ($null -eq $t0) { Write-Output "run $i ($cfg rep $rep): la pagina no cargo; se omite"; Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $p.Id } | Out-Null; }
    else {
      Window $p.Id $cfg $rep 'idle' ($t0 + $IdleFrom) ($t0 + $IdleTo)
      Window $p.Id $cfg $rep 'rest' ($t0 + $RestFrom) ($t0 + $RestTo)
      Write-Output ("run {0}/{1} {2} rep {3} ok" -f $i, $plan.Count, $cfg, $rep)
    }
    foreach ($t in ((Get-Tree $p.Id) | Sort-Object ProcessId -Descending)) { Stop-Process -Id $t.ProcessId -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 4
  }
}
finally {
  if ($srv) { Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue }
  Get-Process miel -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } | Stop-Process -Force -ErrorAction SilentlyContinue
  Copy-Item $bak $settings -Force
  Remove-Item Env:\MIEL_DIAG_SCRIPT -ErrorAction SilentlyContinue
}
$hash1 = (Get-FileHash $settings -Algorithm SHA256).Hash
Remove-Item $bak -Force
"settings identicos: $($hash0 -eq $hash1)"
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
"python server restante: " + @(Get-Process python -ErrorAction SilentlyContinue | Where-Object { $_.Id -eq $srv.Id }).Count
