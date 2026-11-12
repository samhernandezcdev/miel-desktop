<#
E4b (SOLO verificacion en la app REAL, binario `diag` de produccion): comprueba que la ventana de Miel se oculta, reaparece,
que «Mostrar de todos modos» la devuelve, que apagar/encender el interruptor actua en vivo y que ocultar a mano detiene el
sondeo. NO toca el sistema: el estado de notificacion lo inyecta MIEL_DIAG_NOTIF (sonda programada) en el motor real; las
ordenes del motor y de ajustes las inyectan MIEL_DIAG_SCRIPT y MIEL_DIAG_UI_SCRIPT. No toca el cursor ni el teclado.

Cada 0,5 s anota si la ventana principal ("Miel") esta visible (IsWindowVisible, solo lectura sobre el proceso propio).
Respalda y restaura settings.json (queda ausente si estaba ausente) y no deja procesos. Saneado de rutas en el origen.
Salida: <Out>.csv (t_ms, visible), <Out>.log (stderr del binario) y <Out>-meta.json (marcas de tiempo del arranque).
Analisis: python -I tools/e4/analyze_dnd.py <Out>
#>
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Out,
  [int]$Seconds = 200,
  [string]$Notif = 'busy@15,ok@45,busy@60,ok@110,busy@125,ok@175',
  [string]$Script = 'follow_off@1,override@75,hide@165,show@185',
  [string]$Ui = 'set:dnd_auto_enabled=false@140,set:dnd_auto_enabled=true@150'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$dir = Split-Path $settings
$existed = Test-Path $settings
$hash0 = if ($existed) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'AUSENTE' }
$dirExisted = Test-Path $dir
$bak = Join-Path $env:TEMP 'miel-settings-e4-verify.bak'
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
  Remove-Item $settings -Force -ErrorAction SilentlyContinue   # mismo punto de partida: valores por defecto (dnd_auto_enabled = true)
  $env:MIEL_DIAG_NOTIF = $Notif
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
  Write-Utf8NoBom "$Out-meta.json" (('{{"start_ms":{0},"seconds":{1},"exited_early":{2},"notif":"{3}","script":"{4}","ui":"{5}"}}' -f $startMs, $Seconds, $exited.ToString().ToLower(), $Notif, $Script, $Ui) + "`n")
  foreach ($id in @(Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $p.Id } | ForEach-Object ProcessId)) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
  Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 3
}
finally {
  Get-Process miel -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item Env:\MIEL_DIAG_NOTIF, Env:\MIEL_DIAG_SCRIPT, Env:\MIEL_DIAG_UI_SCRIPT -ErrorAction SilentlyContinue
  foreach ($f in @($log, "$log.out", "$Out-meta.json", $csv)) { Clear-PersonalPaths -Path $f }
  if ($existed) { New-Item -ItemType Directory -Force $dir | Out-Null; Copy-Item $bak $settings -Force; Remove-Item $bak -Force } else {
    Remove-Item $settings -Force -ErrorAction SilentlyContinue
    if (-not $dirExisted) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
  }
}
if ($existed) { "settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $hash0)" } else { "settings ausentes como al inicio: $(-not (Test-Path $settings))" }
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
