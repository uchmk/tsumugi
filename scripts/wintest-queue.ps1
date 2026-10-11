# Which rows the next unattended run takes. Dot-sourced by auto-wintest.ps1;
# plain functions over the checklists' text, so scripts/check-ps1.ps1 tests
# them on Linux too.
#
# A run gets one chunk: up to -Rows rows of one section of TESTING-CHECKS.md,
# -Keys lines of TESTING-KEYS.md, or the re-tests of one `retest` issue. It is
# told those rows and nothing else, which is most of what keeps a run small.
# A row a run was given and left `[ ]` (a look, a failure, a row it could not
# reach) is written to the attempted list with a hash of its words, so the
# next run moves on; changing the row's words in TESTING.md, or naming it in a
# new retest issue, offers it again.
#
# The marks a run set are made again on main by lane-marks.ps1 (ito's).

# The win lane's order after the re-tests and the keys; the sections not
# named follow in their own order. The ARM64 lane takes only the sections
# where the CPU can make a difference (ConPTY, the server, the toasts, the
# hooks) and leaves the keys and the rest to x64.
$WintestOrder = @{
    win = @(1, 18, 12, 19, 4, 2, 16, 17, 8, 13, 15)
    arm = @(2, 4, 12, 1)
}

# Rows a lane never takes: the tools they need are not on that machine
# (2.46-2.49 on the ARM64 one: img2sixel, chafa, kitten, imgcat, WSL), and
# 1.12 on both: it needs the build just before, speaking the same protocol
# version, and the older build a run fetches (1.7, 1.11) is a release.
$WintestSkip = @{
    win = @('1.12')
    arm = @('1.12', '2.46', '2.47', '2.48', '2.49')
}

# A row an unattended Windows run can do: not one for Linux or macOS only
# (`Linux/macOS: …`), not one that needs a person (`A person: …`, an IME, a
# layout switch), not one the lane skips.
function Test-Unattended([string]$Lane, $Row) {
    if ($Row.Text -match '^(Linux/macOS|Linux|macOS|A person):') { return $false }
    $Row.Id -notin $WintestSkip[$Lane]
}

# Every `- [ ] **N.M** text` of TESTING-CHECKS.md, in file order, with its
# section's number and title.
function Get-OpenRows([string]$Checks) {
    $title = @{}
    foreach ($line in $Checks -split "`r?`n") {
        if ($line -match '^## (\d+)\. (.*)$') { $title[[int]$Matches[1]] = $Matches[2]; continue }
        if ($line -match '^- \[ \] \*\*((\d+)\.[0-9A-Za-z]+)\*\* (.*)$') {
            $sec = [int]$Matches[2]
            [pscustomobject]@{ Id = $Matches[1]; Section = $sec; Title = $title[$sec]; Text = $Matches[3] }
        }
    }
}

# Every `- [ ] …` of TESTING-KEYS.md, numbered over all its key lines
# (ticked or not) so a chunk's branch name says where it starts.
function Get-OpenKeys([string]$Keys) {
    $n = 0
    foreach ($line in $Keys -split "`r?`n") {
        if ($line -notmatch '^- \[(.)\] (.*)$') { continue }
        $n++
        if ($Matches[1] -eq ' ') { [pscustomobject]@{ Id = "key $n"; Number = $n; Text = $Matches[2] } }
    }
}

# The row ids a retest issue's body still names `[ ]`: `- [ ] 2.52` or
# `- [ ] **2.52** …` (since v0.94.0 the re-tests are issues, not a row of the
# role's table).
function Get-RetestIds([string]$Body) {
    @([regex]::Matches($Body, '(?m)^\s*- \[ \] \**(\d+\.\d+[a-z]?)\b') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
}

function Get-TextHash([string]$Text) {
    $bytes = [Text.Encoding]::UTF8.GetBytes($Text)
    -join ([Security.Cryptography.SHA256]::HashData($bytes)[0..7] | ForEach-Object { $_.ToString('x2') })
}

# The attempted list: `<id>\t<hash>` lines -> @{ id = hash }.
function Read-Attempted([string]$Path) {
    $seen = @{}
    if (Test-Path -LiteralPath $Path) {
        foreach ($line in Get-Content -LiteralPath $Path) {
            $id, $hash = $line -split "`t", 2
            if ($id) { $seen[$id] = $hash }
        }
    }
    $seen
}

function Add-Attempted([string]$Path, $Chunk) {
    $seen = Read-Attempted $Path
    foreach ($r in $Chunk.Rows) { $seen[$r.Id] = $r.Hash }
    $lines = $seen.Keys | Sort-Object | ForEach-Object { "$_`t$($seen[$_])" }
    Set-Content -LiteralPath $Path -Value $lines
}

# The next chunk for a lane, or $null when there is nothing left to give.
# Each row comes back with the hash it is remembered by.
function Select-Chunk {
    param(
        [ValidateSet('win', 'arm')] [string]$Lane,
        [string]$Checks,
        [string]$Keys,
        # the open retest issues, oldest first: @{ Number; Body }
        $Retests = @(),
        [hashtable]$Attempted = @{},
        [int]$Rows = 15,
        [int]$KeyCount = 20
    )
    $open = @(Get-OpenRows $Checks)

    # 1. Re-tests: the rows an open retest issue names, still `[ ]`, the
    # oldest issue first, x64 only (the ARM64 lane keeps to its sections). The
    # issue's number is in the hash, so a row named again in a new issue is
    # offered again.
    if ($Lane -eq 'win') {
        foreach ($issue in @($Retests)) {
            $ids = Get-RetestIds $issue.Body
            $picked = @($open | Where-Object { $_.Id -in $ids -and (Test-Unattended $Lane $_) } | ForEach-Object {
                    $r = $_.PSObject.Copy()
                    $r | Add-Member Hash (Get-TextHash ($r.Text + "`n#" + $issue.Number))
                    $r
                } | Where-Object { $Attempted[$_.Id] -ne $_.Hash } | Select-Object -First $Rows)
            if ($picked) {
                return [pscustomobject]@{ Kind = 'retest'; Title = "Re-tests of changed behaviour (#$($issue.Number))"; Rows = $picked; Issue = [int]$issue.Number; Branch = "test/$Lane-retest-$($issue.Number)" }
            }
        }
    }

    # 2. Keys, x64 only.
    if ($Lane -eq 'win' -and $Keys) {
        $picked = @(Get-OpenKeys $Keys | ForEach-Object { $_ | Add-Member Hash (Get-TextHash $_.Text) -PassThru } |
            Where-Object { $Attempted[$_.Id] -ne $_.Hash } | Select-Object -First $KeyCount)
        if ($picked) {
            return [pscustomobject]@{ Kind = 'keys'; Title = 'TESTING-KEYS.md'; Rows = $picked; Branch = "test/$Lane-keys-$($picked[0].Number)" }
        }
    }

    # 3. The sections, in the lane's order.
    $order = $WintestOrder[$Lane]
    if ($Lane -eq 'win') {
        $order = @($order) + @($open | ForEach-Object Section | Select-Object -Unique | Sort-Object | Where-Object { $_ -notin $order })
    }
    foreach ($sec in $order) {
        $picked = @($open | Where-Object { $_.Section -eq $sec -and (Test-Unattended $Lane $_) } | ForEach-Object { $_ | Add-Member Hash (Get-TextHash $_.Text) -PassThru -Force } |
            Where-Object { $Attempted[$_.Id] -ne $_.Hash } | Select-Object -First $Rows)
        if ($picked) {
            return [pscustomobject]@{ Kind = 'rows'; Title = "$sec. $($picked[0].Title)"; Rows = $picked; Branch = "test/$Lane-$($picked[0].Id -replace '\.', '-')" }
        }
    }
    $null
}

# The chunk as the prompt gives it: one line per row.
function Format-Chunk($Chunk) {
    $lines = foreach ($r in $Chunk.Rows) {
        if ($Chunk.Kind -eq 'keys') { "- $($r.Text)" } else { "- **$($r.Id)** $($r.Text)" }
    }
    "## $($Chunk.Title)`n`n" + ($lines -join "`n")
}
