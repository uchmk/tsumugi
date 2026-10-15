#!/usr/bin/env bash
# Every check a push to main must pass, from a Linux machine (a cloud
# session) or the owner's Windows PC. The first failure stops with its
# output; the last line is `ALL OK: ...` when everything passed. Like kura's
# scripts/verify.sh.
#
# Besides the host's own clippy, the other CI target (Linux or Windows) gets
# one. It is a type check only -- it never runs that code, so read Windows
# path strings by eye. Before 2026-10-15 the second clippy was always
# Windows, so a run on the Windows PC never compiled Linux and still said
# ALL OK (yagura's v0.62.0 went red on Linux that way).

set -o pipefail
cd "$(dirname "$0")/.." || exit 1

host=$(rustc -vV | sed -n 's/^host: //p')
others=
for t in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc; do
    [ "$t" = "$host" ] && continue
    others="$others $t"
    rustup target list --installed 2>/dev/null | grep -qx "$t" || rustup target add "$t" >/dev/null 2>&1
done

run() {
    local out rc
    out=$("$@" 2>&1)
    rc=$?
    if [ $rc -ne 0 ]; then
        echo "FAILED: $*"
        echo "$out" | tail -30
        exit 1
    fi
    last=$out
}

run cargo fmt --all --check
run cargo build -q --locked --workspace --all-features
run cargo test -q --workspace --all-features
tests=$(echo "$last" | grep 'test result' | awk '{p+=$4; f+=$6} END {print p " passed; " f " failed"}')
run cargo +stable clippy -q --workspace --all-targets --all-features -- -D warnings
for t in $others; do
    run cargo +stable clippy -q --workspace --all-targets --all-features --target "$t" -- -D warnings
done
run cargo +stable clippy -q --workspace --all-targets -- -D warnings
# The checklists follow TESTING.md and keys.rs.
run cargo run -q -p tsumugi --example make-testcheck -- --check
run cargo run -q -p tsumugi --example make-keycheck -- --check
# The rules the Merge lanes workflow merges the lanes' pull requests by.
run python3 scripts/merge-lanes.py --self-test
# The lanes' shared scripts are ito's (scripts/lanes.conf's files=).
run scripts/lanes.sh check
# The Windows test lane's scripts (scripts/check-ps1.ps1; CI runs it too).
if command -v pwsh >/dev/null; then
    run pwsh -NoProfile -File scripts/check-ps1.ps1
else
    echo "(no pwsh: scripts/check-ps1.ps1 left to CI)"
fi
echo "ALL OK: $tests"
