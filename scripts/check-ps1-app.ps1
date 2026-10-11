# tsumugi's own checks, dot-sourced by scripts/check-ps1.ps1 (ito's, the same
# for every app) after its own: Expect and $bad are its. wintest-queue.ps1
# picks the chunks it should from a small sample.

. (Join-Path $PSScriptRoot 'wintest-queue.ps1')

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
# Two retest issues, the oldest first: #7's rows are done (one ticked in the
# issue, one ticked in the checklist), so #9's are given.
$retests = @(
    [pscustomobject]@{ Number = 7; Body = "Changed in v1.0.0.`n`n- [x] 2.4`n- [ ] 1.1`n" },
    [pscustomobject]@{ Number = 9; Body = "## Rows`r`n`r`n- [ ] 2.4`r`n- [ ] **1.3** the window opens`r`n- [x] 2.1`r`n" }
)

Expect 'the ids an issue names' ((Get-RetestIds $retests[1].Body) -join ',') '2.4,1.3'
$c = Select-Chunk -Lane win -Checks $checks -Keys $keys -Retests $retests
Expect 'the re-test chunk' "$($c.Kind) $($c.Issue) $($c.Branch) $($c.Rows.Id -join ',')" 'retest 9 test/win-retest-9 1.3,2.4'
Expect 'the re-tests as given' (Format-Chunk $c) "## Re-tests of changed behaviour (#9)`n`n- **1.3** the window opens`n- **2.4** a fourth"
$c = Select-Chunk -Lane arm -Checks $checks -Keys $keys -Retests $retests -Rows 3
Expect 'the ARM64 lane leaves the re-tests' "$($c.Kind) $($c.Branch) $($c.Rows.Id -join ',')" 'rows test/arm-2-1 2.1,2.2,2.3'
Expect 'the chunk as given' (Format-Chunk $c) "## 2. Panes and splits`n`n- **2.1** a split`n- **2.2** another`n- **2.3** a third"
$c = Select-Chunk -Lane win -Checks $checks -Keys $keys
Expect 'the keys chunk' "$($c.Kind) $($c.Branch) $($c.Rows.Id -join ',')" 'keys test/win-keys-2 key 2,key 3'

$tmp = Join-Path ([IO.Path]::GetTempPath()) "check-ps1-$PID.txt"
try {
    # A re-test tried and left `[ ]` is not given again by the same issue,
    # and is by a new one.
    Add-Attempted $tmp (Select-Chunk -Lane win -Checks $checks -Keys $keys -Retests $retests)
    $c = Select-Chunk -Lane win -Checks $checks -Keys $keys -Retests $retests -Attempted (Read-Attempted $tmp)
    Expect 'after the attempted re-tests' $c.Kind 'keys'
    $again = @($retests) + @([pscustomobject]@{ Number = 12; Body = '- [ ] 1.3' })
    $c = Select-Chunk -Lane win -Checks $checks -Keys $keys -Retests $again -Attempted (Read-Attempted $tmp)
    Expect 'a row named in a new issue' "$($c.Branch) $($c.Rows.Id -join ',')" 'test/win-retest-12 1.3'
    Remove-Item -LiteralPath $tmp

    $c = Select-Chunk -Lane arm -Checks $checks -Keys $keys -Rows 3
    Add-Attempted $tmp $c
    $c = Select-Chunk -Lane arm -Checks $checks -Keys $keys -Rows 3 -Attempted (Read-Attempted $tmp)
    Expect 'after the attempted rows' "$($c.Branch) $($c.Rows.Id -join ',')" 'test/arm-2-4 2.4'
    Add-Attempted $tmp $c
    $c = Select-Chunk -Lane arm -Checks $checks -Keys $keys -Rows 3 -Attempted (Read-Attempted $tmp)
    Expect 'the next section' "$($c.Branch) $($c.Rows.Id -join ',')" 'test/arm-1-3 1.3,1.4'
    # A row whose words changed is offered again.
    $c = Select-Chunk -Lane arm -Checks ($checks -replace 'a split', 'a split, reworded') -Keys $keys -Rows 3 -Attempted (Read-Attempted $tmp)
    Expect 'a reworded row' "$($c.Branch) $($c.Rows.Id -join ',')" 'test/arm-2-1 2.1'
} finally {
    Remove-Item -LiteralPath $tmp -ErrorAction SilentlyContinue
}
$c = Select-Chunk -Lane arm -Checks '- [x] **2.1** all done' -Keys $keys
Expect 'nothing left' ($null -eq $c) $true
# Rows for Linux/macOS, for a person, or skipped by the lane are never given.
$other = "## 2. Panes`n`n- [ ] **2.1** Linux/macOS: ls`n- [ ] **2.2** A person: the IME`n- [ ] **2.46** img2sixel`n- [ ] **2.5** a split"
$c = Select-Chunk -Lane arm -Checks $other -Keys $keys
Expect 'the rows a lane cannot do' "$($c.Rows.Id -join ',')" '2.5'
$c = Select-Chunk -Lane win -Checks $other -Keys ''
Expect 'the x64 lane takes 2.46' "$($c.Rows.Id -join ',')" '2.46,2.5'

if (-not $bad) { Write-Output 'The wintest queue picks the chunks it should.' }
