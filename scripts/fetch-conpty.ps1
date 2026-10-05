# Fetch the ConPTY that tsumugi ships on Windows, and put it beside tsumugi.exe.
# Copied from filer's scripts/fetch-conpty.ps1 (same version, same hash); keep
# the two in step.
#
# The terminal pane runs its shell through ConPTY. The ConPTY built into
# Windows is an old one, and two things it does break programs that read the
# console the way tcell does (lazygit, gh-dash):
#
#   - it hands every key to the program as a press *and a release*, and tcell
#     turns the release into a second escape sequence -- which is why Esc did
#     nothing in lazygit until v0.48.6;
#   - it answers a program's startup queries itself, each reply character again
#     followed by a release, and lazygit opened its copy menu at startup, a key
#     nobody pressed.
#
# A newer ConPTY does neither: it passes the queries through to the pane and the
# replies back unchanged. `scripts/keyprobe.ps1 -Query` shows which one is in
# use -- the primary DA reply reads `\e[?6c` (the pane's own answer) with this one,
# and `\e[?61;6;7;22;...c` with the one built into Windows.
#
# `alacritty_terminal` loads `conpty.dll` from beside the executable when there
# is one, and `conpty.dll` starts the `OpenConsole.exe` beside itself, so the
# two files next to tsumugi.exe are all it takes. Alacritty ships them the same
# way. Without them tsumugi still runs, on the older ConPTY.
#
#   pwsh -File scripts\fetch-conpty.ps1                     # into target\release, this machine's arch
#   pwsh -File scripts\fetch-conpty.ps1 -Dest target\debug
#   pwsh -File scripts\fetch-conpty.ps1 -Arch arm64 -Dest <folder>
#
# The version is pinned, and the package's SHA-256 is checked before anything
# is unpacked, so every build of a given tsumugi ships the same bytes. This is
# the version that was tried on the real machine; change both lines together,
# and only after trying the new one there.

param(
    [ValidateSet('x64', 'arm64')] [string]$Arch,
    [string]$Dest = 'target\release'
)

$ErrorActionPreference = 'Stop'

$version = '1.24.260710001'
$sha256 = '175640566A3B59C4B132070EE96C2C77E5AB7EDD2E92732A5EB3610BBF63D90E'
$url = "https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/$version/microsoft.windows.console.conpty.$version.nupkg"

if (-not $Arch) {
    # The architecture tsumugi.exe is built for, which is rustc's host unless a
    # --target says otherwise -- and then -Arch should be given.
    $hostLine = (rustc -vV | Select-String '^host:').ToString()
    $Arch = if ($hostLine -match 'aarch64') { 'arm64' } else { 'x64' }
}

$work = Join-Path ([IO.Path]::GetTempPath()) "tsumugi-conpty-$version"
$pkg = Join-Path $work 'conpty.zip'
New-Item -ItemType Directory -Force -Path $work | Out-Null

# Reuse a download that is already here and still matches.
if (-not (Test-Path $pkg) -or (Get-FileHash $pkg -Algorithm SHA256).Hash -ne $sha256) {
    Invoke-WebRequest -Uri $url -OutFile $pkg
}
$got = (Get-FileHash $pkg -Algorithm SHA256).Hash
if ($got -ne $sha256) {
    throw "The ConPTY package did not match its pinned hash: expected $sha256, got $got"
}

# A .nupkg is a zip; saved under a .zip name because Expand-Archive refuses any
# other extension.
$unpacked = Join-Path $work 'unpacked'
Remove-Item -Recurse -Force $unpacked -ErrorAction SilentlyContinue
Expand-Archive -Path $pkg -DestinationPath $unpacked -Force

New-Item -ItemType Directory -Force -Path $Dest | Out-Null
Copy-Item (Join-Path $unpacked "runtimes/win-$Arch/native/conpty.dll") $Dest -Force
Copy-Item (Join-Path $unpacked "build/native/runtimes/$Arch/OpenConsole.exe") $Dest -Force

# The MIT License asks for its notice to travel with the files.
$notice = Get-Content -Raw (Join-Path $PSScriptRoot '..\packaging\windows\ConPTY-LICENSE.txt')
$notice.Replace('{VERSION}', $version) | Set-Content -NoNewline (Join-Path $Dest 'ConPTY-LICENSE.txt')

"ConPTY $version ($Arch) -> $Dest"
