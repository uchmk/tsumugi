#!/usr/bin/env bash
# Every check a push to main must pass, from a Linux machine (a cloud
# session). The first failure stops with its output; the last line is
# `ALL OK: ...` when everything passed. The same as filer's scripts/verify.sh.
#
# The Windows clippy is a type check only -- it never runs Windows code, so
# read path strings by eye. Never add `cargo fmt` here.

set -o pipefail
cd "$(dirname "$0")/.." || exit 1

rustup target list --installed 2>/dev/null | grep -qx x86_64-pc-windows-msvc \
    || rustup target add x86_64-pc-windows-msvc >/dev/null 2>&1

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

run cargo build -q --locked --workspace --all-features
run cargo test -q --workspace --all-features
tests=$(echo "$last" | grep 'test result' | awk '{p+=$4; f+=$6} END {print p " passed; " f " failed"}')
run cargo +stable clippy -q --workspace --all-targets --all-features -- -D warnings
run cargo +stable clippy -q --workspace --all-targets --all-features --target x86_64-pc-windows-msvc -- -D warnings
run cargo +stable clippy -q --workspace --all-targets -- -D warnings
# The checklists follow TESTING.md and keys.rs.
run cargo run -q -p tsumugi --example make-testcheck -- --check
run cargo run -q -p tsumugi --example make-keycheck -- --check
# The rules the Merge lanes workflow merges the lanes' pull requests by.
run python3 scripts/merge-lanes.py --self-test
# The Windows test lane's scripts (scripts/check-ps1.ps1; CI runs it too).
if command -v pwsh >/dev/null; then
    run pwsh -NoProfile -File scripts/check-ps1.ps1
else
    echo "(no pwsh: scripts/check-ps1.ps1 left to CI)"
fi
echo "ALL OK: $tests"
