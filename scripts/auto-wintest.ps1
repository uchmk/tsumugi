# Start tsumugi's Windows test session by itself when there is new work for it.
#
# tsumugi's copy of filer's scripts/auto-wintest.ps1 (the two lanes, the RAM
# disk, the screen saver and the model work the same way; filer's header tells
# the history behind each). The session's role is .claude/windows-role.md. This
# looks once, and starts one unattended run if:
#
#   - origin/main has changed TESTING.md, TESTING-CHECKS.md or
#     .claude/windows-role.md since the last run it started -- a merged run
#     changes the last two, so merging one pull request starts the next;
#   - no pull request from this lane (test/win-* or test/arm-*) is open;
#   - the screen is not locked (SendInput does nothing on a locked desktop);
#   - the worktree is clean (a dirty one is a run that was cut off).
#
# Register it with Task Scheduler every hour at :50 (filer's runs at :20, after
# its merge at :59), as you, "only when the user is logged on" (the run drives
# a real window). Run the worktree's copy,
# which is moved to origin/main at every firing; the first time, run this copy
# once by hand to make the worktree (C:\dev\tsumugi-wintest):
#
#   pwsh -File C:\dev\tsumugi\scripts\auto-wintest.ps1      # makes the worktree
#   $a = New-ScheduledTaskAction -Execute pwsh -Argument '-NoProfile -WindowStyle Hidden -File C:\dev\tsumugi-wintest\scripts\auto-wintest.ps1'
#   $t = New-ScheduledTaskTrigger -Once -At ((Get-Date).Date.AddHours((Get-Date).Hour).AddMinutes(50)) -RepetitionInterval (New-TimeSpan -Hours 1)
#   $s = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -ExecutionTimeLimit (New-TimeSpan -Hours 4)
#   Register-ScheduledTask -TaskName tsumugi-auto-wintest -Action $a -Trigger $t -Settings $s
#
#   Unregister-ScheduledTask -TaskName tsumugi-auto-wintest   # to stop it
#
# filer's task runs on the same machine and drives the same screen. A firing
# that finds filer's run going (its lock, Local\filer-auto-wintest) skips to the
# next hour; filer's script does not look for this one's yet, so a filer run
# can still start in the middle of a tsumugi run -- the start times half an
# hour apart make that rare, and pausing filer's task during tsumugi's first
# runs avoids it.
#
# By hand:
#
#   pwsh -File scripts\auto-wintest.ps1          # look once, run if there is work
#   pwsh -File scripts\auto-wintest.ps1 -Force   # run even if nothing changed
#   ... -Lane arm                                 # the ARM64 machine's lane (C:\dev\tsumugi-armtest)
#   ... -LogDir R:\Temp -TargetOnDisk -KeepScreenSaver -Model <id>
#
# Scratch: R:\Temp when there is an R: drive, else %TEMP%\tsumugi-scratch; each
# run gets run-<time> under it, TEMP and TMP point there, the newest three are
# kept. Build output: the worktree's `target` is a junction to
# R:\cargo-target\<worktree> when R: has 8 GB free. The screen saver is held
# off for the run and put back in `finally` (or by the next firing). The log
# is %LOCALAPPDATA%\tsumugi-wintest\auto-wintest.log; the state file stays
# there whatever -LogDir says. The run gets --model $Model (Opus by default:
# a wrong [x] is the one mistake nothing downstream catches). Needs `claude`
# and an authenticated `gh` on PATH.

param(
    [ValidateSet('win', 'arm')] [string]$Lane = 'win',
    [string]$Work,
    [string]$LogDir,
    [string]$Scratch,
    [switch]$Force,
    [switch]$KeepScreenSaver,
    [string]$TargetDir,
    [switch]$TargetOnDisk,
    [string]$Model = 'claude-opus-5-5'
)

$ErrorActionPreference = 'Stop'

$repo = 'uchmk/tsumugi'
$watched = @('TESTING.md', 'TESTING-CHECKS.md', '.claude/windows-role.md')
$Tools = 'Bash,PowerShell,Read,Edit,Write,Glob,Grep,TodoWrite'
$Denied = @(
    'Bash(git push origin main:*)', 'PowerShell(git push origin main:*)',
    'Bash(git push -f:*)', 'PowerShell(git push -f:*)',
    'Bash(git push --force:*)', 'PowerShell(git push --force:*)',
    'Bash(gh pr merge:*)', 'PowerShell(gh pr merge:*)',
    'Bash(cargo fmt:*)', 'PowerShell(cargo fmt:*)'
) -join ','

# The `win` lane keeps the names it had before lanes existed, so a machine
# already running it carries on from its state file.
$suffix = if ($Lane -eq 'win') { '' } else { "-$Lane" }
if (-not $Work) { $Work = if ($Lane -eq 'win') { 'C:\dev\tsumugi-wintest' } else { "C:\dev\tsumugi-$($Lane)test" } }
if (-not $Scratch) {
    $Scratch = if (Test-Path 'R:\') { 'R:\Temp' } else { Join-Path ([IO.Path]::GetTempPath()) 'tsumugi-scratch' }
}
$queue = if ($Lane -eq 'win') { 'the queue in "Where the work is"' } else { 'the queue in "Where the work is", read with "The ARM64 lane",' }

$state = Join-Path $env:LOCALAPPDATA 'tsumugi-wintest'
New-Item -ItemType Directory -Force -Path $state | Out-Null
if (-not $LogDir) { $LogDir = $state }
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
New-Item -ItemType Directory -Force -Path $Scratch | Out-Null
$log = Join-Path $LogDir "auto-wintest$suffix.log"
$last = Join-Path $state "last-trigger$suffix"

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

function Say([string]$line) {
    $stamped = '[{0:yyyy-MM-dd HH:mm:ss}] {1}' -f (Get-Date), $line
    $stamped
    Add-Content -Path $log -Value $stamped
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

# Task Scheduler's IgnoreNew already keeps its own firings apart; this also
# covers one started by hand while a scheduled one is running.
$mutex = [Threading.Mutex]::new($false, "Local\tsumugi-auto-wintest$suffix")
if (-not $mutex.WaitOne(0)) { Say 'A run is already going. Nothing to do.'; exit 0 }
# filer's lane drives the same screen: while one of its runs holds its own
# lock, this one waits for the next firing rather than fight it for the keys.
$filerLock = [Threading.Mutex]::new($false, "Local\filer-auto-wintest$suffix")
try { $filerFree = $filerLock.WaitOne(0) } catch [Threading.AbandonedMutexException] { $filerFree = $true }
if ($filerFree) { $filerLock.ReleaseMutex() } else { $mutex.ReleaseMutex(); Say "filer's run is going on this machine. Trying again next time."; exit 0 }

try {
    Restore-LeftOverSaver

    if (Get-Process LogonUI -ErrorAction SilentlyContinue) {
        Say 'The screen is locked. Trying again next time.'
        exit 0
    }

    if (-not (Test-Path $Work)) {
        # The worktree hangs off the checkout this script is in.
        $main = Split-Path -Parent $PSScriptRoot
        git -C $main fetch -q origin main
        git -C $main worktree add -q --detach $Work origin/main
        Say "Made the worktree $Work."
    }

    git -C $Work fetch -q origin main
    if ($LASTEXITCODE -ne 0) { Say 'git fetch failed. Trying again next time.'; exit 0 }
    # Kept on origin/main at every firing, not only when a run starts, so the
    # copy of this script inside it is the newest by the next firing (see the
    # top: the task runs that copy). A worktree with changes in it is left to
    # the check further down.
    if (-not (git -C $Work status --porcelain)) { git -C $Work checkout -q --detach origin/main }

    $trigger = (git -C $Work log -1 --format=%H origin/main -- $watched).Trim()
    $seen = if (Test-Path $last) { (Get-Content -Raw $last).Trim() } else { '' }
    if (-not $Force -and $trigger -eq $seen) { exit 0 }   # quiet: this is most runs

    $open = gh pr list --repo $repo --state open --json headRefName --jq '.[].headRefName' |
        Where-Object { $_ -like "test/$Lane-*" }
    if ($LASTEXITCODE -ne 0) { Say 'gh pr list failed (is gh logged in?). Trying again next time.'; exit 0 }
    if ($open) {
        Say "Waiting: $($open -join ', ') is still open."
        exit 0
    }

    if (git -C $Work status --porcelain) {
        Say "$Work has uncommitted changes, left by a run that was cut off. Look at them, then clean it (git -C $Work stash -u, or git restore/clean) and run again."
        exit 1
    }

    git -C $Work checkout -q --detach origin/main
    $head = (git -C $Work rev-parse --short HEAD).Trim()
    Say "[$Lane] Starting a run on $head (trigger $($trigger.Substring(0, 7)))."
    # Which copy of this script is running, and how old it is (filer's ARM64
    # laptop once ran a stale one for days).
    $self = (git -C $PSScriptRoot log -1 --format='%h %s' -- auto-wintest.ps1 2>$null) -join ''
    Say "Script: $PSCommandPath ($self)"
    Say "Model: $Model"
    $inWork = [IO.Path]::GetFullPath($PSScriptRoot).StartsWith([IO.Path]::GetFullPath($Work), [StringComparison]::OrdinalIgnoreCase)
    if (-not $inWork) {
        Say "This script is not the worktree's copy, so it does not follow origin/main. Point the task at $Work\scripts\auto-wintest.ps1 (see the top of the script)."
    }

    # This run's own scratch folder, and the oldest ones beyond three gone.
    Get-ChildItem -Directory -Path $Scratch -Filter 'run-*' -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -Skip 2 |
        ForEach-Object { Remove-Item -Recurse -Force -LiteralPath $_.FullName -ErrorAction SilentlyContinue }
    $Scratch = Join-Path $Scratch ('run-{0:yyyyMMdd-HHmmss}' -f (Get-Date))
    New-Item -ItemType Directory -Force -Path $Scratch | Out-Null

    $prompt = "無人実行です。人は見ていません。.claude/windows-role.md を読み、その「Unattended runs」の節に従って、$queue の先頭から、節を 3 つまで進めてください。レーンは $Lane で、ブランチは test/$Lane-<節> です。チェックアウトは $Work です（役割定義に出てくる C:\dev\tsumugi は、すべてここに読み替えてください）。作業用の一時ディレクトリは $Scratch で、TEMP / TMP も既にそこを指しています（役割定義に出てくる R:\Temp は、すべてここに読み替えてください）。"
    $env:TEMP = $Scratch
    $env:TMP = $Scratch
    $env:CARGO_INCREMENTAL = '0'
    Say "Build output: $(Set-BuildTarget)"

    if (-not $KeepScreenSaver) {
        Suspend-ScreenSaver
        $prompt += " スクリーンセーバーはこのスクリプトが実行の間だけ止めています（起動していれば 5 秒以内に止めます）。"
    }
    # Not a reason to stop: PostMessage and --keys still reach the window. The
    # run is told, so it does not record SendInput rows as having done nothing.
    Start-Sleep -Seconds 1
    $desk = Get-InputDesktop
    Say "Input desktop at the start: $desk"
    if ($desk -ne 'Default') {
        $prompt += " 起動時の入力デスクトップは `"$desk`" で、Default ではありません。SendInput のキーとマウスは届かないので、そういう行は測らずに理由を書いて残し、--keys と PostMessage で進められる行だけを進めてください。"
    }
    # claude writes UTF-8, and PowerShell decodes a native command's output
    # with the console's code page (CP932 on a Japanese Windows): decoded as
    # UTF-8 here, the log keeps Japanese as it was.
    $utf8 = [Text.UTF8Encoding]::new($false)
    [Console]::OutputEncoding = $utf8
    $OutputEncoding = $utf8
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
    }
    Add-Content -Path $log -Value "===== exit=$code`n$out"

    $tail = ($out.TrimEnd() -split "`r?`n")[-1].Trim()
    # Failures in a row: a lane that fails every firing makes no pull request
    # and says nothing anywhere else.
    $failFile = Join-Path $state "failures$suffix"
    if ($code -eq 0) {
        # Only a finished run uses the trigger up. A failed one is tried
        # again on the next firing, from the same commit.
        Set-Content -NoNewline -Path $last -Value $trigger
        Remove-Item -LiteralPath $failFile -ErrorAction SilentlyContinue
        Say "Done: $tail"
    } elseif ($out -match 'limit') {
        Say 'Hit a usage limit. Trying again next time.'
    } else {
        $fails = 1 + $(if (Test-Path $failFile) { [int](Get-Content -Raw $failFile) } else { 0 })
        Set-Content -NoNewline -Path $failFile -Value $fails
        Say "The run failed (exit $code). See the log. Last line: $tail"
        if ($fails -ge 3) {
            Say "!!!!! [$Lane] $fails runs in a row have failed. Nothing reaches GitHub until this is fixed. Last line: $tail !!!!!"
        }
        exit 1
    }
} finally {
    $mutex.ReleaseMutex()
}
