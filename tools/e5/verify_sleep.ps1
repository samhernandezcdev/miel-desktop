<#
E5 (SOLO verificacion en la app REAL, binario `diag` de produccion): comprueba que Miel se duerme por inactividad, despierta
con actividad, duerme al instante al bloquear, despierta al desbloquear, que el interruptor y el tiempo de «Sueno» actuan en
vivo y que ocultar/mostrar desde la bandeja termina el sueno. NO toca el sistema: la inactividad la inyecta MIEL_DIAG_IDLE
(sonda programada, con umbral en SEGUNDOS por MIEL_DIAG_SLEEP_SECS) en el motor real; los avisos de bloqueo/suspension y las
ordenes del motor, MIEL_DIAG_SCRIPT; los ajustes, MIEL_DIAG_UI_SCRIPT. NO genera ni mueve entradas reales (ni raton ni teclado).

Cada 0,5 s anota si la ventana principal ("Miel") esta visible (IsWindowVisible, solo lectura sobre el proceso propio).
Respalda y restaura settings.json (queda ausente si estaba ausente) y no deja procesos. Saneado de rutas en el origen.
Salida: <Out>.csv (t_ms, visible), <Out>.log (stderr del binario) y <Out>-meta.json (marcas de tiempo del arranque).
Analisis: python -I tools/e5/analyze_sleep.py <Out>
#>
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Out,
  [int]$Seconds = 205,
  [string]$Idle = '0@0,0@45,0@100,0@125',
  [string]$SleepSecs = '20',
  [string]$Script = 'follow_off@1,lock@105,unlock@125,hide@150,show@160',
  [string]$Ui = 'set:sleep_enabled=false@75,set:sleep_enabled=true@85,set:sleep_after_minutes=30@185'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$dir = Split-Path $settings
$existed = Test-Path $settings
$hash0 = if ($existed) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'AUSENTE' }
$dirExisted = Test-Path $dir
$bak = Join-Path $env:TEMP 'miel-settings-e5-verify.bak'
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion; cierralo (no se toca).' }
if ($existed) { Copy-Item $settings $bak -Force }

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class Vis {
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  // true si hay una ventana de nivel superior del proceso con ese titulo exacto que este visible; null si no existe
  public static bool? MainVisible(uint pid, string title) {
    bool? res = null;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid) return true;
      var sb = new StringBuilder(256); GetWindowText(h, sb, 256);
      if (sb.ToString() != title) return true;
      res = (res ?? false) || IsWindowVisible(h);
      return true;
    }, IntPtr.Zero);
    return res;
  }
}
'@

$csv = "$Out.csv"; $log = "$Out.log"
Remove-Item $csv, $log, "$Out-meta.json" -ErrorAction SilentlyContinue
try {
  # valores por defecto salvo No molestar apagado: su sondeo (cada 2,5 s) ensuciaria el recuento de despertares del sueno
  New-Item -ItemType Directory -Force $dir | Out-Null
  Write-Utf8NoBom $settings '{"schema_version":2,"dnd_auto_enabled":false}'
  $env:MIEL_DIAG_IDLE = $Idle
  $env:MIEL_DIAG_SLEEP_SECS = $SleepSecs
  $env:MIEL_DIAG_SCRIPT = $Script
  $env:MIEL_DIAG_UI_SCRIPT = $Ui
  $startMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
  $p = Start-Process $Exe -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out"
  $t0 = [DateTimeOffset]::UtcNow
  $rows = New-Object System.Collections.Generic.List[string]
  $rows.Add('t_ms,visible')
  while (([DateTimeOffset]::UtcNow - $t0).TotalSeconds -lt $Seconds) {
    if ($p.HasExited) { break }
    $v = [Vis]::MainVisible([uint32]$p.Id, 'Miel')
    $rows.Add(('{0},{1}' -f [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(), $(if ($null -eq $v) { 'none' } elseif ($v) { '1' } else { '0' })))
    Start-Sleep -Milliseconds 500
  }
  $exited = $p.HasExited
  Write-Utf8NoBom "$Out.csv" (($rows -join "`n") + "`n")
  Write-Utf8NoBom "$Out-meta.json" (('{{"start_ms":{0},"seconds":{1},"exited_early":{2},"idle":"{3}","sleep_secs":"{4}","script":"{5}","ui":"{6}"}}' -f $startMs, $Seconds, $exited.ToString().ToLower(), $Idle, $SleepSecs, $Script, $Ui) + "`n")
  foreach ($id in @(Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $p.Id } | ForEach-Object ProcessId)) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
  Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 3
}
finally {
  Get-Process miel -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item Env:\MIEL_DIAG_IDLE, Env:\MIEL_DIAG_SLEEP_SECS, Env:\MIEL_DIAG_SCRIPT, Env:\MIEL_DIAG_UI_SCRIPT -ErrorAction SilentlyContinue
  foreach ($f in @($log, "$log.out", "$Out-meta.json", $csv)) { Clear-PersonalPaths -Path $f }
  if ($existed) { New-Item -ItemType Directory -Force $dir | Out-Null; Copy-Item $bak $settings -Force; Remove-Item $bak -Force } else {
    Remove-Item $settings -Force -ErrorAction SilentlyContinue
    if (-not $dirExisted) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
  }
}
if ($existed) { "settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $hash0)" } else { "settings ausentes como al inicio: $(-not (Test-Path $settings))" }
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
