# A lane's state on GitHub, where a cloud session can read it: one issue per
# lane, "Lane status: <lane>" with the label lane-status, whose body every
# firing of auto-wintest.ps1 writes again. Editing a body sends no
# notification. Until this, a lane that stopped said why only on its own
# machine (the log, dirty.txt, the failures file): on 2026-10-10 kura's lanes
# had made no pull request for a day, and nobody outside the machine could
# tell why.
# Dot-sourced by auto-wintest.ps1 and checked by check-ps1.ps1. The same file
# in kura and tsumugi.

# The last $Keep lines of a firing history kept in $File, with $Line added
# first. Never throws.
function Add-LaneHistory([string]$File, [string]$Line, [int]$Keep = 24) {
    $lines = @()
    try { if (Test-Path $File) { $lines = @(Get-Content -LiteralPath $File) } } catch { }
    $lines = @(@($Line) + $lines | Select-Object -First $Keep)
    try { Set-Content -LiteralPath $File -Value $lines } catch { }
    $lines
}

# The issue's body. $Said is this firing's log lines, $Dirty the worktree's
# `git status --porcelain`, $History the firings before (newest first). The
# user's own folder is written as ~ (the repository is public), an @ cannot
# mention anyone, and nothing here is the run's own output but its last line.
function Format-LaneStatus {
    param(
        [string]$Lane, [datetime]$When, [string]$Script, [string]$Outcome, [string]$Task,
        [string[]]$Said = @(), [string[]]$Dirty = @(), [int]$Failures = 0,
        [string[]]$History = @(), [string]$UserDir = $env:USERPROFILE
    )
    $fence = '```'
    $clean = {
        param([string]$s)
        if ($UserDir) { $s = $s.Replace($UserDir, '~') }
        $s = $s.Replace($fence, "'''") -replace '@', "@$([char]0x200B)"
        if ($s.Length -gt 300) { $s = $s.Substring(0, 300) + '...' }
        $s
    }
    $worktree = if ($Dirty.Count) {
        "**dirty** ($($Dirty.Count) changed path(s)): no run starts until a person looks at it and cleans it"
    } else { 'clean' }
    $body = @(
        '<!-- Written by scripts/auto-wintest.ps1 on the lane''s machine at every firing (scripts/lane-status.ps1). An edit here is overwritten. -->'
        ('**Lane** `{0}` · **Last firing** {1:yyyy-MM-dd HH:mm zzz} · **Script** {2}' -f $Lane, $When, (& $clean $Script))
        ''
        "**This firing:** $(& $clean $Outcome)"
        ''
        "**Worktree:** $worktree · **Failed runs in a row:** $Failures"
        ''
        "If **Last firing** is more than two hours old and the firing before it did not start a run (a run may take up to four), the scheduled task is not starting the script: the machine is off or asleep, or the task fails before the script runs (on the machine: ``Get-ScheduledTaskInfo $Task``, LastTaskResult)."
    )
    if ($Said.Count) {
        $body += @('', "<details><summary>This firing's log ($($Said.Count) lines)</summary>", '', "${fence}text")
        $body += @($Said | Select-Object -Last 60 | ForEach-Object { & $clean $_ })
        $body += @($fence, '', '</details>')
    }
    if ($History.Count) {
        $body += @('', "<details><summary>The firings before ($($History.Count))</summary>", '', "${fence}text")
        $body += @($History | ForEach-Object { & $clean $_ })
        $body += @($fence, '', '</details>')
    }
    if ($Dirty.Count) {
        $body += @('', '<details><summary>git status --porcelain</summary>', '', "${fence}text")
        $body += @($Dirty | Select-Object -First 40 | ForEach-Object { & $clean $_ })
        $body += @($fence, '', '</details>')
    }
    ($body -join "`n") + "`n"
}

# Writes $Body into the lane's issue, made the first time. Its number is kept
# in $StateDir; when that is missing or stale, the issue is found by its title.
# REST only (gh api). Never throws:
# returns $null, or what went wrong.
function Publish-LaneStatus([string]$Repo, [string]$Lane, [string]$Body, [string]$StateDir) {
    try {
        $title = "Lane status: $Lane"
        $numFile = Join-Path $StateDir "status-issue-$Lane"
        $json = Join-Path $StateDir "status-body-$Lane.json"
        $utf8 = [Text.UTF8Encoding]::new($false)
        [IO.File]::WriteAllText($json, (@{ body = $Body } | ConvertTo-Json -Compress), $utf8)
        $n = if (Test-Path $numFile) { (Get-Content -Raw -LiteralPath $numFile).Trim() } else { '' }
        if ($n -match '^\d+$') {
            $out = gh api -X PATCH "repos/$Repo/issues/$n" --input $json --jq .number 2>&1
            if ($LASTEXITCODE -eq 0) { return $null }
        }
        # Not the title as it is: "status:" would be read as a search qualifier.
        $found = gh api -X GET search/issues -f "q=repo:$Repo is:issue in:title Lane status $Lane" --jq .items 2>&1
        if ($LASTEXITCODE -ne 0) { return "gh api search/issues failed: $found" }
        $hit = @($found | Out-String | ConvertFrom-Json | Where-Object { $_.title -eq $title } | Sort-Object number) | Select-Object -First 1
        if ($hit) {
            $n = "$($hit.number)"
            $out = gh api -X PATCH "repos/$Repo/issues/$n" --input $json --jq .number 2>&1
            if ($LASTEXITCODE -ne 0) { return "gh api could not edit #${n}: $out" }
        } else {
            # The label is what a cloud session finds the issues by (GitHub makes
            # it the first time); without it, if that is refused.
            [IO.File]::WriteAllText($json, (@{ title = $title; body = $Body; labels = @('lane-status') } | ConvertTo-Json -Compress), $utf8)
            $out = gh api "repos/$Repo/issues" --input $json --jq .number 2>&1
            if ($LASTEXITCODE -ne 0) {
                [IO.File]::WriteAllText($json, (@{ title = $title; body = $Body } | ConvertTo-Json -Compress), $utf8)
                $out = gh api "repos/$Repo/issues" --input $json --jq .number 2>&1
            }
            if ($LASTEXITCODE -ne 0 -or "$out".Trim() -notmatch '^\d+$') { return "gh api could not open the issue: $out" }
            $n = "$out".Trim()
        }
        Set-Content -NoNewline -LiteralPath $numFile -Value $n
        $null
    } catch {
        "Publish-LaneStatus: $_"
    }
}
