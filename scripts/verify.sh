#!/usr/bin/env bash
# Every check a push to main must pass, from a Linux machine (a cloud
# session) or the owner's Windows PC. The first failure stops with its
# output; the last line is `ALL OK: ...` when everything passed. Like kura's
# scripts/verify.sh.
#
# Clippy covers every OS tsumugi ships for -- Linux, Windows and macOS --
# whatever the host: the host first, then the others. Those are type checks
# only -- they never run that code, so read Windows path strings by eye.
# objc2 compiles a little Objective-C for macOS, so off a Mac that clippy
# goes through cargo-zigbuild, whose zig is a C compiler for it (installed
# from PyPI below when it is missing). Before 2026-10-15 the second clippy
# was always Windows, so a run on the Windows PC never compiled Linux and
# still said ALL OK (yagura's v0.62.0 went red on Linux that way); before
# 2026-10-16 macOS was left to CI.

set -o pipefail
cd "$(dirname "$0")/.." || exit 1

host=$(rustc -vV | sed -n 's/^host: //p')
others=
for t in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc aarch64-apple-darwin; do
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

# cargo-zigbuild comes from PyPI with zig. Where pip may not touch the system
# Python (PEP 668) it goes to ~/.local/bin. zig's cache goes with Cargo's
# output (on the owner's PC the RAM disk, not the SSD).
zigbuild() {
    if ! command -v cargo-zigbuild >/dev/null 2>&1; then
        python3 -m pip install -q cargo-zigbuild >/dev/null 2>&1 \
            || python3 -m pip install -q --user --break-system-packages cargo-zigbuild >/dev/null 2>&1
        PATH="$PATH:$HOME/.local/bin"
        command -v cargo-zigbuild >/dev/null 2>&1 || { echo "FAILED: no cargo-zigbuild (pip install cargo-zigbuild)"; exit 1; }
    fi
    local dir
    dir=$(cargo metadata -q --no-deps --format-version 1 | sed 's/.*"target_directory":"\([^"]*\)".*/\1/; s/\\\\/\//g')
    run env RUSTUP_TOOLCHAIN=stable ZIG_GLOBAL_CACHE_DIR="$dir/zig" cargo-zigbuild "$@"
}

run cargo fmt --all --check
run cargo build -q --locked --workspace --all-features
run cargo test -q --workspace --all-features
tests=$(echo "$last" | grep 'test result' | awk '{p+=$4; f+=$6} END {print p " passed; " f " failed"}')
run cargo +stable clippy -q --workspace --all-targets --all-features -- -D warnings
for t in $others; do
    case "$t:$host" in
        *-apple-darwin:*-apple-darwin | *-linux-gnu:* | *-windows-msvc:*)
            run cargo +stable clippy -q --workspace --all-targets --all-features --target "$t" -- -D warnings ;;
        *) zigbuild clippy -q --workspace --all-targets --all-features --target "$t" -- -D warnings ;;
    esac
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
