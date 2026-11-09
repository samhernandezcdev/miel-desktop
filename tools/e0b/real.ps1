param([string]$Work,[string]$Exe,[string]$Out,[int]$Seconds=900)
$ErrorActionPreference='Stop'
$settings = Join-Path $env:APPDATA 'lat.bkode.miel\settings.json'
$bak = Join-Path $env:TEMP 'miel-settings-e0b-real.bak'
$h0=(Get-FileHash $settings -Algorithm SHA256).Hash
if (Get-Process miel -ErrorAction SilentlyContinue) { throw 'Hay un miel.exe en ejecucion' }
Copy-Item $settings $bak -Force
$srv=$null
try {
  $s = Get-Content $bak -Raw | ConvertFrom-Json
  $s.messages_enabled=$false; $s.follow_enabled=$false; $s.reduced_motion=$false; $s.paused=$false; $s.pet_size=96
  ($s|ConvertTo-Json) | Set-Content $settings -Encoding UTF8
  $srv = Start-Process python -ArgumentList @('-I',"$Work\server.py","$Work\dist","$Work\css","$Work\state.json","$Work\srv") -WindowStyle Hidden -PassThru
  Start-Sleep 2
  $env:MIEL_DIAG_SCRIPT='follow_off@1'
  & 'C:\dev\miel-desktop\tools\measure.ps1' -Exe $Exe -Seconds $Seconds -IntervalSec 5 -WarmupSec 20 -Out $Out
} finally {
  if ($srv) { Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue }
  Get-Process miel -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } | Stop-Process -Force -ErrorAction SilentlyContinue
  Copy-Item $bak $settings -Force
  Remove-Item Env:\MIEL_DIAG_SCRIPT -ErrorAction SilentlyContinue
}
"settings identicos: $((Get-FileHash $settings -Algorithm SHA256).Hash -eq $h0)"
Remove-Item $bak -Force
"miel restantes: " + @(Get-Process miel -ErrorAction SilentlyContinue).Count
