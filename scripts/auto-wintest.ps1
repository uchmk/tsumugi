# Start tsumugi's Windows test session by itself when there is work for it.
#
# tsumugi's copy of filer's scripts/auto-wintest.ps1 (the two lanes, the RAM
# disk, the screen saver and the model work the same way; filer's header tells
# the history behind each). The session's role is .claude/windows-role.md.
#
# Kept small, because a run's tokens are the cost: everything that needs no
# judgement is done here, before claude starts, and claude is given one chunk.
#
#   1. The chunk. scripts/wintest-queue.ps1 picks it from origin/main's
#      TESTING-CHECKS.md and TESTING-KEYS.md and the oldest open `retest`
#      issue (x64 only, since v0.94.0): up to 15 rows of one section (or 20
#      keys, or the rows of one retest issue). Rows a run was
#      given and left `[ ]` go to %LOCALAPPDATA%\tsumugi-wintest\attempted*.txt
#      so the next run moves on; no chunk left, no run (0 tokens).
#   2. No pull request of this lane (test/win-* or test/arm-*) is open: the
#      merge Routine merges it at :40, and the next firing takes the next chunk.
#   3. The build, the tests and the ConPTY: done here, their output to
#      build*.log. A failed build starts no run.
#   4. The desktop. filer's lane drives the same screen; both take
#      Local\wintest-desktop for the time they drive it, and wait up to
#      -DesktopWaitMin minutes for the other.
#   5. The isolation: TSUMUGI_ADDRESS (a pipe of the run's own),
#      TSUMUGI_STATE_HOME and TSUMUGI_CONFIG_HOME (with TSUMUGI_STATE and
#      TSUMUGI_SETTINGS for an older build), TSUMUGI_PTY_LOG and TSUMUGI_KEYLOG are set for the
#      whole run, so even a bare `tsumugi ls` reaches the run's server, never
#      the owner's; scripts/wintest-kit.ps1 has the tools the run uses.
#   6. The older build: for a chunk with a row that names WINTEST_OLD_EXE,
#      the newest release below the version built (gh release download, kept
#      in %LOCALAPPDATA%\tsumugi-wintest\old), copied into the run's kit.
#
# Register it with Task Scheduler every hour at :50 (filer's runs at :20), as
# you, "only when the user is logged on" (the run drives a real window). The
# task runs scripts/lane-boot.ps1 (ito's, the same as kura's), which fetches
# main and runs main's copy of this script from the state folder
# (boot<suffix>), whatever state the worktree is in, so a change to this
# script reaches the very next firing. A worktree a cut-off run left dirty is
# saved to a branch of its own, rescue/<lane>-<time>, and pushed, and goes
# back to origin/main (Save-DirtyWorktree, since v0.94.0). The first time,
# make the worktree (C:\dev\tsumugi-wintest):
#
#   git -C C:\dev\tsumugi fetch origin
#   git -C C:\dev\tsumugi worktree add --detach C:\dev\tsumugi-wintest origin/main
#   $w = 'C:\dev\tsumugi-wintest'
#   $a = New-ScheduledTaskAction -Execute (Get-Command pwsh).Source -Argument "-NoProfile -WindowStyle Hidden -File $w\scripts\lane-boot.ps1"
#   $t = New-ScheduledTaskTrigger -Once -At ((Get-Date).Date.AddHours((Get-Date).Hour).AddMinutes(50)) -RepetitionInterval (New-TimeSpan -Hours 1)
#   $s = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -ExecutionTimeLimit (New-TimeSpan -Hours 4) -StartWhenAvailable
#   Register-ScheduledTask -TaskName tsumugi-auto-wintest -Action $a -Trigger $t -Settings $s
#
#   Unregister-ScheduledTask -TaskName tsumugi-auto-wintest   # to stop it
#
# A task registered before v0.94.0 ran `-Command "git ... ; & $w\scripts\auto-wintest.ps1"`;
# to move it over, run the $w and $a lines above, then
#
#   Set-ScheduledTask -TaskName tsumugi-auto-wintest -Action $a
#
# pwsh is given by its full path: on 2026-10-09 a task registered with a bare
# `pwsh` ended every firing with 0x80070002 (2147942402, file not found)
# before the script ran, so nothing reached the log.
# If that path is under C:\Program Files\WindowsApps (the Store's PowerShell),
# it names the version and stops working at the next update: give the task
# $env:LOCALAPPDATA\Microsoft\WindowsApps\pwsh.exe (the Store's own alias,
# which follows updates) or install the MSI build (C:\Program Files\PowerShell\7).
#
# The ARM64 laptop: C:\dev\tsumugi-armtest, `-File $w\scripts\lane-boot.ps1 -Lane arm`,
# and the task name tsumugi-auto-wintest-arm. It takes no re-tests: those are
# x64's.
#
# Status on GitHub. Every firing writes the lane's issue, "Lane status: win"
# or "Lane status: arm" (label lane-status, opened by the first firing), again:
# when it fired, which copy of this script, what the firing came to, whether
# the worktree is dirty, the failed runs in a row and this firing's log lines
# (scripts/lane-status.ps1, the same file as kura's). A run also writes it
# when it starts, and a rescue adds a comment (which notifies). A cloud
# session reads it with
#   gh api 'repos/uchmk/tsumugi/issues?labels=lane-status&state=all'
# A failure to write it is logged and changes nothing else.
#
# By hand:
#
#   pwsh -File scripts\lane-boot.ps1             # main's copy: look once, run if there is work
#   pwsh -File scripts\auto-wintest.ps1 -DryRun   # the chunk and the prompt it would give; builds nothing
#   pwsh -File scripts\auto-wintest.ps1           # look once, run if there is work
#   pwsh -File scripts\auto-wintest.ps1 -Force    # forget the attempted rows first
#   ... -Lane arm                                  # the ARM64 machine's lane (C:\dev\tsumugi-armtest)
#   ... -LogDir R:\Temp -TargetOnDisk -KeepScreenSaver -Model <id> -Rows 15 -KeyCount 20
#
# Scratch: R:\Temp when there is an R: drive, else %TEMP%\tsumugi-scratch; each
# run gets run-<time> under it, TEMP and TMP point there, the newest three are
# kept. Build output: the worktree's `target` is a junction to
# R:\cargo-target\<worktree> when R: has 8 GB free. The screen saver is held
# off for the run and put back in `finally` (or by the next firing). The log
# is %LOCALAPPDATA%\tsumugi-wintest\auto-wintest.log; the state files stay
# there whatever -LogDir says. The run gets --model $Model (Sonnet by default,
# as filer's lanes since v0.80.15; -Model claude-opus-5-5 goes back). Needs
# cargo, `claude` and an authenticated `gh` on PATH.

param(
    [ValidateSet('win', 'arm')] [string]$Lane = 'win',
    [string]$Work,
    [string]$LogDir,
    [string]$Scratch,
    [switch]$Force,
    [switch]$DryRun,
    [switch]$KeepScreenSaver,
    [string]$TargetDir,
    [switch]$TargetOnDisk,
    [string]$Model = 'claude-sonnet-5-5',
    [int]$Rows = 15,
    [int]$KeyCount = 20,
    [int]$DesktopWaitMin = 20,
    # Given by lane-boot.ps1: the origin/main commit this copy was taken from,
    # and the checkout lane-boot.ps1 is in (the worktree is made from it).
    [string]$Booted,
    [string]$From
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'wintest-queue.ps1')
. (Join-Path $PSScriptRoot 'lane-marks.ps1')
. (Join-Path $PSScriptRoot 'lane-status.ps1')

$repo = 'uchmk/tsumugi'
$Tools = 'Bash,PowerShell,Read,Edit,Write,Glob,Grep,TodoWrite'
$Denied = @(
    'Bash(git push origin main:*)', 'PowerShell(git push origin main:*)',
    'Bash(git push -f:*)', 'PowerShell(git push -f:*)',
    'Bash(git push --force:*)', 'PowerShell(git push --force:*)',
    'Bash(gh pr merge:*)', 'PowerShell(gh pr merge:*)',
    'Bash(cargo fmt:*)', 'PowerShell(cargo fmt:*)',
    # Built and tested before the run starts (see the top).
    'Bash(cargo build:*)', 'PowerShell(cargo build:*)',
    'Bash(cargo test:*)', 'PowerShell(cargo test:*)',
    'Bash(cargo clean:*)', 'PowerShell(cargo clean:*)'
) -join ','

# The `win` lane keeps the names it had before lanes existed, so a machine
# already running it carries on from its state file.
$suffix = if ($Lane -eq 'win') { '' } else { "-$Lane" }
if (-not $Work) { $Work = if ($Lane -eq 'win') { 'C:\dev\tsumugi-wintest' } else { "C:\dev\tsumugi-$($Lane)test" } }
if (-not $Scratch) {
    $Scratch = if (Test-Path 'R:\') { 'R:\Temp' } else { Join-Path ([IO.Path]::GetTempPath()) 'tsumugi-scratch' }
}
$state = Join-Path $env:LOCALAPPDATA 'tsumugi-wintest'
New-Item -ItemType Directory -Force -Path $state | Out-Null
if (-not $LogDir) { $LogDir = $state }
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
New-Item -ItemType Directory -Force -Path $Scratch | Out-Null
$log = Join-Path $LogDir "auto-wintest$suffix.log"
$attemptedFile = Join-Path $state "attempted$suffix.txt"
$failFile = Join-Path $state "failures$suffix.json"
$dirtyFile = Join-Path $state "dirty$suffix.txt"
$buildLog = Join-Path $LogDir "build$suffix.log"

# Points $Work\target at the RAM disk (see the top). Returns where the build
# output goes, for the log.
function Set-BuildTarget {
    $link = Join-Path $Work 'target'
    if ($TargetOnDisk) { return $link }
    $dir = if ($TargetDir) { $TargetDir } elseif (Test-Path 'R:\') { Join-Path 'R:\cargo-target' (Split-Path -Leaf $Work) } else { $null }
    if (-not $dir) { return $link }
    $drive = Get-PSDrive -Name ($dir.Substring(0, 1)) -ErrorAction SilentlyContinue
    $item = Get-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue
    $isLink = $item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)
    if (-not $isLink -and $drive -and $drive.Free -lt 8GB) {
        # Out-Host: Say also writes to the output, which here is the return value.
        Say ("Only {0:N1} GB free on {1}: building in {2} this time." -f ($drive.Free / 1GB), $drive.Root, $link) | Out-Host
        return $link
    }
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    if ($isLink -and ("$($item.Target)" -eq $dir)) { return $dir }
    if ($isLink) {
        [IO.Directory]::Delete($link)                    # the junction only, not what it points at
    } elseif ($item) {
        $gb = (Get-ChildItem -LiteralPath $link -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum / 1GB
        Remove-Item -Recurse -Force -LiteralPath $link   # build output: the next build makes it again
        Say ("Removed {0} ({1:N1} GB) to build on {2} instead." -f $link, $gb, $dir) | Out-Host
    }
    New-Item -ItemType Junction -Path $link -Target $dir | Out-Null
    return $dir
}

# This firing's lines, for the lane's status issue (Publish-Status).
$said = [Collections.Generic.List[string]]::new()
function Say([string]$line) {
    $stamped = '[{0:yyyy-MM-dd HH:mm:ss}] {1}' -f (Get-Date), $line
    $stamped
    Add-Content -Path $log -Value $stamped
    [void]$script:said.Add($stamped)
}

# This firing, written into the lane's status issue on GitHub (lane-status.ps1)
# at the start of a run and at the end of every firing, so a cloud session can
# tell why a lane is not making pull requests. $outcome says what the firing
# came to; when it is not set, the last line it logged does. Never throws.
$outcome = $null
function Publish-Status {
    if ($DryRun) { return }
    try {
        $what = if ($script:outcome) { $script:outcome } elseif ($said.Count) { $said[-1] -replace '^\[[^\]]*\] ', '' } else { 'Nothing to do.' }
        $now = Get-Date
        $history = @(Add-LaneHistory (Join-Path $state "status-history$suffix.txt") ('{0:yyyy-MM-dd HH:mm} {1}' -f $now, $what))
        $toml = Get-Content -Raw -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'Cargo.toml') -ErrorAction SilentlyContinue
        $version = if ($toml -match '(?m)^version = "([^"]+)"') { "v$($Matches[1])" } else { '?' }
        $copy = if ($Booted) { "$version ($($Booted.Substring(0, [Math]::Min(7, $Booted.Length))), main's copy by lane-boot.ps1)" }
                else { "$version ($((git -C $PSScriptRoot rev-parse --short HEAD 2>$null) -join ''), $PSCommandPath)" }
        $dirty = if (Test-Path $Work) { @(git -C $Work status --porcelain 2>$null) } else { @() }
        $fails = if (Test-Path $failFile) { [int](Get-Content -Raw -LiteralPath $failFile | ConvertFrom-Json).Total } else { 0 }
        $task = if ($Lane -eq 'win') { 'tsumugi-auto-wintest' } else { "tsumugi-auto-wintest-$Lane" }
        $body = Format-LaneStatus -Lane $Lane -When $now -Script $copy -Outcome $what -Task $task `
            -Said @($said) -Dirty $dirty -Failures $fails -History @($history | Select-Object -Skip 1)
        $err = Publish-LaneStatus -Repo $repo -Lane $Lane -Body $body -StateDir $state
        if ($err) { Say "Could not write the lane's status issue: $err" }
        if ($script:rescued -and -not $err) {
            $err = Add-LaneComment -Repo $repo -Lane $Lane -Body ($script:rescued -join "`n") -StateDir $state
            if ($err) { Say "Could not comment on the lane's status issue: $err" }
            $script:rescued = $null
        }
    } catch {
        Say "Could not write the lane's status issue: $_"
    }
}

# The screen saver, held off while a run drives the window (see the top).
$saverFile = Join-Path $state "screensaver$suffix.json"
Add-Type -Namespace TsumugiWintest -Name Power -MemberDefinition @'
[DllImport("kernel32.dll")]
public static extern uint SetThreadExecutionState(uint flags);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SystemParametersInfo(uint action, uint param, ref bool value, uint winIni);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SystemParametersInfo(uint action, uint param, System.IntPtr value, uint winIni);
[DllImport("user32.dll", SetLastError = true)]
public static extern System.IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool CloseDesktop(System.IntPtr desktop);
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
public static extern bool GetUserObjectInformation(System.IntPtr obj, int index, System.Text.StringBuilder info, int length, out int needed);
'@
$ES_CONTINUOUS = [uint32]'0x80000000'
$ES_SYSTEM_REQUIRED = [uint32]1
$ES_DISPLAY_REQUIRED = [uint32]2
$SPI_GETSCREENSAVEACTIVE = [uint32]16
$SPI_SETSCREENSAVEACTIVE = [uint32]17

function Get-SaverActive {
    $on = $false
    [void][TsumugiWintest.Power]::SystemParametersInfo($SPI_GETSCREENSAVEACTIVE, 0, [ref]$on, 0)
    $on
}

# In memory only: winIni 0, so the profile keeps whatever the owner chose.
function Set-SaverActive([bool]$on) {
    [void][TsumugiWintest.Power]::SystemParametersInfo($SPI_SETSCREENSAVEACTIVE, [uint32][int]$on, [IntPtr]::Zero, 0)
}

# A run killed before its `finally` left the saver off; put it back first.
function Restore-LeftOverSaver {
    if (-not (Test-Path $saverFile)) { return }
    $was = (Get-Content -Raw $saverFile | ConvertFrom-Json).active
    Set-SaverActive $was
    Remove-Item $saverFile
    Say "Put the screen saver back (active = $was), left off by a run that was cut off."
}

# The desktop that receives input: `Default` when keys and clicks reach the
# windows on it, `Screen-saver` or `Winlogon` when they go nowhere. Asked
# rather than inferred from LogonUI, which a screen saver does not start (#88).
function Get-InputDesktop {
    $h = [TsumugiWintest.Power]::OpenInputDesktop(0, $false, 0x0001)   # DESKTOP_READOBJECTS
    if ($h -eq [IntPtr]::Zero) { return '(none: OpenInputDesktop failed)' }
    try {
        $name = [Text.StringBuilder]::new(256)
        $needed = 0
        [void][TsumugiWintest.Power]::GetUserObjectInformation($h, 2, $name, 512, [ref]$needed)   # UOI_NAME
        $name.ToString()
    } finally {
        [void][TsumugiWintest.Power]::CloseDesktop($h)
    }
}

function Stop-ScreenSavers {
    Get-Process | Where-Object { $_.Path -like '*.scr' } | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        $_.Path
    }
}

function Suspend-ScreenSaver {
    $was = Get-SaverActive
    @{ active = $was } | ConvertTo-Json | Set-Content -Path $saverFile
    Set-SaverActive $false
    [void][TsumugiWintest.Power]::SetThreadExecutionState($ES_CONTINUOUS -bor $ES_SYSTEM_REQUIRED -bor $ES_DISPLAY_REQUIRED)
    foreach ($p in Stop-ScreenSavers) { Say "Stopped a running screen saver: $p" }
    # The watcher. A thread job shares nothing with this script but what it is
    # given, so the function goes in as text.
    $body = ${function:Stop-ScreenSavers}.ToString()
    $script:saverWatch = Start-ThreadJob -ArgumentList $body -ScriptBlock {
        param($body)
        $stop = [scriptblock]::Create($body)
        while ($true) {
            foreach ($p in & $stop) { "{0:HH:mm:ss} stopped {1}" -f (Get-Date), $p }
            Start-Sleep -Seconds 5
        }
    }
    Say "Holding the screen saver off for the run (it was active = $was)."
}

function Resume-ScreenSaver {
    if ($script:saverWatch) {
        $stopped = Receive-Job $script:saverWatch -ErrorAction SilentlyContinue
        Remove-Job $script:saverWatch -Force
        $script:saverWatch = $null
        if ($stopped) { Say "During the run the watcher stopped a screen saver $(@($stopped).Count) time(s): $(@($stopped)[-1])" }
    }
    [void][TsumugiWintest.Power]::SetThreadExecutionState($ES_CONTINUOUS)
    if (Test-Path $saverFile) {
        $was = (Get-Content -Raw $saverFile | ConvertFrom-Json).active
        Set-SaverActive $was
        Remove-Item $saverFile
        Say "Gave the screen saver back (active = $was)."
    }
}

# The machine an exe is built for, from its PE header: 0x8664 is x64, 0xAA64
# ARM64. An ARM64 run that tested an x64 build proved nothing about ARM64.
function Get-PeMachine([string]$Path) {
    $fs = [IO.File]::OpenRead($Path)
    try {
        $br = [IO.BinaryReader]::new($fs)
        $fs.Position = 0x3C
        $fs.Position = $br.ReadInt32() + 4
        '0x{0:X4}' -f $br.ReadUInt16()
    } finally { $fs.Dispose() }
}

# One step of the build, its output to $buildLog only: claude never reads it.
function Invoke-BuildStep([string]$Name, [scriptblock]$Do) {
    Add-Content -Path $buildLog -Value "===== $Name"
    $t = Get-Date
    $global:LASTEXITCODE = 0
    & $Do 2>&1 | ForEach-Object { "$_" } | Add-Content -Path $buildLog
    $code = $LASTEXITCODE
    $took = ((Get-Date) - $t).TotalSeconds
    if ($code -eq 0) { Say ("{0}: OK ({1:N0} s)" -f $Name, $took) | Out-Host; return $true }
    Say ("{0}: FAILED (exit {1}, {2:N0} s). See {3}." -f $Name, $code, $took, $buildLog) | Out-Host
    $false
}

# The build of an older release, for the rows about a server older than the
# window (1.7, 1.11, 1.12): the newest release below the version built here,
# downloaded once into $state\old and copied into the kit, so Stop-RunTsumugi
# stops whatever it starts. Its path, or $null (said in the log) when there is
# none; a run without it leaves those rows, it does not fail.
function Get-OldBuild([string]$Arch) {
    try {
        $toml = Get-Content -Raw -LiteralPath (Join-Path $Work 'Cargo.toml')
        if ($toml -notmatch '(?m)^version = "(\d+\.\d+\.\d+)"') { throw 'no version in Cargo.toml' }
        $built = [version]$Matches[1]
        $tags = @(gh api "repos/$repo/releases?per_page=30" --jq '.[] | select(.draft | not) | select(.prerelease | not) | .tag_name')
        if ($LASTEXITCODE -ne 0) { throw 'gh api failed' }
        $tag = $tags | Where-Object { $_ -match '^v(\d+\.\d+\.\d+)$' -and [version]$Matches[1] -lt $built } |
            Sort-Object { [version]$_.Substring(1) } -Descending | Select-Object -First 1
        if (-not $tag) { throw "no release below $built" }
        $name = "tsumugi-$tag-windows-$Arch"
        $cache = Join-Path $state 'old'
        if (-not (Test-Path (Join-Path $cache "$name\tsumugi.exe"))) {
            New-Item -ItemType Directory -Force -Path $cache | Out-Null
            Remove-Item -Recurse -Force -LiteralPath (Join-Path $cache $name) -ErrorAction SilentlyContinue
            gh release download $tag --repo $repo --pattern "$name.zip" --dir $cache --clobber
            if ($LASTEXITCODE -ne 0) { throw "gh release download $tag failed" }
            Expand-Archive -LiteralPath (Join-Path $cache "$name.zip") -DestinationPath $cache -Force
            Remove-Item -LiteralPath (Join-Path $cache "$name.zip")
        }
        $dest = Join-Path $kit 'old'
        Copy-Item -Recurse -LiteralPath (Join-Path $cache $name) -Destination $dest
        Say "Older build: $tag ($Arch), in $dest."
        Join-Path $dest 'tsumugi.exe'
    } catch {
        Say "No older build for this run: $_"
        $null
    }
}

# Every tsumugi this run started: the window from the worktree's build (by
# its path through the junction and the path it points at), and
# the server, which runs from a copy under the run's kit folder. The owner's
# runs from %LOCALAPPDATA%\uchmk\tsumugi\server\ and matches neither.
function Stop-RunTsumugi([string[]]$Exe, [string]$Under) {
    $mine = @(Get-Process tsumugi* -ErrorAction SilentlyContinue | Where-Object {
            $_.Path -and ($_.Path -in $Exe -or $_.Path.StartsWith($Under, [StringComparison]::OrdinalIgnoreCase)) })
    foreach ($p in $mine) {
        Get-CimInstance Win32_Process -Filter "ParentProcessId = $($p.Id)" -ErrorAction SilentlyContinue |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    }
    if ($mine) { Say "Stopped $($mine.Count) tsumugi process(es) the run left running." }
}

# Failures in a row: of the lane (a lane that fails every firing makes no pull
# request and says nothing anywhere else) and of the chunk (one that fails
# twice is set aside, so one bad row does not stop the lane).
function Add-Failure([string]$Branch, [string]$Why) {
    $f = if (Test-Path $failFile) { Get-Content -Raw $failFile | ConvertFrom-Json } else { [pscustomobject]@{ Branch = ''; Count = 0; Total = 0 } }
    $count = if ($f.Branch -eq $Branch) { $f.Count + 1 } else { 1 }
    $total = $f.Total + 1
    @{ Branch = $Branch; Count = $count; Total = $total } | ConvertTo-Json | Set-Content -Path $failFile
    Say "The run failed ($Why). See the log."
    $script:outcome = "The run failed ($Why), $total in a row."
    if ($Branch -ne 'build' -and $count -ge 2 -and $chunk) {
        Add-Attempted $attemptedFile $chunk
        Say "$Branch failed $count times: set aside; the next firing takes the next chunk."
    }
    if ($total -ge 3) {
        Say "!!!!! [$Lane] $total runs in a row have failed. Nothing reaches GitHub until this is fixed. Last: $Why !!!!!"
    }
}

# A dirty worktree that Save-DirtyWorktree could not save stops every run
# until a person cleans it. The log says so once a day, and $dirtyFile lists
# what is changed.
function Note-Dirty([string[]]$changes) {
    $now = Get-Date
    $since = $now
    $told = [datetime]::MinValue
    if (Test-Path $dirtyFile) {
        foreach ($line in Get-Content $dirtyFile) {
            if ($line -match '^since: (.+)$') { $since = [datetime]::Parse($Matches[1]) }
            if ($line -match '^told: (.+)$') { $told = [datetime]::Parse($Matches[1]) }
        }
    }
    $hours = [int]($now - $since).TotalHours
    $what = "$Work has uncommitted changes (first seen $('{0:yyyy-MM-dd HH:mm}' -f $since), $hours h ago). No run starts until a person looks at them and cleans the worktree."
    if (($now - $told).TotalHours -ge 24) {
        Say "!!!!! [$Lane] $what !!!!!"
        $told = $now
    }
    $body = @(
        "since: $('{0:o}' -f $since)"
        "told: $('{0:o}' -f $told)"
        "checked: $('{0:o}' -f $now)"
        $what
        ''
        'git status --porcelain:'
    ) + @($changes | Select-Object -First 40)
    Set-Content -Path $dirtyFile -Value $body
}

# A dirty worktree is a run that was cut off. Until v0.94.0 it stopped every
# run until a person cleaned it, and kura's lanes sat still for days that way
# (2026-09-30, 2026-10-10). Now its changes are committed to a branch of their
# own, rescue/<lane>-<time>, which is pushed (kept locally when the push
# fails), and the worktree goes back to origin/main: nothing is lost and the
# lane carries on. Only in a lane's own worktree -- on a detached HEAD, a
# test/<lane>-* branch or an earlier rescue/<lane>-* branch, which is all a
# lane's worktree is ever on -- and never in the middle of a rebase. Returns
# the branch, or $null when the worktree is left as it is (Note-Dirty then
# stops the run). Say goes to the host: its output would be the return value.
$rescued = $null
function Save-DirtyWorktree([string[]]$changes) {
    $head = (git -C $Work rev-parse --abbrev-ref HEAD 2>$null | Out-String).Trim()
    if ($head -ne 'HEAD' -and $head -notlike "test/$Lane-*" -and $head -notlike "rescue/$Lane-*") {
        Say "$Work is on $head, not a lane's branch: its changes are left alone." | Out-Host
        return $null
    }
    foreach ($dir in 'rebase-merge', 'rebase-apply') {
        if (Test-Path (git -C $Work rev-parse --path-format=absolute --git-path $dir | Out-String).Trim()) {
            Say "$Work is in the middle of a rebase: its changes are left alone." | Out-Host
            return $null
        }
    }
    $on = if ($head -eq 'HEAD') { 'a detached HEAD' } else { $head }
    $branch = 'rescue/{0}-{1:yyyyMMdd-HHmm}' -f $Lane, (Get-Date)
    $was = (git -C $Work rev-parse --short HEAD | Out-String).Trim()
    try {
        git -C $Work checkout -q -B $branch
        if ($LASTEXITCODE -ne 0) { throw "git checkout -B $branch failed" }
        git -C $Work add -A
        if ($LASTEXITCODE -ne 0) { throw 'git add -A failed' }
        $what = "auto-wintest.ps1 found $($changes.Count) uncommitted path(s) in the $Lane lane's worktree on $on ($was), left by a run that was cut off, and saved them here before putting the worktree back on origin/main."
        git -C $Work commit -q --no-verify -m "Rescue the $Lane lane's uncommitted changes" -m $what
        if ($LASTEXITCODE -ne 0) { throw 'git commit failed' }
        git -C $Work push -q origin "refs/heads/${branch}:refs/heads/$branch" 2>$null
        $pushed = $LASTEXITCODE -eq 0
        git -C $Work checkout -q --detach origin/main
        if ($LASTEXITCODE -ne 0) { throw 'git checkout origin/main failed' }
    } catch {
        Say "Could not save the dirty worktree to ${branch}: $_" | Out-Host
        return $null
    }
    $where = if ($pushed) { "pushed to origin as $branch" } else { "committed to the local branch $branch (the push failed: it is only on this machine)" }
    Say "!!!!! [$Lane] $Work had $($changes.Count) uncommitted path(s) on $on ($was): $where, and the worktree is back on origin/main. !!!!!" | Out-Host
    $script:rescued = @(
        "**Rescued a dirty worktree.** It had $($changes.Count) uncommitted path(s) on $on (``$was``), left by a run that was cut off. They are $where, and the worktree is back on origin/main, so the lane carries on."
        ''
        'Look at the branch: merge what is worth keeping (a run''s ticks need their evidence, as in any lane pull request) and delete it.'
        ''
        '```text'
    ) + @($changes | Select-Object -First 40) + @('```')
    if (Test-Path $dirtyFile) { Remove-Item -LiteralPath $dirtyFile }
    $branch
}

# A run takes an hour and main moves under it: a row reworded, another lane's
# ticks, a re-test list changed. The run's marks then sit beside lines main
# changed, and its pull request opens in conflict. When only the checklists
# conflict, merge main into the run's branch with main's checklists and the
# run's marks made again on them (Set-LaneMarks), and push, so the pull
# request is mergeable from the start. A row main reworded or took out keeps
# no mark, and the pull request says so. Anything else that conflicts is left
# as it is for the Merge lanes workflow and a person. Never throws.
function Sync-LaneBranch([string]$Branch) {
    $lists = @('TESTING-CHECKS.md', 'TESTING-KEYS.md')
    Push-Location $Work
    try {
        if ((git rev-parse --abbrev-ref HEAD).Trim() -ne $Branch) { Say "Sync: $Work is not on $Branch; left as it is."; return }
        git fetch -q origin main $Branch
        if ($LASTEXITCODE -ne 0) { Say 'Sync: git fetch failed; left as it is.'; return }
        # Only what the run pushed: an unpushed commit stays the run's.
        if ((git rev-parse HEAD).Trim() -ne (git rev-parse "origin/$Branch").Trim()) { Say "Sync: $Branch is not what was pushed; left as it is."; return }
        git merge-base --is-ancestor origin/main HEAD
        if ($LASTEXITCODE -eq 0) { return }
        $base = (git merge-base origin/main HEAD).Trim()
        git merge -q --no-ff --no-commit origin/main *> $null
        $conflicted = @(git diff --name-only --diff-filter=U)
        if (-not $conflicted) { git merge --abort; return }   # GitHub merges it as it is
        $others = @($conflicted | Where-Object { $_ -notin $lists })
        if ($others) { git merge --abort; Say "Sync: $Branch conflicts with main in $($others -join ', '); left for a person."; return }
        $dropped = @()
        foreach ($path in $conflicted) {
            $marks = Get-LaneMarks -Base ((git show "${base}:$path") -join "`n") -Branch ((git show "HEAD:$path") -join "`n")
            git checkout -q --theirs -- $path
            $file = Join-Path $Work $path
            $r = Set-LaneMarks -Main ([IO.File]::ReadAllText($file)) -Marks $marks
            [IO.File]::WriteAllText($file, $r.Text, $utf8)
            git add -- $path
            $dropped += $r.Dropped
        }
        git commit -q --no-edit
        if ($LASTEXITCODE -ne 0) { throw 'git commit failed' }
        git push -q origin "HEAD:refs/heads/$Branch"
        if ($LASTEXITCODE -ne 0) { Say "Sync: git push failed; $Branch stays in conflict for the Merge lanes workflow."; return }
        Say "Sync: merged main into $Branch with the run's marks made again ($($conflicted -join ', '))."
        if ($dropped) {
            $body = "Merged main into this branch: main changed these rows while the run was going, so their marks were not kept (what the run checked is not what the rows say now): $($dropped -join ', ')."
            gh pr comment $Branch --repo $repo --body $body *> $null
            Say "Sync: marks dropped for $($dropped -join ', ')."
        }
    } catch {
        Say "Sync: $_; left as it is."
    } finally {
        git rev-parse -q --verify MERGE_HEAD *> $null
        if ($LASTEXITCODE -eq 0) { git merge --abort }
        Pop-Location
    }
}

# claude writes UTF-8 and so does git, and PowerShell decodes a native
# command's output with the console's code page (CP932 on a Japanese
# Windows): decoded as UTF-8 here, Japanese stays as it was.
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$OutputEncoding = $utf8

# Task Scheduler's IgnoreNew already keeps its own firings apart; this also
# covers one started by hand while a scheduled one is running.
$mutex = [Threading.Mutex]::new($false, "Local\tsumugi-auto-wintest$suffix")
if (-not $mutex.WaitOne(0)) { Say 'A run is already going. Nothing to do.'; exit 0 }
$desktop = $null
$chunk = $null

try {
    Restore-LeftOverSaver

    if (Get-Process LogonUI -ErrorAction SilentlyContinue) {
        Say 'The screen is locked. Trying again next time.'
        exit 0
    }

    if (-not (Test-Path $Work)) {
        # The worktree hangs off the checkout lane-boot.ps1 or this script is in.
        $main = if ($From) { $From } else { Split-Path -Parent $PSScriptRoot }
        git -C $main fetch -q origin main
        git -C $main worktree add -q --detach $Work origin/main
        Say "Made the worktree $Work."
    }

    git -C $Work fetch -q origin main
    if ($LASTEXITCODE -ne 0) { Say 'git fetch failed. Trying again next time.'; exit 0 }
    $changes = @(git -C $Work status --porcelain)
    # Files changed in their line endings only (a checkout of a commit made
    # before .gitattributes, on a machine with core.autocrlf=true): nothing is
    # lost by putting them back, and left alone they stop the lane for good.
    if ($changes.Count -and -not ($changes | Where-Object { $_ -notmatch '^ M ' })) {
        $paths = @($changes | ForEach-Object { $_.Substring(3) })
        git -C $Work diff --ignore-cr-at-eol --quiet HEAD -- $paths
        if ($LASTEXITCODE -eq 0) {
            git -C $Work checkout -q -- $paths
            Say "Put back $($paths -join ', '): only their line endings had changed."
            $changes = @(git -C $Work status --porcelain)
        }
    }
    # A dirty worktree is saved to a rescue/ branch (Save-DirtyWorktree); when
    # it cannot be, it stops the run further down.
    if ($changes.Count -and -not $DryRun -and (Save-DirtyWorktree $changes)) { $changes = @() }
    if ($changes.Count -eq 0) {
        git -C $Work checkout -q --detach origin/main
        if (Test-Path $dirtyFile) { Say "$Work is clean again."; Remove-Item -LiteralPath $dirtyFile }
    } elseif (-not $DryRun) { Note-Dirty $changes }

    # 1. The chunk, from origin/main's checklists and, for x64, the open
    # retest issues, the oldest first.
    if ($Force) { Remove-Item -LiteralPath $attemptedFile -ErrorAction SilentlyContinue }
    $show = { param($f) (git -C $Work show "origin/main:$f") -join "`n" }
    $retests = @()
    if ($Lane -eq 'win') {
        $json = gh issue list --repo $repo --label retest --state open --limit 50 --json number,body,createdAt
        if ($LASTEXITCODE -ne 0) { Say 'gh issue list failed (is gh logged in?). Trying again next time.'; exit 0 }
        $retests = @($json | ConvertFrom-Json | Sort-Object { [datetime]$_.createdAt } |
            ForEach-Object { [pscustomobject]@{ Number = $_.number; Body = $_.body } })
    }
    $chunk = Select-Chunk -Lane $Lane -Checks (& $show 'TESTING-CHECKS.md') -Keys (& $show 'TESTING-KEYS.md') `
        -Retests $retests -Attempted (Read-Attempted $attemptedFile) -Rows $Rows -KeyCount $KeyCount
    if (-not $chunk) {
        if ($DryRun) { 'Nothing left for this lane: every open row has been tried.' }
        $outcome = 'Nothing left for this lane: every open row has been tried.'
        if ($changes.Count) { $outcome += ' The worktree is dirty and could not be saved, so new work would not start either.' }
        if ($rescued) { $outcome = "Saved a dirty worktree to a rescue/ branch (see the comment). $outcome" }
        exit 0   # quiet: nothing left until a row changes or a retest issue names it
    }
    $chunkText = Format-Chunk $chunk
    $branch = $chunk.Branch

    # 2. This lane's pull request, if one is open, waits for the merge.
    $open = gh pr list --repo $repo --state open --json headRefName --jq '.[].headRefName' |
        Where-Object { $_ -like "test/$Lane-*" }
    if ($LASTEXITCODE -ne 0) { Say 'gh pr list failed (is gh logged in?). Trying again next time.'; exit 0 }
    if ($open -and -not $DryRun) {
        Say "Waiting: $($open -join ', ') is still open."
        exit 0
    }

    # Since v0.76.4 the merge-lanes workflow merges this lane's pull request
    # as soon as its checks are green, and then does the share (the PATCH and
    # CHANGELOG.md, the report's findings as issues). Starting between
    # the two would take the same rows again, so wait while the lane's latest
    # merged pull request is not named in main's CHANGELOG.md yet.
    $merged = gh pr list --repo $repo --state merged --limit 30 --json number,headRefName,mergedAt
    if ($LASTEXITCODE -ne 0) { Say 'gh pr list failed (is gh logged in?). Trying again next time.'; exit 0 }
    $lastMerged = $merged | ConvertFrom-Json | Where-Object { $_.headRefName -like "test/$Lane-*" } |
        Sort-Object { [datetime]$_.mergedAt } -Descending | Select-Object -First 1
    if ($lastMerged -and [datetime]$lastMerged.mergedAt -gt (Get-Date).AddDays(-3) -and -not $DryRun) {
        $changelog = (git -C $Work show origin/main:CHANGELOG.md) -join "`n"
        if ($changelog -notmatch "#$($lastMerged.number)(?!\d)") {
            Say "Waiting: #$($lastMerged.number) is merged, but the merger's share for it is not on main yet."
            exit 0
        }
    }

    if ($changes.Count -and -not $DryRun) {
        $names = @($changes | Select-Object -First 5 | ForEach-Object { $_.Trim() }) -join ', '
        Say "$Work has uncommitted changes ($names; all in $dirtyFile), left by a run that was cut off, and they could not be saved to a rescue/ branch (the lines above say why). Look at them, then clean it (git -C $Work stash -u, or git restore/clean) and run again."
        $outcome = "Stopped: $Work has uncommitted changes that could not be saved to a rescue/ branch. A person has to look at them and clean the worktree."
        exit 1
    }
    if (-not $DryRun) { git -C $Work checkout -q --detach origin/main }

    $head = (git -C $Work rev-parse --short HEAD).Trim()
    $exe = Join-Path $Work 'target\release\tsumugi.exe'
    $kit = $null

    # This run's own scratch folder, and the oldest ones beyond three gone.
    $scratchRoot = $Scratch
    if (-not $DryRun) {
        Get-ChildItem -Directory -Path $Scratch -Filter 'run-*' -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending | Select-Object -Skip 2 |
            ForEach-Object { Remove-Item -Recurse -Force -LiteralPath $_.FullName -ErrorAction SilentlyContinue }
    }
    $started = Get-Date
    $stamp = '{0:yyyyMMdd-HHmmss}' -f $started
    # The report's own name. A chunk's branch comes back (a section whose
    # first open row is the same), and a second run's report under the same
    # name was a changed file, which the Merge lanes workflow refuses.
    $report = 'qa-reports/{0:yyyy-MM-dd}-{1}-{0:HHmm}.md' -f $started, ($branch -replace '^test/', '')
    $Scratch = Join-Path $Scratch "run-$stamp"
    $kit = Join-Path $Scratch 'kit'

    $what = if ($chunk.Kind -eq 'keys') { "TESTING-KEYS.md の次の $(@($chunk.Rows).Count) キー" } else { "TESTING-CHECKS.md の次の $(@($chunk.Rows).Count) 行" }
    $prompt = @"
無人実行です。人は見ていません。.claude/windows-role.md を読み、その「Unattended runs」の節に従ってください。TESTING.md は先頭から「## Covered by tests」の手前まで（規則の部分）だけを読み、行の一覧は読まないでください。

この実行で確かめるのは、このスクリプトが選んだ ${what}だけです。ほかの行には触れません。TESTING-CHECKS.md / TESTING-KEYS.md でも、この行の印だけを変えます。

$chunkText

- レーンは $Lane です。ブランチは ``git checkout -B $branch origin/main`` で作ります。
- 報告は ``$report`` に書きます（新しいファイル。既にある報告は直しません）。
- チェックアウトは $Work です（役割定義の C:\dev\tsumugi は、すべてここに読み替えてください）。
- ビルド（--release）、``cargo test``、ConPTY の取得は、このスクリプトが済ませました。exe は $exe で、テストは緑です。cargo build / cargo test はしないでください。表の確かめは ``cargo run --release -q -p tsumugi --example make-testcheck -- --check``（make-keycheck も同じ）で、ビルド済みのものを使います。
- 道具は scripts\wintest-kit.ps1 にあります。PowerShell を呼ぶたびに、先頭で ``. .\scripts\wintest-kit.ps1`` を読み込んでください（関数の一覧はファイルの先頭）。SendInput・PrintWindow・バックアップを自分で書き直さないでください。
- 環境変数は分離済みです（TSUMUGI_ADDRESS・TSUMUGI_STATE_HOME・TSUMUGI_CONFIG_HOME・TSUMUGI_PTY_LOG・TSUMUGI_KEYLOG・WINTEST_KIT=$kit）。素の ``tsumugi ls`` もこの実行のサーバに届き、持ち主のサーバには届きません。終わる前に ``Stop-Mine`` を呼びます。
- 作業用の一時ディレクトリは $Scratch で、TEMP / TMP も既にそこを指しています（役割定義に出てくる R:\Temp は、すべてここに読み替えてください）。
- 所見（不具合・提案）は ``gh issue create --repo $repo -l finding,lane:$Lane --title "<1 行>" --body-file <ファイル>`` で issue にし（欄は .github/ISSUE_TEMPLATE/finding.yml、不具合なら ``bug`` も付ける）、PR 本文に ``Findings: #N, #M`` と書きます。gh で issue が作れなかったときだけ、報告の ``### Proposals`` に書きます（マージのときに issue になります）。

"@
    if ($Lane -eq 'win') {
        $prompt += @"
- 開いている ``vote`` の issue（``gh issue list --repo $repo -l vote --state open``）に、まだ ``vote[win]:`` のコメントが無ければ票を入れます（``gh issue comment N --repo $repo --body "vote[win]: 2 — <理由>"``。理由はこの機械でしたこと・見たことに基づける。持ち主が決めることなら ``owner``）。役割定義の「How to work」の Vote の項目のとおりです。

"@
    }
    if ($chunk.Kind -eq 'retest') {
        $n = $chunk.Issue
        $prompt += @"
- この塊は再テストの issue #$n の行です。押した結果は TESTING-CHECKS.md に加えて issue の本文にも付けます（``gh issue view $n --repo $repo --json body -q .body`` をファイルに書き、箱を ``[x]`` にして ``gh issue edit $n --repo $repo --body-file <ファイル>``）。
- PR 本文には、issue の行が全部 ``[x]`` になったら ``Closes #$n``、残りがあれば ``Refs #$n`` と書きます。落ちた行は所見の issue にし、その番号を #$n へのコメント 1 つに書きます。

"@
    }

    if ($DryRun) {
        "Lane $Lane on $head; branch $branch; open pull requests of the lane: $(if ($open) { $open -join ', ' } else { 'none' })"
        "Attempted list: $attemptedFile"
        ''
        $prompt
        exit 0
    }

    Say "[$Lane] $branch on ${head}: $($chunk.Kind), $(@($chunk.Rows).Count) rows ($((@($chunk.Rows) | ForEach-Object Id) -join ', '))."
    # Which copy of this script is running, and how old it is (filer's ARM64
    # laptop once ran a stale one for days).
    $self = if ($Booted) { (git -C $Work log -1 --format='%h %s' $Booted -- scripts/auto-wintest.ps1 2>$null) -join '' }
            else { (git -C $PSScriptRoot log -1 --format='%h %s' -- auto-wintest.ps1 2>$null) -join '' }
    Say "Script: $PSCommandPath ($self)"
    Say "Model: $Model"
    $outcome = "A run is going (started $('{0:yyyy-MM-dd HH:mm}' -f (Get-Date)): $branch on $head, $(@($chunk.Rows).Count) rows, model $Model)."
    Publish-Status
    $outcome = $null
    $inWork = [IO.Path]::GetFullPath($PSScriptRoot).StartsWith([IO.Path]::GetFullPath($Work), [StringComparison]::OrdinalIgnoreCase)
    if (-not $Booted -and -not $inWork) {
        Say "This script is neither main's copy nor the worktree's, so it does not follow origin/main. Point the task at $Work\scripts\lane-boot.ps1 (see the top of the script)."
    }

    New-Item -ItemType Directory -Force -Path $kit | Out-Null
    $env:TEMP = $Scratch
    $env:TMP = $Scratch
    $env:CARGO_INCREMENTAL = '0'
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $buildDir = Set-BuildTarget
    Say "Build output: $buildDir"

    # 3. The build, the tests and the ConPTY, before claude starts.
    Set-Content -Path $buildLog -Value "[$Lane] $branch on $head, $(Get-Date -Format s)"
    Push-Location $Work
    try {
        $built = (Invoke-BuildStep 'Build' { cargo build --release --locked -p tsumugi --bins --examples }) -and
            (Invoke-BuildStep 'Tests' { cargo test --release --locked --workspace }) -and
            (Invoke-BuildStep 'ConPTY' { pwsh -NoProfile -File scripts\fetch-conpty.ps1 -Dest target\release })
    } finally { Pop-Location }
    if (-not $built) { Add-Failure 'build' "the build or the tests failed on $head"; exit 1 }
    $machine = Get-PeMachine $exe
    Say "tsumugi.exe is $machine ($(if ($machine -eq '0xAA64') { 'ARM64' } elseif ($machine -eq '0x8664') { 'x64' } else { 'unknown' }))."
    if ($Lane -eq 'arm' -and $machine -ne '0xAA64') { Add-Failure 'build' "the ARM64 lane built $machine, not 0xAA64"; exit 1 }

    # 5. The isolation, inherited by claude and every shell it starts.
    $env:TSUMUGI_ADDRESS = "\\.\pipe\tsumugi-wintest$suffix-$stamp"
    $env:WINTEST_KIT = $kit
    $env:WINTEST_EXE = $exe
    $env:TSUMUGI_STATE_HOME = $kit
    $env:TSUMUGI_CONFIG_HOME = $kit
    # The names before v0.87.0, for an older build (WINTEST_OLD_EXE): the same files.
    $env:TSUMUGI_STATE = Join-Path $kit 'state'
    $env:TSUMUGI_SETTINGS = Join-Path $kit 'config.toml'
    $env:TSUMUGI_PTY_LOG = Join-Path $kit 'pty.log'
    $env:TSUMUGI_KEYLOG = '1'
    Remove-Item Env:TSUMUGI_SESSION -ErrorAction SilentlyContinue

    # The older build, only for a chunk with a row that asks for it.
    Remove-Item Env:WINTEST_OLD_EXE -ErrorAction SilentlyContinue
    if (@($chunk.Rows | Where-Object { $_.Text -match 'WINTEST_OLD_EXE' }).Count) {
        $old = Get-OldBuild $(if ($Lane -eq 'arm') { 'arm64' } else { 'x64' })
        if ($old) {
            $env:WINTEST_OLD_EXE = $old
            $prompt += "- 古いリリースのビルドが ``$old``（WINTEST_OLD_EXE）にあります。``Start-OldTsumugi`` でこの実行の環境のまま起動すると、そのサーバがこの実行のパイプで動きます。窓を閉じてから ``Start-Tsumugi`` で新しい窓を開きます。`n"
        } else {
            $prompt += "- 古いリリースのビルドは取れませんでした。WINTEST_OLD_EXE の要る行は、理由を書いて ``[ ]`` のまま残してください。`n"
        }
    }

    # 4. The desktop, shared with filer's lane.
    $desktop = [Threading.Mutex]::new($false, 'Local\wintest-desktop')
    $t = Get-Date
    try { $got = $desktop.WaitOne([TimeSpan]::FromMinutes($DesktopWaitMin)) } catch [Threading.AbandonedMutexException] { $got = $true }
    $waited = ((Get-Date) - $t).TotalMinutes
    if (-not $got) {
        $desktop = $null
        Say ("desktop lock: still held after {0:N0} min (filer's run?). Trying again next time." -f $waited)
        exit 0
    }
    if ($waited -ge 0.5) { Say ("desktop lock: waited {0:N0} min" -f $waited) }
    if (Get-Process LogonUI -ErrorAction SilentlyContinue) {
        Say 'The screen locked while waiting. Trying again next time.'
        exit 0
    }

    if (-not $KeepScreenSaver) {
        Suspend-ScreenSaver
        $prompt += "- スクリーンセーバーはこのスクリプトが実行の間だけ止めています（起動していれば 5 秒以内に止めます）。`n"
    }
    # Not a reason to stop: PostMessage and --keys still reach the window. The
    # run is told, so it does not record SendInput rows as having done nothing.
    Start-Sleep -Seconds 1
    $desk = Get-InputDesktop
    Say "Input desktop at the start: $desk"
    if ($desk -ne 'Default') {
        $prompt += "- 起動時の入力デスクトップは `"$desk`" で、Default ではありません。SendInput のキーとマウスは届かないので、そういう行は測らずに理由を書いて残し、文字で確かめられる行だけを進めてください。`n"
    }

    Push-Location $Work
    try {
        $out = claude -p $prompt --model $Model --permission-mode acceptEdits --allowedTools $Tools --disallowedTools $Denied 2>&1 | Out-String
        $code = $LASTEXITCODE
        # A `claude` too old for $Model fails in seconds, every firing.
        # Update once and go again.
        if ($code -ne 0 -and $out -match 'does not support this model|or newer is required') {
            Add-Content -Path $log -Value "===== exit=$code`n$out"
            $before = (claude --version 2>&1 | Out-String).Trim()
            $upd = (claude update 2>&1 | Out-String).Trim()
            $after = (claude --version 2>&1 | Out-String).Trim()
            Say "claude was too old for $Model ($before); ran claude update: $(($upd -split "`r?`n")[-1]) Now $after. Trying again."
            $out = claude -p $prompt --model $Model --permission-mode acceptEdits --allowedTools $Tools --disallowedTools $Denied 2>&1 | Out-String
            $code = $LASTEXITCODE
        }
    } finally {
        Pop-Location
        if (-not $KeepScreenSaver) { Resume-ScreenSaver }
        Stop-RunTsumugi @($exe, (Join-Path $buildDir 'release\tsumugi.exe')) "$kit\"
    }
    Add-Content -Path $log -Value "===== exit=$code`n$out"

    $tail = ($out.TrimEnd() -split "`r?`n")[-1].Trim()
    if ($code -eq 0 -and $tail -match '^WINTEST_(DONE|NOTHING)\b') {
        # The chunk is used up: what it left `[ ]` is not offered again
        # until its words change or it is named in a new retest issue.
        Add-Attempted $attemptedFile $chunk
        Remove-Item -LiteralPath $failFile -ErrorAction SilentlyContinue
        Say "Done: $tail"
        $outcome = "Done ($branch): $tail"
        if ($tail -match '^WINTEST_DONE\b') { Sync-LaneBranch $branch }
    } elseif ($code -ne 0 -and $out -match 'limit') {
        Say 'Hit a usage limit. Trying again next time.'
    } else {
        Add-Failure $branch "exit $code; last line: $tail"
        exit 1
    }
} catch {
    $outcome = "The script stopped with an error at line $($_.InvocationInfo.ScriptLineNumber): $_"
    Say $outcome
    throw
} finally {
    Publish-Status
    if ($desktop) { $desktop.ReleaseMutex() }
    $mutex.ReleaseMutex()
}
