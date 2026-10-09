# The tools a Windows test run uses, written once so a run does not write
# them again (that was most of what a run spent). Each PowerShell call of a
# run is a process of its own, so dot-source it at the top of each, and the
# kit keeps what it must remember (the windows it started, the backups) in
# files under $KitDir:
#
#   . .\scripts\wintest-kit.ps1
#
#   Start-Tsumugi [-ArgumentList ...]   the window, isolated (below); returns its process
#   Invoke-Tsumugi ls --json            the CLI against the same server: {Code, Out, Err}
#   Test-InputReady                     input reaches our window: the Default desktop, ours in front
#   Set-Foreground                      brings our window to the front
#   Send-Keys 'Ctrl+Shift+Z'            one chord through SendInput ('Alt+Shift++' is the + key)
#   Send-Text '日本語'                  characters through SendInput (KEYEVENTF_UNICODE)
#   Send-Click -X 120 -Y 40 [-Right]    a click at egui points from the window's client corner
#   Get-KeyLog / Get-FocusLog           TSUMUGI_KEYLOG's `key …` lines, its `focus x,y wxh` lines as objects
#   Get-PtyLog                          the bytes the panes were sent (TSUMUGI_PTY_LOG)
#   Save-Shot -Name before              PrintWindow of our window to <kit dir>\shots\before.png
#   Backup-UserFile / Restore-UserFile  a person's file: copy and hash first, put back and compare
#   Stop-Mine                           every window and the server this kit started; never the owner's
#
# Isolation: auto-wintest.ps1 sets TSUMUGI_ADDRESS, TSUMUGI_STATE,
# TSUMUGI_SETTINGS, TSUMUGI_PTY_LOG and TSUMUGI_KEYLOG for the whole run, and
# WINTEST_KIT (the kit's folder) and WINTEST_EXE, so even a bare `tsumugi ls`
# reaches the run's own server: the owner's server, sessions and settings are
# never touched. By hand, the kit sets them itself (a folder under TEMP).

$ErrorActionPreference = 'Stop'

if (-not ('TsumugiKit.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace TsumugiKit {
public static class Native {
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk, wScan; public uint dwFlags, time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Explicit)] public struct InputUnion { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
    // 40 bytes on a 64-bit process (x64 and ARM64): the union is 8-aligned.
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public InputUnion u; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint n, INPUT[] inputs, int size);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern short VkKeyScan(char c);
    [DllImport("user32.dll")] public static extern IntPtr SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll", SetLastError = true)] public static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr d);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool GetUserObjectInformation(IntPtr obj, int index, StringBuilder info, int length, out int needed);

    public static int InputSize() { return Marshal.SizeOf(typeof(INPUT)); }

    static uint Send(INPUT[] inputs) { return SendInput((uint)inputs.Length, inputs, InputSize()); }

    static INPUT Key(ushort vk, ushort scan, uint flags) {
        INPUT i = new INPUT();
        i.type = 1;   // INPUT_KEYBOARD
        i.u.ki.wVk = vk; i.u.ki.wScan = scan; i.u.ki.dwFlags = flags;
        return i;
    }

    static bool Extended(ushort vk) {
        // Arrows, Insert/Delete/Home/End/PageUp/PageDown, the right-hand Ctrl/Alt, Win.
        return (vk >= 0x21 && vk <= 0x28) || vk == 0x2D || vk == 0x2E || vk == 0x5B || vk == 0x5C || vk == 0xA3 || vk == 0xA5;
    }

    // The modifiers down in order, the key down and up, the modifiers up in
    // reverse: one SendInput, so nothing else can come between them.
    public static uint Chord(ushort[] mods, ushort vk) {
        INPUT[] all = new INPUT[mods.Length * 2 + 2];
        int n = 0;
        foreach (ushort m in mods) all[n++] = Key(m, 0, Extended(m) ? 1u : 0u);
        uint ext = Extended(vk) ? 1u : 0u;
        all[n++] = Key(vk, 0, ext);
        all[n++] = Key(vk, 0, ext | 2u);   // KEYEVENTF_KEYUP
        for (int k = mods.Length - 1; k >= 0; k--) all[n++] = Key(mods[k], 0, (Extended(mods[k]) ? 1u : 0u) | 2u);
        return Send(all);
    }

    public static uint Text(string s) {
        INPUT[] all = new INPUT[s.Length * 2];
        for (int k = 0; k < s.Length; k++) {
            all[2 * k] = Key(0, s[k], 4u);              // KEYEVENTF_UNICODE
            all[2 * k + 1] = Key(0, s[k], 4u | 2u);
        }
        return Send(all);
    }

    // Down and up where the cursor is (SetCursorPos first).
    public static uint Click(bool right) {
        INPUT[] all = new INPUT[2];
        all[0].type = 0; all[0].u.mi.dwFlags = right ? 0x0008u : 0x0002u;
        all[1].type = 0; all[1].u.mi.dwFlags = right ? 0x0010u : 0x0004u;
        return Send(all);
    }

    public static string InputDesktop() {
        IntPtr h = OpenInputDesktop(0, false, 0x0001);   // DESKTOP_READOBJECTS
        if (h == IntPtr.Zero) return "(none)";
        try {
            StringBuilder name = new StringBuilder(256);
            int needed;
            GetUserObjectInformation(h, 2, name, 512, out needed);   // UOI_NAME
            return name.ToString();
        } finally { CloseDesktop(h); }
    }
}
}
'@
    # Physical pixels everywhere (cursor, window rectangles, screenshots), as
    # per-monitor aware v2; fails harmlessly when already set.
    [void][TsumugiKit.Native]::SetProcessDpiAwarenessContext([IntPtr]::new(-4))
}

# Sets the isolation when the run's script has not (by hand). Idempotent.
function Use-Isolation {
    if (-not $env:WINTEST_KIT) { $env:WINTEST_KIT = Join-Path $env:TEMP 'tsumugi-kit' }
    $script:KitDir = $env:WINTEST_KIT
    New-Item -ItemType Directory -Force -Path $script:KitDir | Out-Null
    if (-not $env:TSUMUGI_ADDRESS -or $env:TSUMUGI_ADDRESS -notlike '*tsumugi-wintest*') {
        $env:TSUMUGI_ADDRESS = '\\.\pipe\tsumugi-wintest-kit'
        $env:TSUMUGI_STATE = Join-Path $script:KitDir 'state'
        $env:TSUMUGI_SETTINGS = Join-Path $script:KitDir 'settings.toml'
        $env:TSUMUGI_PTY_LOG = Join-Path $script:KitDir 'pty.log'
    }
    $env:TSUMUGI_KEYLOG = '1'
    Remove-Item Env:TSUMUGI_SESSION -ErrorAction SilentlyContinue
    if (-not $env:WINTEST_EXE) { $env:WINTEST_EXE = (Resolve-Path 'target\release\tsumugi.exe').Path }
}
Use-Isolation

# The window, with its key log going to <kit dir>\keylog-<n>.txt. Waits for
# it to show and returns the process. The newest window is the one the other
# functions act on, in this call and the later ones.
function Start-Tsumugi([string[]]$ArgumentList = @(), [int]$TimeoutSec = 20) {
    $n = @(Get-ChildItem -LiteralPath $script:KitDir -Filter 'keylog-*.txt' -ErrorAction SilentlyContinue).Count + 1
    $keyLog = Join-Path $script:KitDir "keylog-$n.txt"
    $p = if ($ArgumentList) {
        Start-Process -FilePath $env:WINTEST_EXE -ArgumentList $ArgumentList -WorkingDirectory (Get-Location) -RedirectStandardError $keyLog -PassThru
    } else {
        Start-Process -FilePath $env:WINTEST_EXE -WorkingDirectory (Get-Location) -RedirectStandardError $keyLog -PassThru
    }
    # With its start time: a process id can be reused.
    Add-Content -LiteralPath (Join-Path $script:KitDir 'windows.txt') -Value "$($p.Id)`t$($p.StartTime.Ticks)"
    @{ Pid = $p.Id; KeyLog = $keyLog } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $script:KitDir 'window.json')
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $p.Refresh()
        if ($p.HasExited) { throw "tsumugi exited ($($p.ExitCode)) before showing a window; $keyLog says: $(Get-Content -Raw -LiteralPath $keyLog -ErrorAction SilentlyContinue)" }
        if ($p.MainWindowHandle -ne [IntPtr]::Zero) { return $p }
        Start-Sleep -Milliseconds 100
    }
    throw "no window from tsumugi (pid $($p.Id)) in $TimeoutSec s"
}

# The newest window Start-Tsumugi started: {Pid, KeyLog}.
function Get-KitWindow {
    $f = Join-Path $script:KitDir 'window.json'
    if (-not (Test-Path -LiteralPath $f)) { throw 'Start-Tsumugi first' }
    Get-Content -Raw -LiteralPath $f | ConvertFrom-Json
}

# The CLI with its output caught (a release build is a GUI program, so only a
# redirected stdout is reliable). -Timeout in seconds.
function Invoke-Tsumugi {
    param([Parameter(ValueFromRemainingArguments)] [string[]]$Arguments, [int]$Timeout = 60)
    $psi = [Diagnostics.ProcessStartInfo]::new($env:WINTEST_EXE)
    foreach ($a in $Arguments) { $psi.ArgumentList.Add($a) }
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $psi.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
    $psi.WorkingDirectory = (Get-Location).Path
    $p = [Diagnostics.Process]::Start($psi)
    $out = $p.StandardOutput.ReadToEndAsync()
    $err = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit($Timeout * 1000)) { $p.Kill(); throw "tsumugi ${Arguments}: no exit in $Timeout s" }
    $p.WaitForExit()
    [pscustomobject]@{ Code = $p.ExitCode; Out = $out.Result; Err = $err.Result }
}

function Get-KitHandle {
    $w = Get-KitWindow
    $p = Get-Process -Id $w.Pid -ErrorAction SilentlyContinue
    if (-not $p) { throw "the window (pid $($w.Pid)) has gone" }
    $p.MainWindowHandle
}

# Keys and clicks reach our window: the input desktop is Default (not the
# lock screen, not a screen saver) and the window in front is ours. Nothing
# measured after this turned false counts.
function Test-InputReady {
    $desk = [TsumugiKit.Native]::InputDesktop()
    $fg = [TsumugiKit.Native]::GetForegroundWindow()
    $pid_ = [uint32]0
    [void][TsumugiKit.Native]::GetWindowThreadProcessId($fg, [ref]$pid_)
    $ours = (Test-Path -LiteralPath (Join-Path $script:KitDir 'window.json')) -and $pid_ -eq (Get-KitWindow).Pid
    if ($desk -ne 'Default' -or -not $ours) { "Input not ready: desktop '$desk', foreground pid $pid_" | Out-Host; return $false }
    $true
}

function Set-Foreground {
    $h = Get-KitHandle
    if ([TsumugiKit.Native]::IsIconic($h)) { [void][TsumugiKit.Native]::ShowWindow($h, 9) }   # SW_RESTORE
    # Windows lets a process take the foreground right after a key of its own:
    # a lone Alt tap first.
    [void][TsumugiKit.Native]::Chord([uint16[]]@(), [uint16]0x12)
    [void][TsumugiKit.Native]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 200
    Test-InputReady
}

$script:KitVk = @{
    ctrl = 0x11; control = 0x11; shift = 0x10; alt = 0x12; win = 0x5B
    enter = 0x0D; return = 0x0D; esc = 0x1B; escape = 0x1B; tab = 0x09; space = 0x20; backspace = 0x08
    delete = 0x2E; del = 0x2E; insert = 0x2D; home = 0x24; end = 0x23; pageup = 0x21; pagedown = 0x22
    left = 0x25; up = 0x26; right = 0x27; down = 0x28
    arrowleft = 0x25; arrowup = 0x26; arrowright = 0x27; arrowdown = 0x28
}

# 'Ctrl+Shift+Z', 'Alt+Shift++', 'F2', 'Ctrl+,'. A punctuation key is the key
# that types it on this keyboard; if that needs Shift (or AltGr), it is added.
function ConvertTo-Chord([string]$Chord) {
    $parts = if ($Chord.Length -gt 1 -and $Chord.EndsWith('++')) { @($Chord.Substring(0, $Chord.Length - 2) -split '\+') + '+' } else { $Chord -split '\+' }
    $parts = @($parts | Where-Object { $_ -ne '' })
    if ($Chord -eq '+') { $parts = @('+') }
    $key = $parts[-1]
    $mods = [Collections.Generic.List[uint16]]::new()
    if ($parts.Count -gt 1) {
        foreach ($m in $parts[0..($parts.Count - 2)]) {
            $vk = $script:KitVk[$m.ToLower()]
            if (-not $vk) { throw "unknown modifier '$m' in '$Chord'" }
            $mods.Add([uint16]$vk)
        }
    }
    $lower = $key.ToLower()
    if ($script:KitVk.ContainsKey($lower)) { $vk = [uint16]$script:KitVk[$lower] }
    elseif ($lower -match '^f([1-9]|1\d|2[0-4])$') { $vk = [uint16](0x6F + [int]$Matches[1]) }
    elseif ($key -match '^[A-Za-z0-9]$') { $vk = [uint16][char]$key.ToUpper() }
    elseif ($key.Length -eq 1) {
        $scan = [TsumugiKit.Native]::VkKeyScan($key[0])
        if ($scan -eq -1) { throw "no key types '$key' on this keyboard" }
        $vk = [uint16]($scan -band 0xFF)
        $shiftState = ($scan -shr 8) -band 0xFF
        foreach ($need in @(@(1, 0x10), @(2, 0x11), @(4, 0x12))) {
            if (($shiftState -band $need[0]) -and -not $mods.Contains([uint16]$need[1])) { $mods.Add([uint16]$need[1]) }
        }
    } else { throw "unknown key '$key' in '$Chord'" }
    [pscustomobject]@{ Mods = [uint16[]]$mods.ToArray(); Vk = $vk }
}

# Each chord in turn: Send-Keys 'Ctrl+Shift+T' 'Esc'. Throws when input would
# not reach the window, so a key never goes to whatever else is in front.
function Send-Keys([Parameter(ValueFromRemainingArguments)] [string[]]$Chords) {
    foreach ($c in $Chords) {
        if (-not (Test-InputReady)) { throw "input does not reach tsumugi; '$c' not sent" }
        $k = ConvertTo-Chord $c
        $sent = [TsumugiKit.Native]::Chord($k.Mods, $k.Vk)
        if ($sent -eq 0) { throw "SendInput sent nothing for '$c' (blocked by UIPI or the desktop)" }
        Start-Sleep -Milliseconds 150
    }
}

function Send-Text([string]$Text) {
    if (-not (Test-InputReady)) { throw 'input does not reach tsumugi; text not sent' }
    if ([TsumugiKit.Native]::Text($Text) -eq 0) { throw 'SendInput sent nothing' }
    Start-Sleep -Milliseconds 150
}

# X, Y in egui points from the client area's top-left, as the focus log
# gives them; -Scale when tsumugi's zoom is not 100%.
function Send-Click([double]$X, [double]$Y, [switch]$Right, [double]$Scale = 1.0) {
    if (-not (Test-InputReady)) { throw 'input does not reach tsumugi; click not sent' }
    $h = Get-KitHandle
    $px = [TsumugiKit.Native]::GetDpiForWindow($h) / 96.0 * $Scale
    $pt = [TsumugiKit.Native+POINT]::new()
    $pt.X = [int][Math]::Round($X * $px)
    $pt.Y = [int][Math]::Round($Y * $px)
    [void][TsumugiKit.Native]::ClientToScreen($h, [ref]$pt)
    [void][TsumugiKit.Native]::SetCursorPos($pt.X, $pt.Y)
    Start-Sleep -Milliseconds 50
    if ([TsumugiKit.Native]::Click($Right.IsPresent) -eq 0) { throw 'SendInput sent nothing for the click' }
    Start-Sleep -Milliseconds 150
}

# A file another process is still writing.
function Read-Shared([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return '' }
    $fs = [IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
    try { [IO.StreamReader]::new($fs, [Text.Encoding]::UTF8).ReadToEnd() } finally { $fs.Dispose() }
}

function Get-KeyLog { (Read-Shared (Get-KitWindow).KeyLog) -split "`r?`n" | Where-Object { $_ -like 'key *' } }

# Each `focus x,y wxh` as {X, Y, W, H}; `focus none` as {None = $true}.
function Get-FocusLog {
    foreach ($line in (Read-Shared (Get-KitWindow).KeyLog) -split "`r?`n") {
        if ($line -match '^focus (-?\d+),(-?\d+) (\d+)x(\d+)$') {
            [pscustomobject]@{ None = $false; X = [int]$Matches[1]; Y = [int]$Matches[2]; W = [int]$Matches[3]; H = [int]$Matches[4] }
        } elseif ($line -eq 'focus none') {
            [pscustomobject]@{ None = $true; X = $null; Y = $null; W = $null; H = $null }
        }
    }
}

function Get-PtyLog { Read-Shared $env:TSUMUGI_PTY_LOG }

# The window as it is drawn (PrintWindow with PW_RENDERFULLCONTENT), to
# <kit dir>\shots\<name>.png; returns the path. Crop it with -Crop x,y,w,h
# in physical pixels from the window's corner.
function Save-Shot([string]$Name, [int[]]$Crop) {
    Add-Type -AssemblyName System.Drawing
    $h = Get-KitHandle
    $r = [TsumugiKit.Native+RECT]::new()
    [void][TsumugiKit.Native]::GetWindowRect($h, [ref]$r)
    $bmp = [Drawing.Bitmap]::new($r.Right - $r.Left, $r.Bottom - $r.Top)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    try { [void][TsumugiKit.Native]::PrintWindow($h, $hdc, 2) } finally { $g.ReleaseHdc($hdc); $g.Dispose() }
    if ($Crop) {
        $part = $bmp.Clone([Drawing.Rectangle]::new($Crop[0], $Crop[1], $Crop[2], $Crop[3]), $bmp.PixelFormat)
        $bmp.Dispose()
        $bmp = $part
    }
    $dir = Join-Path $script:KitDir 'shots'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $path = Join-Path $dir "$Name.png"
    $bmp.Save($path, [Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $path
}

# A person's file (~/.claude/settings.json, a profile): its hash and a copy
# before a row touches it. Restore-UserFile puts it back and says whether the
# hash matches again.
function Get-BackupName([string]$Path) {
    $full = [IO.Path]::GetFullPath($Path).ToLowerInvariant()
    $hash = -join ([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($full))[0..7] | ForEach-Object { $_.ToString('x2') })
    Join-Path $script:KitDir "backup\$([IO.Path]::GetFileName($Path)).$hash"
}

function Backup-UserFile([string]$Path) {
    $base = Get-BackupName $Path
    if (Test-Path -LiteralPath "$base.json") { throw "$Path is backed up already ($base.json): Restore-UserFile it first" }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $base) | Out-Null
    $had = Test-Path -LiteralPath $Path
    if ($had) { Copy-Item -LiteralPath $Path -Destination "$base.copy" }
    $hash = if ($had) { (Get-FileHash -LiteralPath $Path).Hash } else { 'absent' }
    @{ Path = $Path; Copy = "$base.copy"; Hash = $hash; Had = $had } | ConvertTo-Json | Set-Content -LiteralPath "$base.json"
    "Backed up $Path ($hash) to $base.copy" | Out-Host
}

function Restore-UserFile([string]$Path) {
    $base = Get-BackupName $Path
    if (-not (Test-Path -LiteralPath "$base.json")) { throw "no backup of $Path" }
    $b = Get-Content -Raw -LiteralPath "$base.json" | ConvertFrom-Json
    if ($b.Had) { Copy-Item -LiteralPath $b.Copy -Destination $Path -Force } else { Remove-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue }
    $now = if (Test-Path -LiteralPath $Path) { (Get-FileHash -LiteralPath $Path).Hash } else { 'absent' }
    "Restored ${Path}: $now (was $($b.Hash)) $(if ($now -eq $b.Hash) { 'MATCH' } else { 'DIFFERENT' })" | Out-Host
    if ($now -eq $b.Hash) { Remove-Item -LiteralPath "$base.json" }
    $now -eq $b.Hash
}

# Every window this kit started, and the server it started: the one running
# from <kit dir>\server\ (the copy a server runs from sits beside its
# TSUMUGI_STATE), with what that server started. The owner's server runs from
# %LOCALAPPDATA%\tsumugi\server\ and is never matched.
function Stop-Mine {
    $list = Join-Path $script:KitDir 'windows.txt'
    $windows = @(if (Test-Path -LiteralPath $list) { Get-Content -LiteralPath $list })
    foreach ($line in $windows) {
        # Only the process that was started: a process id can be reused.
        $id, $ticks = $line -split "`t", 2
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($p -and "$($p.StartTime.Ticks)" -eq $ticks) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
    }
    Remove-Item -LiteralPath $list -ErrorAction SilentlyContinue
    # The server runs from a copy beside TSUMUGI_STATE.
    $root = (Join-Path (Split-Path -Parent $env:TSUMUGI_STATE) 'server') + [IO.Path]::DirectorySeparatorChar
    $servers = @(Get-Process tsumugi* -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase) })
    foreach ($s in $servers) {
        Get-CimInstance Win32_Process -Filter "ParentProcessId = $($s.Id)" | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        Stop-Process -Id $s.Id -Force -ErrorAction SilentlyContinue
    }
    "Stopped $($windows.Count) window(s) and $($servers.Count) server(s) under $root" | Out-Host
}
