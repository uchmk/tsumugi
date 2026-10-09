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
#      TESTING-CHECKS.md, TESTING-KEYS.md and the role's re-test row: up to
#      15 rows of one section (or 20 keys, or the re-tests). Rows a run was
#      given and left `[ ]` go to %LOCALAPPDATA%\tsumugi-wintest\attempted*.txt
#      so the next run moves on; no chunk left, no run (0 tokens).
#   2. No pull request of this lane (test/win-* or test/arm-*) is open: the
#      merge Routine merges it at :40, and the next firing takes the next chunk.
#   3. The build, the tests and the ConPTY: done here, their output to
#      build*.log. A failed build starts no run.
#   4. The desktop. filer's lane drives the same screen; both take
#      Local\wintest-desktop for the time they drive it, and wait up to
#      -DesktopWaitMin minutes for the other.
#   5. The isolation: TSUMUGI_ADDRESS (a pipe of the run's own), TSUMUGI_STATE,
#      TSUMUGI_SETTINGS, TSUMUGI_PTY_LOG and TSUMUGI_KEYLOG are set for the
#      whole run, so even a bare `tsumugi ls` reaches the run's server, never
#      the owner's; scripts/wintest-kit.ps1 has the tools the run uses.
#
# Register it with Task Scheduler every hour at :50 (filer's runs at :20), as
# you, "only when the user is logged on" (the run drives a real window). The
# task moves the worktree to origin/main and then runs the worktree's copy, so
# a change to this script reaches the very next firing. The first time, make
# the worktree (C:\dev\tsumugi-wintest):
#
#   git -C C:\dev\tsumugi fetch origin
#   git -C C:\dev\tsumugi worktree add --detach C:\dev\tsumugi-wintest origin/main
#   $w = 'C:\dev\tsumugi-wintest'
#   $a = New-ScheduledTaskAction -Execute pwsh -Argument "-NoProfile -WindowStyle Hidden -Command `"git -C $w fetch -q origin main; if (-not (git -C $w status --porcelain)) { git -C $w checkout -q --detach origin/main }; & $w\scripts\auto-wintest.ps1`""
#   $t = New-ScheduledTaskTrigger -Once -At ((Get-Date).Date.AddHours((Get-Date).Hour).AddMinutes(50)) -RepetitionInterval (New-TimeSpan -Hours 1)
#   $s = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -ExecutionTimeLimit (New-TimeSpan -Hours 4) -StartWhenAvailable
#   Register-ScheduledTask -TaskName tsumugi-auto-wintest -Action $a -Trigger $t -Settings $s
#
#   Unregister-ScheduledTask -TaskName tsumugi-auto-wintest   # to stop it
#
# The ARM64 laptop: C:\dev\tsumugi-armtest, `& $w\scripts\auto-wintest.ps1 -Lane arm`,
# and the task name tsumugi-auto-wintest-arm.
#
# By hand:
#
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
    [int]$DesktopWaitMin = 20
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'wintest-queue.ps1')

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

# Every tsumugi this run started: the window from the worktree's build (by
# its path through the junction and the path it points at), and
# the server, which runs from a copy under the run's kit folder. The owner's
# runs from %LOCALAPPDATA%\tsumugi\server\ and matches neither.
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
    if ($Branch -ne 'build' -and $count -ge 2 -and $chunk) {
        Add-Attempted $attemptedFile $chunk
        Say "$Branch failed $count times: set aside; the next firing takes the next chunk."
    }
    if ($total -ge 3) {
        Say "!!!!! [$Lane] $total runs in a row have failed. Nothing reaches GitHub until this is fixed. Last: $Why !!!!!"
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
        # The worktree hangs off the checkout this script is in.
        $main = Split-Path -Parent $PSScriptRoot
        git -C $main fetch -q origin main
        git -C $main worktree add -q --detach $Work origin/main
        Say "Made the worktree $Work."
    }

    git -C $Work fetch -q origin main
    if ($LASTEXITCODE -ne 0) { Say 'git fetch failed. Trying again next time.'; exit 0 }
    if (-not (git -C $Work status --porcelain)) { git -C $Work checkout -q --detach origin/main }

    # 1. The chunk, from origin/main's checklists.
    if ($Force) { Remove-Item -LiteralPath $attemptedFile -ErrorAction SilentlyContinue }
    $show = { param($f) (git -C $Work show "origin/main:$f") -join "`n" }
    $chunk = Select-Chunk -Lane $Lane -Checks (& $show 'TESTING-CHECKS.md') -Keys (& $show 'TESTING-KEYS.md') `
        -Role (& $show '.claude/windows-role.md') -Attempted (Read-Attempted $attemptedFile) -Rows $Rows -KeyCount $KeyCount
    if (-not $chunk) {
        if ($DryRun) { 'Nothing left for this lane: every open row has been tried.' }
        exit 0   # quiet: nothing left until a row changes or a re-test is named
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

    if ((git -C $Work status --porcelain) -and -not $DryRun) {
        Say "$Work has uncommitted changes, left by a run that was cut off. Look at them, then clean it (git -C $Work stash -u, or git restore/clean) and run again."
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
    $stamp = '{0:yyyyMMdd-HHmmss}' -f (Get-Date)
    $Scratch = Join-Path $Scratch "run-$stamp"
    $kit = Join-Path $Scratch 'kit'

    $what = if ($chunk.Kind -eq 'keys') { "TESTING-KEYS.md の次の $(@($chunk.Rows).Count) キー" } else { "TESTING-CHECKS.md の次の $(@($chunk.Rows).Count) 行" }
    $prompt = @"
無人実行です。人は見ていません。.claude/windows-role.md を読み、その「Unattended runs」の節に従ってください。TESTING.md は先頭から「## Covered by tests」の手前まで（規則の部分）だけを読み、行の一覧は読まないでください。

この実行で確かめるのは、このスクリプトが選んだ ${what}だけです。ほかの行には触れません。TESTING-CHECKS.md / TESTING-KEYS.md でも、この行の印だけを変えます。

$chunkText

- レーンは $Lane です。ブランチは ``git checkout -B $branch origin/main`` で作ります。
- チェックアウトは $Work です（役割定義の C:\dev\tsumugi は、すべてここに読み替えてください）。
- ビルド（--release）、``cargo test``、ConPTY の取得は、このスクリプトが済ませました。exe は $exe で、テストは緑です。cargo build / cargo test はしないでください。表の確かめは ``cargo run --release -q -p tsumugi --example make-testcheck -- --check``（make-keycheck も同じ）で、ビルド済みのものを使います。
- 道具は scripts\wintest-kit.ps1 にあります。PowerShell を呼ぶたびに、先頭で ``. .\scripts\wintest-kit.ps1`` を読み込んでください（関数の一覧はファイルの先頭）。SendInput・PrintWindow・バックアップを自分で書き直さないでください。
- 環境変数は分離済みです（TSUMUGI_ADDRESS・TSUMUGI_STATE・TSUMUGI_SETTINGS・TSUMUGI_PTY_LOG・TSUMUGI_KEYLOG・WINTEST_KIT=$kit）。素の ``tsumugi ls`` もこの実行のサーバに届き、持ち主のサーバには届きません。終わる前に ``Stop-Mine`` を呼びます。
- 作業用の一時ディレクトリは $Scratch で、TEMP / TMP も既にそこを指しています（役割定義に出てくる R:\Temp は、すべてここに読み替えてください）。
"@

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
    $self = (git -C $PSScriptRoot log -1 --format='%h %s' -- auto-wintest.ps1 2>$null) -join ''
    Say "Script: $PSCommandPath ($self)"
    Say "Model: $Model"
    $inWork = [IO.Path]::GetFullPath($PSScriptRoot).StartsWith([IO.Path]::GetFullPath($Work), [StringComparison]::OrdinalIgnoreCase)
    if (-not $inWork) {
        Say "This script is not the worktree's copy, so it does not follow origin/main. Point the task at $Work\scripts\auto-wintest.ps1 (see the top of the script)."
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
    $env:TSUMUGI_STATE = Join-Path $kit 'state'
    $env:TSUMUGI_SETTINGS = Join-Path $kit 'settings.toml'
    $env:TSUMUGI_PTY_LOG = Join-Path $kit 'pty.log'
    $env:TSUMUGI_KEYLOG = '1'
    Remove-Item Env:TSUMUGI_SESSION -ErrorAction SilentlyContinue

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
        # until its words change or it is named in the re-tests.
        Add-Attempted $attemptedFile $chunk
        Remove-Item -LiteralPath $failFile -ErrorAction SilentlyContinue
        Say "Done: $tail"
    } elseif ($code -ne 0 -and $out -match 'limit') {
        Say 'Hit a usage limit. Trying again next time.'
    } else {
        Add-Failure $branch "exit $code; last line: $tail"
        exit 1
    }
} finally {
    if ($desktop) { $desktop.ReleaseMutex() }
    $mutex.ReleaseMutex()
}
