<#
E0b Fase 2 (SOLO medicion): compara binarios de PRODUCCION (release + custom-protocol; con la caracteristica `diag`
solo para el guion de pausa/ocultar y el registro de estados del motor) con el MISMO metodo y repeticiones
intercaladas. No toca el cursor ni el teclado. Respalda y restaura settings.json (SHA-256). No deja procesos en marcha.

Cada ejecucion (variante x repeticion):
  - MIEL_FAST=1 (acelera los plazos del motor; no cambia el CSS) y guion follow_off@1, pause@185, resume@205, hide@210, show@230.
  - Muestreo cada 1 s del CPU acumulado de TODO el arbol de procesos de ese miel.exe (miel.exe + WebView2).
  - El registro del motor (stderr) delimita los estados; python analyze_prod.py asigna cada muestra al estado vigente.
Salida: <Out>-samples.csv y <Out>-logs\<variante>-r<N>.log
#>
param(
  [Parameter(Mandatory)][string[]]$Variants,   # nombres de .exe en $Dir (sin extension)
  [Parameter(Mandatory)][string]$Dir,
  [Parameter(Mandatory)][string]$Out,
  [int]$Reps = 5, [int]$Seconds = 238, [int]$WarmupSec = 12, [int]$Seed = 2026
)
$ErrorActionPreference = 'Stop'
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$bak = Join-Path $env:TEMP 'miel-settings-e0b-prod.bak'
$hash0 = (Get-FileHash $settings -Algorithm SHA256).Hash
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion; cierralo (no se toca).' }
Copy-Item $settings $bak -Force
$logs = "$Out-logs"; New-Item -ItemType Directory -Force $logs | Out-Null
$csv = "$Out-samples.csv"

function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
  $set = [System.Collections.Generic.HashSet[int]]::new(); [void]$set.Add($root)
  do { $n = $set.Count; foreach ($p in $all) { if ($set.Contains([int]$p.ParentProcessId)) { [void]$set.Add([int]$p.ProcessId) } } } while ($set.Count -ne $n)
  @($set)
}

try {
  $s = Get-Content $bak -Raw | ConvertFrom-Json
  $s.messages_enabled = $false; $s.follow_enabled = $false; $s.reduced_motion = $false; $s.paused = $false; $s.pet_size = 96
  ($s | ConvertTo-Json) | Set-Content $settings -Encoding UTF8
  $env:MIEL_FAST = '1'
  $env:MIEL_DIAG_SCRIPT = 'follow_off@1,pause@185,resume@205,hide@210,show@230'
  $run = 0
  $total = $Reps * $Variants.Count
  for ($r = 1; $r -le $Reps; $r++) {
    $order = [string[]]($Variants | Get-Random -Count $Variants.Count -SetSeed ($Seed + $r))
    foreach ($v in $order) {
      $run++
      $exe = Join-Path $Dir "$v.exe"
      $log = Join-Path $logs "$v-r$r.log"
      $p = Start-Process $exe -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out"
      Start-Sleep -Seconds $WarmupSec
      $prev = @{}; $prevT = $null; $ids = @(Get-Tree $p.Id); $tick = 0
      $t0 = [DateTimeOffset]::UtcNow
      while (([DateTimeOffset]::UtcNow - $t0).TotalSeconds -lt ($Seconds - $WarmupSec)) {
        if ($p.HasExited) { break }
        $now = [DateTimeOffset]::UtcNow; $tMs = $now.ToUnixTimeMilliseconds()
        if ($tick % 5 -eq 0) { $ids = @(Get-Tree $p.Id) }
        $cur = @{}; $priv = 0.0
        foreach ($id in $ids) { try { $q = Get-Process -Id $id -ErrorAction Stop; $cur[$id] = $q.TotalProcessorTime.TotalMilliseconds; $priv += $q.PrivateMemorySize64 } catch {} }
        if ($prevT) {
          $dt = ($now - $prevT).TotalMilliseconds; $d = 0.0
          foreach ($k in $cur.Keys) { if ($prev.ContainsKey($k)) { $d += $cur[$k] - $prev[$k] } else { $d += 0 } }
          [pscustomobject]@{ variant = $v; rep = $r; t_ms = $tMs; dt_ms = [math]::Round($dt, 1); cpu_pct = [math]::Round($d / $dt * 100, 3); procs = $cur.Count; private_mb = [math]::Round($priv / 1MB, 1) } |
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
  Get-Process miel -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Dir*" } | Stop-Process -Force -ErrorAction SilentlyContinue
  Copy-Item $bak $settings -Force
  Remove-Item Env:\MIEL_FAST, Env:\MIEL_DIAG_SCRIPT -ErrorAction SilentlyContinue
}
"settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $hash0)"
Remove-Item $bak -Force
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
