# The PowerShell scripts, checked from any machine (CI runs it on Linux):
# every scripts/*.ps1 parses, and wintest-queue.ps1 picks the chunks it
# should from a small sample. The scripts themselves run only on Windows, so
# this is all a cloud session can learn about them before a push.
#
#   pwsh -NoProfile -File scripts/check-ps1.ps1

$ErrorActionPreference = 'Stop'
$failed = 0

foreach ($f in Get-ChildItem -Path $PSScriptRoot -Filter '*.ps1') {
    $tokens = $null
    $errors = $null
    [void][System.Management.Automation.Language.Parser]::ParseFile($f.FullName, [ref]$tokens, [ref]$errors)
    foreach ($e in $errors) {
        "$($f.Name):$($e.Extent.StartLineNumber): $($e.Message)"
        $failed++
    }
}

. (Join-Path $PSScriptRoot 'wintest-queue.ps1')

function Expect([string]$What, $Got, $Want) {
    if ("$Got" -ne "$Want") { "queue: $What is '$Got', not '$Want'"; $script:failed++ }
}

$checks = @'
## 1. The window and the server

- [x] **1.1** done already
- [ ] **1.3** the window opens
- [ ] **1.4** the title reads

## 2. Panes and splits

- [ ] **2.1** a split
- [ ] **2.2** another
- [ ] **2.3** a third
- [ ] **2.4** a fourth
'@
$keys = @'
- [x] `Ctrl+Shift+T` new tab
- [ ] `Ctrl+Shift+W` close
- [ ] `F2` rename
'@
$role = @'
| Chunk | Up to | Notes |
| --- | --- | --- |
| **Re-tests of changed behaviour** | 15 rows | The rows named here, still `[ ]`: 2.4, 1.3 |
'@
$none = '| **Re-tests of changed behaviour** | none yet |'

$c = Select-Chunk -Lane arm -Checks $checks -Keys $keys -Role $role
Expect 'the re-test chunk' "$($c.Kind) $($c.Branch) $($c.Rows.Id -join ',')" 'retest test/arm-retest-1-3 1.3,2.4'
$c = Select-Chunk -Lane win -Checks $checks -Keys $keys -Role $none
Expect 'the keys chunk' "$($c.Kind) $($c.Branch) $($c.Rows.Id -join ',')" 'keys test/win-keys-2 key 2,key 3'
$c = Select-Chunk -Lane arm -Checks $checks -Keys $keys -Role $none -Rows 3
Expect 'the ARM64 lane, section 2 first' "$($c.Kind) $($c.Branch) $($c.Rows.Id -join ',')" 'rows test/arm-2-1 2.1,2.2,2.3'
Expect 'the chunk as given' (Format-Chunk $c) "## 2. Panes and splits`n`n- **2.1** a split`n- **2.2** another`n- **2.3** a third"

$tmp = Join-Path ([IO.Path]::GetTempPath()) "check-ps1-$PID.txt"
try {
    Add-Attempted $tmp $c
    $c = Select-Chunk -Lane arm -Checks $checks -Keys $keys -Role $none -Rows 3 -Attempted (Read-Attempted $tmp)
    Expect 'after the attempted rows' "$($c.Branch) $($c.Rows.Id -join ',')" 'test/arm-2-4 2.4'
    Add-Attempted $tmp $c
    $c = Select-Chunk -Lane arm -Checks $checks -Keys $keys -Role $none -Rows 3 -Attempted (Read-Attempted $tmp)
    Expect 'the next section' "$($c.Branch) $($c.Rows.Id -join ',')" 'test/arm-1-3 1.3,1.4'
    # A row whose words changed is offered again.
    $c = Select-Chunk -Lane arm -Checks ($checks -replace 'a split', 'a split, reworded') -Keys $keys -Role $none -Rows 3 -Attempted (Read-Attempted $tmp)
    Expect 'a reworded row' "$($c.Branch) $($c.Rows.Id -join ',')" 'test/arm-2-1 2.1'
} finally {
    Remove-Item -LiteralPath $tmp -ErrorAction SilentlyContinue
}
$c = Select-Chunk -Lane arm -Checks '- [x] **2.1** all done' -Keys $keys -Role $none
Expect 'nothing left' ($null -eq $c) $true
# Rows for Linux/macOS, for a person, or skipped by the lane are never given.
$other = "## 2. Panes`n`n- [ ] **2.1** Linux/macOS: ls`n- [ ] **2.2** A person: the IME`n- [ ] **2.46** img2sixel`n- [ ] **2.5** a split"
$c = Select-Chunk -Lane arm -Checks $other -Keys $keys -Role $none
Expect 'the rows a lane cannot do' "$($c.Rows.Id -join ',')" '2.5'
$c = Select-Chunk -Lane win -Checks $other -Keys '' -Role $none
Expect 'the x64 lane takes 2.46' "$($c.Rows.Id -join ',')" '2.46,2.5'

# A run's marks made again on a main that moved: 2.1 ticked (main left it
# alone), 2.2 ticked (main reworded it: dropped), 2.3 `[~]` (main ticked it
# too: kept, not dropped), a key ticked; main added 2.5 and kept CRLF.
$base = "## 2. Panes`n`n- [ ] **2.1** a split`n- [ ] **2.2** another`n- [ ] **2.3** a third`n- [x] **2.4** done`n- [ ] ``F2`` rename"
$ran = "## 2. Panes`n`n- [x] **2.1** a split`n- [x] **2.2** another`n- [~] **2.3** a third`n- [x] **2.4** done`n- [x] ``F2`` rename"
$marks = @(Get-LaneMarks -Base $base -Branch $ran)
Expect 'the marks a run set' (($marks | ForEach-Object { "$($_.Mark) $($_.Rest)" }) -join '|') 'x **2.1** a split|x **2.2** another|~ **2.3** a third|x `F2` rename'
$main = "## 2. Panes`r`n`r`n- [ ] **2.1** a split`r`n- [ ] **2.2** another, reworded`r`n- [~] **2.3** a third`r`n- [x] **2.4** done`r`n- [ ] **2.5** new`r`n- [ ] ``F2`` rename`r`n"
$r = Set-LaneMarks -Main $main -Marks $marks
Expect 'the marks on main' $r.Text "## 2. Panes`r`n`r`n- [x] **2.1** a split`r`n- [ ] **2.2** another, reworded`r`n- [~] **2.3** a third`r`n- [x] **2.4** done`r`n- [ ] **2.5** new`r`n- [x] ``F2`` rename`r`n"
Expect 'the marks dropped' ($r.Dropped -join ',') '2.2'
$r = Set-LaneMarks -Main "- [ ] ``F3`` other`n" -Marks @(Get-LaneMarks -Base '- [ ] `F2` rename' -Branch '- [x] `F2` rename')
Expect 'a key dropped' "$($r.Dropped)|$($r.Text)" "``F2`` rename|- [ ] ``F3`` other`n"
Expect 'no marks' (Set-LaneMarks -Main "a`nb" -Marks @()).Text "a`nb"

if ($failed) { "check-ps1: $failed problem(s)"; exit 1 }
'check-ps1: OK'
