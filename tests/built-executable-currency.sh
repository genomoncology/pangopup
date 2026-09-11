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
# instead of passing.
#
# Only code counts, and a whole-line comment is the only commentary these
# patterns recognise. Anchoring them at `^[^#]*` instead would hide a use from
# every code line carrying an earlier `#`, and
# `trimmed=${bin#$PWD/}; "$repo/target/debug/pangopup" --version` is ordinary
# shell -- reading it as commentary is the one direction this gate must never
# fail in. `code_lines` drops the comment lines instead, and the fixture below
# holds both directions.
use_pattern='target/debug/[A-Za-z]'

# A mention of the build script is not a call of it. The path stands in a
# `printf` argument in three lines of `tests/shell-spawn-cache-isolation.sh`,
# which write it into a fixture they are about to read, and a text match over
# the line reads the lowest of those as the build -- so a harness whose real
# build stands below its first use, or which builds nothing at all, is
# accepted. This pattern asks where the path stands instead: as the command
# word, as what `bash`, `sh`, `source` or `.` is given to run, or as the value
# of an assignment run through its variable afterwards. Each of those hands
# somebody a build; a mention hands nobody one.
#
# The use side deliberately keeps its plain text match. A mention of the built
# executable counts as a use, because demanding a build from a harness that
# only names the path refuses a harness that runs nothing -- the direction this
# gate is allowed to be wrong in -- while excusing one is the direction it
# cannot be.
build_pattern='^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=)?(\$\()?((bash|sh|source|\.)[[:space:]]+)?"?[^"[:space:]]*scripts/require-built-commands\.sh'

# The line numbers in $1 matching the extended pattern $2, lowest first. A
# whole-line comment is commentary rather than code and never matches.
code_lines() {
    { grep -nE -- "$2" "$1" || true; } \
        | { grep -vE '^[0-9]+:[[:space:]]*#' || true; } \
        | cut -d: -f1
}

# The first such line, or empty.
first_line() {
    code_lines "$1" "$2" | head -n 1
}

# Refuse the tree of shell harnesses in directory $1. Prints the number of
# harnesses that reach into target/debug; refuses when one of them uses an
# executable it did not build first.
examine_harnesses() {
    local directory=$1 harness name use_line build_line guarded=0 scanned=0
    for harness in "$directory"/*.sh; do
        [[ -f "$harness" ]] || continue
        name=$(basename "$harness")
        scanned=$((scanned + 1))
        [[ "$name" != "$self" ]] || continue
        use_line=$(first_line "$harness" "$use_pattern")
        [[ -n "$use_line" ]] || continue
        guarded=$((guarded + 1))
        build_line=$(first_line "$harness" "$build_pattern")
        if [[ -z "$build_line" ]]; then
            printf 'tests/%s uses an executable from target/debug at line %s without building it first, so a standalone run compares against whatever the last build left behind\n' \
                "$name" "$use_line" >&2
            return 1
        fi
        if [[ "$build_line" -ge "$use_line" ]]; then
            printf 'tests/%s builds at line %s but first uses target/debug at line %s: a missing or stale build has to be answered before the first use, not after\n' \
                "$name" "$build_line" "$use_line" >&2
            return 1
        fi
    done
    if (( scanned == 0 )); then
        printf 'found no shell harness under %s, so this scan inspected nothing\n' "$directory" >&2
        return 1
    fi
    printf '%s\n' "$guarded"
}

# The scan reads a code line as code and a comment as commentary. Proved
# against a fixture first, because both directions are silent failures: a
# pattern that read a `#`-carrying code line as a comment would excuse the
# harness this gate exists to hold, and one that read a comment as a use would
# demand a build from a harness that runs nothing.
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
fixture_tests="$fixture/tests"
mkdir "$fixture_tests"

{
    printf '#!/usr/bin/env bash\n'
    printf '# a comment naming %s runs nothing\n' 'target/debug/pangopup'
    printf 'printf ok\n'
} >"$fixture_tests/mentions.sh"

fixture_guarded=$(examine_harnesses "$fixture_tests") \
    || fail 'the scan refused a fixture holding one harness that names target/debug only in a comment'
[[ "$fixture_guarded" == 0 ]] \
    || fail "the scan read a whole-line comment naming target/debug as a harness running the built executable, so it would demand a build from a harness that runs nothing: counted $fixture_guarded"

{
    printf '#!/usr/bin/env bash\n'
    printf 'bin=/somewhere/pangopup\n'
    printf 'trimmed=${bin#$PWD/}; "$repo/%s" --version\n' 'target/debug/pangopup'
} >"$fixture_tests/hash-then-use.sh"

if fixture_report=$(examine_harnesses "$fixture_tests" 2>&1); then
    fail "the scan accepted a harness whose use line carries an earlier '#' and which builds nothing, so a code line spelling a parameter expansion is read as commentary: $fixture_report"
fi
case "$fixture_report" in
    *hash-then-use.sh*) ;;
    *) fail "the refusal does not name the harness it refused: $fixture_report" ;;
esac

# The repository itself.
guarded=$(examine_harnesses "$repository/tests") || exit 1

for required in production-release-qualification.sh executable-delivery.sh; do
    [[ -n "$(first_line "$repository/tests/$required" "$use_pattern")" ]] \
        || fail "tests/$required no longer reaches into target/debug, so this scan is checking the wrong files"
done

# Measured over this tree on 2026-09-11: four harnesses reach into
# target/debug in code -- executable-delivery.sh, inherited-cache-variables.sh,
# model-cache-limit-inheritance.sh and production-release-qualification.sh.
# The floor sits on four rather than under it. The two named above already have
# to be in the set, so a floor of two could never fail: it was satisfied by the
# loop that ran before it. On the measured number, a harness that stops
# reaching into target/debug has to move this line, and moving it is a change
# someone reads.
harness_floor=4
[[ "$guarded" -ge "$harness_floor" ]] \
    || fail "the scan found $guarded harness(es) using target/debug against a floor of $harness_floor, so it matched less than the repository holds"

printf 'built executable currency: %s harness(es) reaching into target/debug, each building first\n' "$guarded"

# --- 2. the script builds first, then refuses ------------------------------
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
