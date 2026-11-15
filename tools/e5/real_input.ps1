<#
E5 (correccion): prueba del sueno con ENTRADA REAL. SOLO un script de pruebas (jamas compilado en miel.exe ni en src-tauri).

AUTORIZACION ACOTADA del usuario para esta tarea: las UNICAS entradas que genera son
  - 'm': mover el raton 1 pixel y devolverlo (SendInput relativo +1 y -1), y
  - 'k': pulsar y soltar VK_F15 (una tecla sin efecto).
Nada de clics, otras teclas, escritura, cierre, bloqueo ni suspension. Solo en los instantes programados.
NO lee ni registra nada de lo que haga la persona ni captura la pantalla: registra solo marcas de tiempo (ms Unix) y, en el
registro de Miel (binario diag), los numeros de las lecturas de inactividad.
Durante la prueba la persona NO debe tocar el equipo. Imprime un aviso y una cuenta atras de 10 s antes de empezar.

Plan: lista de pasos separados por comas; cada paso es 'm', 'k' (esperar a que duerma, enviar la entrada, esperar a que despierte,
registrar cuanto tarda) o 'hold:N' (esperar a que duerma y NO enviar nada N segundos; debe seguir dormida).
Salida: <Out>.sends.csv (t_ms,tipo,...), <Out>.log (stderr del binario diag), <Out>.summary.txt.
Respalda y restaura settings.json (queda ausente si estaba ausente) y no deja procesos.
#>
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Out,
  [string]$Plan = 'm,m,m,k,k,k',
  [string]$SleepSecs = '20',
  [string]$SettingsJson = '{"schema_version":2,"dnd_auto_enabled":false}',
  [int]$SleepWaitMax = 60,
  [int]$WakeWaitMax = 12,
  [switch]$NoDiagEnv,
  [switch]$Cdp,
  [int]$BlindWaitSecs = 0   # binario SIN diag: no hay registro; espera este tiempo sin entrada y observa el DOM (requiere -Cdp)
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\common.ps1')
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$dir = Split-Path $settings
$existed = Test-Path $settings
$hash0 = if ($existed) { (Get-FileHash $settings -Algorithm SHA256).Hash } else { 'AUSENTE' }
$dirExisted = Test-Path $dir
$bak = Join-Path $env:TEMP 'miel-settings-e5-real.bak'
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion; cierralo (no se toca).' }
if ($existed) { Copy-Item $settings $bak -Force }

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RealInput {
  [StructLayout(LayoutKind.Sequential)] struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] struct KEYBDINPUT { public ushort wVk, wScan; public uint dwFlags, time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit)] struct U { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
  [StructLayout(LayoutKind.Sequential)] struct INPUT { public uint type; public U u; }
  [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint n, INPUT[] i, int size);
  const uint MOUSEEVENTF_MOVE = 1, KEYEVENTF_KEYUP = 2;
  public static uint MouseWiggle() {  // +1 px y -1 px: el cursor queda donde estaba
    var a = new INPUT { type = 0 }; a.u.mi = new MOUSEINPUT { dx = 1, dy = 0, dwFlags = MOUSEEVENTF_MOVE };
    var b = new INPUT { type = 0 }; b.u.mi = new MOUSEINPUT { dx = -1, dy = 0, dwFlags = MOUSEEVENTF_MOVE };
    return SendInput(2, new[] { a, b }, Marshal.SizeOf(typeof(INPUT)));
  }
  public static uint KeyF15() {       // VK_F15 = 0x7E: pulsar y soltar; sin efecto
    var d = new INPUT { type = 1 }; d.u.ki = new KEYBDINPUT { wVk = 0x7E };
    var u = new INPUT { type = 1 }; u.u.ki = new KEYBDINPUT { wVk = 0x7E, dwFlags = KEYEVENTF_KEYUP };
    return SendInput(2, new[] { d, u }, Marshal.SizeOf(typeof(INPUT)));
  }
}
'@

function Now-Ms { [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
function Read-Log([string]$path) {
  $fs = [IO.File]::Open($path, 'Open', 'Read', 'ReadWrite'); $sr = New-Object IO.StreamReader($fs, [Text.Encoding]::UTF8)
  try { $sr.ReadToEnd() } finally { $sr.Dispose(); $fs.Dispose() }
}
function Count-Of([string]$pattern) { ([regex]::Matches((Read-Log $log), $pattern)).Count }

$script:ws = $null
function Cdp-Open {
  # Depuracion LOCAL de WebView2 (127.0.0.1) solo durante la prueba: lee el atributo data-state de la abeja (la cara que se ve).
  $tabs = Invoke-RestMethod 'http://127.0.0.1:9222/json' -TimeoutSec 5
  $page = $tabs | Where-Object { $_.type -eq 'page' } | Select-Object -First 1
  $script:ws = New-Object System.Net.WebSockets.ClientWebSocket
  $script:ws.ConnectAsync([Uri]$page.webSocketDebuggerUrl, [Threading.CancellationToken]::None).Wait()
}
function Cdp-State {
  $expr = "JSON.stringify({s:(document.querySelector('.bee')||{getAttribute:()=>null}).getAttribute('data-state'),v:document.visibilityState,p:!!document.querySelector('.stage[data-paused]')})"
  $msg = '{"id":1,"method":"Runtime.evaluate","params":{"expression":"' + ($expr -replace '"','\"') + '","returnByValue":true}}'
  $b = [Text.Encoding]::UTF8.GetBytes($msg)
  $script:ws.SendAsync([ArraySegment[byte]]$b, 'Text', $true, [Threading.CancellationToken]::None).Wait()
  $buf = New-Object byte[] 8192; $sb = ''
  do { $r = $script:ws.ReceiveAsync([ArraySegment[byte]]$buf, [Threading.CancellationToken]::None).Result; $sb += [Text.Encoding]::UTF8.GetString($buf, 0, $r.Count) } until ($r.EndOfMessage)
  ($sb | ConvertFrom-Json).result.result.value
}

$log = "$Out.log"
$sends = New-Object System.Collections.Generic.List[string]
$sends.Add('t_ms,paso,tipo,resultado,detalle')
$summary = New-Object System.Collections.Generic.List[string]
Remove-Item $log, "$log.out", "$Out.sends.csv", "$Out.summary.txt" -ErrorAction SilentlyContinue
$p = $null
try {
  Write-Host ''
  Write-Host '################################################################################'
  Write-Host '# AVISO: PRUEBA CON ENTRADA REAL. NO TOQUES EL RATON NI EL TECLADO mientras corre.'
  Write-Host '# El script movera el raton 1 pixel (y lo devolvera) y pulsara F15 en instantes programados.'
  Write-Host '################################################################################'
  for ($i = 10; $i -ge 1; $i--) { Write-Host "Empieza en $i s..."; Start-Sleep -Seconds 1 }

  New-Item -ItemType Directory -Force $dir | Out-Null
  Write-Utf8NoBom $settings $SettingsJson
  if (-not $NoDiagEnv) { $env:MIEL_DIAG_SLEEP_SECS = $SleepSecs }
  if ($Cdp) { $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9222' }
  $p = Start-Process $Exe -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out"
  $sends.Add("$(Now-Ms),0,inicio,miel arrancado,pid")
  if ($Cdp) { Start-Sleep -Seconds 4; Cdp-Open }
  $step = 0; $seenSlept = 0; $seenWoke = 0
  foreach ($s in ($Plan -split ',')) {
    $step++
    if ($p.HasExited) { $summary.Add("paso $step ($s): el proceso termino"); break }
    if ($BlindWaitSecs -gt 0) {
      Start-Sleep -Seconds $BlindWaitSecs
      $dom0 = Cdp-State
      $tSend = Now-Ms
      $n = if ($s -eq 'm') { [RealInput]::MouseWiggle() } else { [RealInput]::KeyF15() }
      $sends.Add("$tSend,$step,$s,enviado,eventos_aceptados=$n;DOM_antes=$dom0")
      $seen = $false; $tw = $null
      for ($k = 0; $k -lt 28; $k++) {
        $st = Cdp-State; $sends.Add("$(Now-Ms),$step,$s,DOM,$st")
        if (-not $seen -and $st -match 'surprised') { $seen = $true; $tw = Now-Ms }
        Start-Sleep -Milliseconds 250
      }
      if ($seen) { $summary.Add("paso ${step} ($s) sin registro: tras $BlindWaitSecs s sin entrada, la cara cambia a surprised $($tw - $tSend) ms despues de la entrada (DOM antes: $dom0)") }
      else { $summary.Add("paso ${step} ($s) sin registro: NO aparece surprised en 7 s (DOM antes: $dom0) (FALLO)") }
      continue
    }
    $sleptBefore = $seenSlept   # contadores del paso anterior: si duerme antes de la primera comprobacion, tambien cuenta
    $wokeBefore = $seenWoke
    # esperar a que duerma
    $t0 = Now-Ms
    while ((Count-Of 'se duerme') -le (Count-Of 'despierta')) {
      if (((Now-Ms) - $t0) -gt ($SleepWaitMax * 1000)) { break }
      Start-Sleep -Milliseconds 250
    }
    $slept = (Count-Of 'se duerme') -gt (Count-Of 'despierta')
    if (-not $slept) { $sends.Add("$(Now-Ms),$step,$s,NO_DUERME,espera $SleepWaitMax s"); $summary.Add("paso ${step} ($s): NO se durmio en $SleepWaitMax s"); continue }
    $tSleep = Now-Ms; $seenSlept = Count-Of 'se duerme'
    if ($s -like 'hold:*') {
      $n = [int]($s -split ':')[1]
      Start-Sleep -Seconds $n
      $woke = (Count-Of 'despierta') -gt $wokeBefore
      $sends.Add("$(Now-Ms),$step,hold,$(if ($woke) { 'DESPERTO_SOLA' } else { 'sigue_dormida' }),$n s")
      $summary.Add("paso ${step} (hold:$n): durmio y $(if ($woke) { 'DESPERTO SOLA (FALLO)' } else { 'siguio dormida' }) tras $n s sin entrada")
      continue
    }
    if ($Cdp) { $sends.Add("$(Now-Ms),$step,$s,DOM_antes,$(Cdp-State)") }
    Start-Sleep -Milliseconds 3000   # asentamiento: varias lecturas durmiendo antes de la entrada
    $tSend = Now-Ms
    $n = if ($s -eq 'm') { [RealInput]::MouseWiggle() } elseif ($s -eq 'k') { [RealInput]::KeyF15() } else { throw "paso desconocido $s" }
    $sends.Add("$tSend,$step,$s,enviado,eventos_aceptados=$n")
    if ($Cdp) { for ($k = 0; $k -lt 14; $k++) { $sends.Add("$(Now-Ms),$step,$s,DOM,$(Cdp-State)"); Start-Sleep -Milliseconds 250 } }
    $tw = $null
    while (((Now-Ms) - $tSend) -lt ($WakeWaitMax * 1000)) {
      if ((Count-Of 'despierta') -gt $wokeBefore) { $tw = Now-Ms; break }
      Start-Sleep -Milliseconds 100
    }
    $seenWoke = Count-Of 'despierta'
    if ($tw) { $sends.Add("$tw,$step,$s,DESPIERTA,latencia_ms=$($tw - $tSend)"); $summary.Add("paso ${step} ($s): duerme; entrada enviada; despierta en $($tw - $tSend) ms") }
    else { $sends.Add("$(Now-Ms),$step,$s,NO_DESPIERTA,espera $WakeWaitMax s"); $summary.Add("paso ${step} ($s): NO DESPERTO en $WakeWaitMax s (FALLO)") }
  }
  Start-Sleep -Seconds 2
}
finally {
  if ($p) {
    foreach ($id in @(Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $p.Id } | ForEach-Object ProcessId)) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
  }
  Get-Process miel -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item Env:\MIEL_DIAG_SLEEP_SECS, Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
  Write-Utf8NoBom "$Out.sends.csv" (($sends -join "`n") + "`n")
  Write-Utf8NoBom "$Out.summary.txt" (($summary -join "`n") + "`n")
  foreach ($f in @($log, "$log.out", "$Out.sends.csv", "$Out.summary.txt")) { if (Test-Path $f) { Clear-PersonalPaths -Path $f } }
  if ($existed) { New-Item -ItemType Directory -Force $dir | Out-Null; Copy-Item $bak $settings -Force; Remove-Item $bak -Force } else {
    Remove-Item $settings -Force -ErrorAction SilentlyContinue
    if (-not $dirExisted) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
  }
}
$summary
if ($existed) { "settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $hash0)" } else { "settings ausentes como al inicio: $(-not (Test-Path $settings))" }
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
