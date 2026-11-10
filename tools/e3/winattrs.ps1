<#
E3 (SOLO lectura): atributos de las ventanas de nivel superior de un proceso miel.exe (y su arbol): titulo, visible,
WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_CAPTION, WS_THICKFRAME. No modifica ni
envia mensajes a ninguna ventana. Salida: JSON.
#>
param([Parameter(Mandatory)][int]$RootPid)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class WinInfo {
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] static extern IntPtr GetWindowLongPtr(IntPtr h, int i);
  public static List<object[]> ForPids(HashSet<uint> pids) {
    var res = new List<object[]>();
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (!pids.Contains(pid)) return true;
      var sb = new StringBuilder(256); GetWindowText(h, sb, 256);
      long style = GetWindowLongPtr(h, -16).ToInt64(); long ex = GetWindowLongPtr(h, -20).ToInt64();
      var cn = new StringBuilder(256); GetClassName(h, cn, 256);
      res.Add(new object[] { sb.ToString(), IsWindowVisible(h), style, ex, cn.ToString() });
      return true;
    }, IntPtr.Zero);
    return res;
  }
}
'@
$all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
$set = [System.Collections.Generic.HashSet[uint32]]::new(); [void]$set.Add([uint32]$RootPid)
do { $n = $set.Count; foreach ($p in $all) { if ($set.Contains([uint32]$p.ParentProcessId)) { [void]$set.Add([uint32]$p.ProcessId) } } } while ($set.Count -ne $n)
$out = foreach ($w in [WinInfo]::ForPids($set)) {
  $style = [int64]$w[2]; $ex = [int64]$w[3]
  [pscustomobject]@{
    title = $w[0]; visible = $w[1]; class = $w[4]
    topmost = (($ex -band 0x8) -ne 0)        # WS_EX_TOPMOST
    transparent = (($ex -band 0x20) -ne 0)   # WS_EX_TRANSPARENT (click-through)
    noactivate = (($ex -band 0x8000000) -ne 0) # WS_EX_NOACTIVATE
    toolwindow = (($ex -band 0x80) -ne 0)    # WS_EX_TOOLWINDOW (sin boton en la barra de tareas)
    caption = (($style -band 0xC00000) -eq 0xC00000)  # WS_CAPTION
    thickframe = (($style -band 0x40000) -ne 0)       # WS_THICKFRAME
  }
}
$out | Where-Object { $_.title -ne '' -and $_.title -match 'Miel' } | ConvertTo-Json -Compress
