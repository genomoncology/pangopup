#!/usr/bin/env bash
set -euo pipefail

# A harness that runs an executable out of `target/debug` is making a claim
# about the source beside it. An executable left over from an older source is a
# renderer from the past: change what the renderer prints, skip the build, run
# `tests/production-release-qualification.sh` on its own, and the stub and the
# comparison beside it both reach the same stale bytes, agree with each other,
# and the harness goes green while the release would fail. Checking that the
# file exists -- which is all `[[ -x "$real_cli" && ! -L "$real_cli" ]]` did --
# does not see that.
#
# One mechanism answers it for every harness, so two call sites cannot drift
# apart again: `scripts/require-built-commands.sh` builds the commands the
# harnesses run. This file holds two claims.
#
#   1. Every harness under tests/ that reaches into target/debug calls that
#      script before its first use.
#   2. The script builds first and refuses afterwards. Proved against a fake
#      `cargo`, so this gate never runs a real build.
#
# What this does not prove: that cargo's own freshness check is correct, or that
# an executable someone overwrote in place is detected. The guarantee is the one
# `make spec` already relies on -- edited source is rebuilt before use.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
script="$repository/scripts/require-built-commands.sh"
self=$(basename "${BASH_SOURCE[0]}")

fail() { printf 'built executable currency: %s\n' "$*" >&2; exit 1; }

[[ -f "$script" ]] || fail "no $script: nothing makes target/debug match the source a harness compares against it"
[[ -x "$script" ]] || fail "$script is not executable"

# --- 1. every harness reaching into target/debug builds first ---------------
#
# Discovered rather than listed, so a third harness cannot start using a built
# executable without being held to the same rule. The two the repository has
# today are still named, so a scan that silently matches nothing fails here
# instead of passing. Both patterns require the match to stand in code: a line
# that only mentions the build script in a comment is not a call, and a comment
# naming an executable is not a use.
guarded=0
for harness in "$repository"/tests/*.sh; do
    name=$(basename "$harness")
    [[ "$name" != "$self" ]] || continue
    use_line=$({ grep -nE '^[^#]*target/debug/[A-Za-z]' "$harness" || true; } | head -n 1 | cut -d: -f1)
    [[ -n "$use_line" ]] || continue
    guarded=$((guarded + 1))
    build_line=$({ grep -nE '^[^#]*scripts/require-built-commands\.sh' "$harness" || true; } | head -n 1 | cut -d: -f1)
    [[ -n "$build_line" ]] \
        || fail "tests/$name uses an executable from target/debug at line $use_line without building it first, so a standalone run compares against whatever the last build left behind"
    [[ "$build_line" -lt "$use_line" ]] \
        || fail "tests/$name builds at line $build_line but first uses target/debug at line $use_line: a missing or stale build has to be answered before the first use, not after"
done
for required in production-release-qualification.sh executable-delivery.sh; do
    grep -qE '^[^#]*target/debug/[A-Za-z]' "$repository/tests/$required" \
        || fail "tests/$required no longer reaches into target/debug, so this scan is checking the wrong files"
done
[[ "$guarded" -ge 2 ]] || fail "the scan found $guarded harness(es) using target/debug, so it matched less than the repository holds"

# --- 2. the script builds first, then refuses ------------------------------
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
fake_bin="$fixture/bin"
mkdir "$fake_bin"
cat >"$fake_bin/cargo" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"${CARGO_CALLS:?}"
for name in ${CARGO_PRODUCES-}; do
    mkdir -p "${CARGO_BUILT:?}"
    printf '#!/bin/sh\nexit 0\n' >"$CARGO_BUILT/$name"
    chmod +x "$CARGO_BUILT/$name"
done
exit "${CARGO_STATUS:?}"
SH
chmod +x "$fake_bin/cargo"

# The script resolves its repository from its own location, so each case gets a
# throwaway checkout holding nothing but a copy of the script.
new_case() {
    CASE=$(mktemp -d "$fixture/case.XXXXXX")
    mkdir -p "$CASE/scripts" "$CASE/target/debug"
    install -m 755 "$script" "$CASE/scripts/require-built-commands.sh"
    CALLS="$CASE/cargo-calls"
    : >"$CALLS"
}

run_script() {
    local cargo_status=$1 produces=$2
    set +e
    PATH="$fake_bin:$PATH" CARGO_CALLS="$CALLS" CARGO_STATUS="$cargo_status" \
        CARGO_PRODUCES="$produces" CARGO_BUILT="$CASE/target/debug" \
        bash "$CASE/scripts/require-built-commands.sh" >"$CASE/out" 2>"$CASE/err"
    SCRIPT_STATUS=$?
    set -e
    SCRIPT_ERROR=$(cat "$CASE/err")
}

# A build that produces both commands is accepted, and it is one build.
new_case
run_script 0 'pangopup pangopup-build'
[[ "$SCRIPT_STATUS" == 0 ]] || fail "the script refused a successful build: $SCRIPT_ERROR"
[[ "$(wc -l <"$CALLS" | tr -d ' ')" == 1 ]] || fail 'the script did not invoke cargo exactly once'
call=$(cat "$CALLS")
for flag in build --locked '--package pangopup-cli' '--package pangopup-build'; do
    case "$call" in
        *"$flag"*) ;;
        *) fail "the build the script runs is missing '$flag': cargo $call" ;;
    esac
done

# The hole itself. An executable already sitting in target/debug is not evidence
# that it matches the source, so a failed build must not be excused by one.
new_case
printf '#!/bin/sh\nexit 0\n' >"$CASE/target/debug/pangopup"
printf '#!/bin/sh\nexit 0\n' >"$CASE/target/debug/pangopup-build"
chmod +x "$CASE/target/debug/pangopup" "$CASE/target/debug/pangopup-build"
run_script 1 ''
[[ "$SCRIPT_STATUS" != 0 ]] || fail 'the script accepted an executable already in target/debug after the build failed, so a harness would run bytes from an older source'
[[ "$(wc -l <"$CALLS" | tr -d ' ')" == 1 ]] || fail 'the script skipped the build when an executable was already present'

# A build that reports success but leaves no command is refused, and cargo ran
# first -- an absent executable cannot be what made the script stop.
new_case
run_script 0 ''
[[ "$SCRIPT_STATUS" != 0 ]] || fail 'the script accepted a build that produced no executable'
[[ "$(wc -l <"$CALLS" | tr -d ' ')" == 1 ]] || fail 'the script checked for the executable before building it'

# Each command the harnesses run is checked, not just the first.
new_case
run_script 0 'pangopup'
[[ "$SCRIPT_STATUS" != 0 ]] || fail 'the script accepted a build that produced pangopup but no pangopup-build'

# A symlink standing in for the executable is still refused.
new_case
printf '#!/bin/sh\nexit 0\n' >"$CASE/real-pangopup"
chmod +x "$CASE/real-pangopup"
ln -s "$CASE/real-pangopup" "$CASE/target/debug/pangopup"
run_script 0 'pangopup-build'
[[ "$SCRIPT_STATUS" != 0 ]] || fail 'the script accepted a symlink in place of the built executable'
