<#
E5 (correccion): observador PASIVO de la inactividad. Solo llama a GetLastInputInfo cada 500 ms y anota las MARCAS DE TIEMPO en que
cambia dwTime (cuando hubo alguna entrada, sin saber de que tipo ni cual). No genera entrada, no lee teclas ni ventanas.
Sirve para saber si alguien o algo toca el equipo durante una prueba "sin entrada".
Uso: .\tools\e5\idle_observer.ps1 -Seconds 420 -Out <ruta.csv>
#>
param([int]$Seconds = 420, [Parameter(Mandatory)][string]$Out)
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class Idle {
  [StructLayout(LayoutKind.Sequential)] struct LII { public uint cb, t; }
  [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref LII p);
  [DllImport("kernel32.dll")] static extern uint GetTickCount();
  public static uint[] Read() { var i = new LII { cb = 8 }; bool ok = GetLastInputInfo(ref i); return new[] { ok ? 1u : 0u, GetTickCount(), i.t }; }
}
'@
$rows = New-Object System.Collections.Generic.List[string]
$rows.Add('t_ms,tick,dwTime,idle_ms')
$last = 0; $t0 = [DateTimeOffset]::UtcNow
while (([DateTimeOffset]::UtcNow - $t0).TotalSeconds -lt $Seconds) {
  $r = [Idle]::Read()
  if ($r[2] -ne $last) {
    $rows.Add(('{0},{1},{2},{3}' -f [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds(), $r[1], $r[2], ([uint32]($r[1] - $r[2]))))
    $last = $r[2]
  }
  Start-Sleep -Milliseconds 500
}
[IO.File]::WriteAllText($Out, (($rows -join "`n") + "`n"))
"cambios de dwTime registrados: $($rows.Count - 1)"
