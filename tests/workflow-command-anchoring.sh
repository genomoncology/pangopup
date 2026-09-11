#!/usr/bin/env bash
set -euo pipefail

# A gate that proves a workflow runs a command by searching the workflow file
# for that command's text reads a YAML comment as happily as a step. The step
# can then be deleted or replaced with `run: "true"` while the original line
# stays above it behind a `#`, and the gate stays green. The ARM64
# cross-compile in `.github/workflows/ci.yml` and the release build in
# `.github/workflows/package-linux.yml` are both guarded that way today, by
# `tests/ci-platform-support.sh` and `tests/executable-delivery.sh`.
#
# Ticket 0050 closed the same shape inside `tests/built-executable-currency.sh`
# by anchoring its discovery patterns so a match has to stand in code. This
# file holds that answer for the two workflow gates, through one shared
# mechanism so the two cannot drift apart again.
#
# The contract this file pins:
#
#   `tests/support/workflow-commands.sh` defines `require_workflow_command`.
#   It is sourced -- sourcing it runs nothing.
#
#       require_workflow_command <workflow-file> <command> [<scope>]
#
#   Argument 1 is the path of the workflow file to read. Argument 2 is the
#   command text that must run there. Argument 3, when the caller gives one, is
#   whatever the caller uses to narrow where in the file the command may stand;
#   this file passes it through unchanged and makes no claim about it.
#
#   Arguments 2 and 3 are written in the calling harness as single-quoted
#   literals, and argument 1 either ends in the workflow's file name or is a
#   variable the same harness assigns such a path to. That is what lets the
#   scan below enumerate the guarded commands out of the harnesses themselves
#   rather than out of a hand-written list that goes stale.
#
#   A command stands in code when the workflow holds a line matching
#   `^[^#]*<command>` -- no `#` before the match. That is ticket 0050's rule,
#   so a full-line comment and a trailing comment on a live line are both
#   refused. The command text is matched literally: a `.` in a version number
#   matches a `.` and nothing else, so the anchoring never exempts more than the
#   command it names. On refusal the message names the command and the workflow
#   file and the status is non-zero.
#
# What this does not prove: that the workflow is valid YAML, that the step the
# command sits in is reached, or that the command does what its name says.
#
# Nothing here writes to a workflow file. Every comment-out happens on a
# throwaway copy under a temporary directory, so a failure part way through
# cannot leave a shipped workflow edited.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
support="$repository/tests/support/workflow-commands.sh"
harnesses=(ci-platform-support.sh executable-delivery.sh)

fail() { printf 'workflow command anchoring: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

[[ -f "$support" ]] \
    || fail "no tests/support/workflow-commands.sh: nothing makes the two workflow gates read a step rather than a comment about one"

# --- 1. one mechanism, shared by both harnesses ----------------------------
#
# Named rather than discovered, because these two are the gates the guarantee
# is about. A third harness adopting the mechanism is welcome; either of these
# two dropping it is the drift this file exists to stop.
for name in "${harnesses[@]}"; do
    harness="$repository/tests/$name"
    [[ -f "$harness" ]] || fail "tests/$name is gone, so this scan is checking the wrong files"
    grep -qE '^[^#]*support/workflow-commands\.sh' "$harness" \
        || fail "tests/$name does not source tests/support/workflow-commands.sh, so the two workflow gates can answer a commented-out command differently"
done

# --- 2. the mechanism itself -----------------------------------------------
#
# Proved against a fixture workflow, so these cases stay readable and never
# depend on what the real workflows happen to say today.
fixture="$work/fixture.yml"
cat >"$fixture" <<'YML'
name: fixture
jobs:
  gate:
    steps:
      - name: Build the thing
        run: |
          cargo build --locked --release --package pangopup-cli
      - name: Ship the thing
        run: scripts/ship.sh
YML

# The same file with the build replaced by a no-op and the original line left
# above it behind a `#`. This is the dodge the mechanism is for.
commented="$work/commented.yml"
cat >"$commented" <<'YML'
name: fixture
jobs:
  gate:
    steps:
      - name: Build the thing
        run: |
          # cargo build --locked --release --package pangopup-cli
          true
      - name: Ship the thing
        run: scripts/ship.sh
YML

# The same dodge written on one line instead of two.
trailing="$work/trailing.yml"
cat >"$trailing" <<'YML'
name: fixture
jobs:
  gate:
    steps:
      - name: Build the thing
        run: |
          true # cargo build --locked --release --package pangopup-cli
      - name: Ship the thing
        run: scripts/ship.sh
YML

# A comment repeating the line, beside the line that still runs.
beside="$work/beside.yml"
cat >"$beside" <<'YML'
name: fixture
jobs:
  gate:
    steps:
      - name: Build the thing
        run: |
          # cargo build --locked --release --package pangopup-cli
          cargo build --locked --release --package pangopup-cli
      - name: Ship the thing
        run: scripts/ship.sh
YML

# A near miss for a command carrying regex metacharacters. `0.5.9` read as a
# pattern rather than as text matches this line, which exempts a version the
# gate never named.
near="$work/near.yml"
cat >"$near" <<'YML'
name: fixture
jobs:
  gate:
    steps:
      - name: Install the SBOM tool
        run: cargo install --locked --version 0X5Y9 cargo-cyclonedx
YML

build='cargo build --locked --release --package pangopup-cli'

require_status=0
require_error=
run_require() {
    set +e
    ( set -euo pipefail; source "$support"; require_workflow_command "$@" ) \
        >"$work/out" 2>"$work/err"
    require_status=$?
    set -e
    require_error=$(cat "$work/err")
}

run_require "$fixture" "$build"
[[ "$require_status" == 0 ]] \
    || fail "the mechanism refused a command that stands in code: $require_error"

run_require "$fixture" 'cargo build --locked --offline'
[[ "$require_status" != 0 ]] \
    || fail 'the mechanism accepted a command the workflow does not hold at all'

run_require "$commented" "$build"
[[ "$require_status" != 0 ]] \
    || fail 'the mechanism accepted a command that only appears as a YAML comment'
[[ "$require_error" == *"$build"* ]] \
    || fail "the refusal does not name the command that stopped running: $require_error"
[[ "$require_error" == *'commented.yml'* ]] \
    || fail "the refusal does not name the workflow file the command is missing from: $require_error"

run_require "$trailing" "$build"
[[ "$require_status" != 0 ]] \
    || fail 'the mechanism accepted a command that only appears as a trailing comment beside a no-op'

run_require "$beside" "$build"
[[ "$require_status" == 0 ]] \
    || fail "the mechanism refused a command that stands in code beside a comment repeating it: $require_error"

run_require "$near" 'cargo install --locked --version 0.5.9 cargo-cyclonedx'
[[ "$require_status" != 0 ]] \
    || fail 'the mechanism read the command as a pattern rather than as text, so it accepts a line the gate never named'

# --- 3. every guarded command, commented out in turn -----------------------
#
# The pairs are read out of the harnesses, so a command that stops being
# guarded stops being proved here too and the required set below notices.
ledger="$work/ledger"
unparsed="$work/unparsed"
unit=$(printf '\037')
: >"$ledger"
: >"$unparsed"
for name in "${harnesses[@]}"; do
    awk -v harness="$name" -v unparsed="$unparsed" '
        /^[^#]*require_workflow_command/ {
            quote = sprintf("%c", 39)
            unit = sprintf("%c", 31)
            n = split($0, field, quote)
            if (n < 3) { printf "%s:%d: %s\n", harness, NR, $0 >> unparsed; next }
            printf "%s%s%s%s%s%s%s\n", harness, unit, field[1], unit, field[2], unit, (n >= 5) ? field[4] : ""
        }
    ' "$repository/tests/$name" >>"$ledger"
done

# A call this scan cannot decompose is the way a command goes unproved while the
# run still reports a total: the scan would skip it and say nothing. Refuse
# instead, so the guarded set is the set this file proves.
if [[ -s "$unparsed" ]]; then
    fail "a require_workflow_command call is written in a form this scan cannot read, so the command it guards would never be commented out here: $(tr '\n' ' ' <"$unparsed")"
fi

# The workflow the call reads, named on the call line or through a variable the
# same harness assigns. Resolved rather than assumed, so the file this scan
# comments out is the file the gate reads.
resolve_workflow() {
    local harness=$1 head=$2 name variable
    name=$({ grep -oE '[A-Za-z0-9._-]+\.yml' <<<"$head" || true; } | tail -n 1)
    if [[ -z "$name" ]]; then
        variable=$({ grep -oE '[$]\{?[A-Za-z_][A-Za-z0-9_]*' <<<"$head" || true; } | tail -n 1 | tr -d '${')
        [[ -n "$variable" ]] || return 1
        name=$({ grep -E "^[[:space:]]*$variable=" "$repository/tests/$harness" || true; } \
            | { grep -oE '[A-Za-z0-9._-]+\.yml' || true; } | tail -n 1)
    fi
    [[ -n "$name" ]] || return 1
    printf '%s\n' "$name"
}

pairs=$(wc -l <"$ledger" | tr -d ' ')
[[ "$pairs" -gt 0 ]] \
    || fail 'the scan found no guarded commands in either harness, so it proved nothing'

seen_ci_platform_support=0
seen_executable_delivery=0
while IFS="$unit" read -r harness head command scope; do
    workflow=$(resolve_workflow "$harness" "$head") \
        || fail "tests/$harness guards '$command' without naming a workflow file the scan can resolve, so it cannot tell which file the gate reads"
    [[ -n "$command" ]] \
        || fail "tests/$harness has a require_workflow_command call with an empty command"
    real="$repository/.github/workflows/$workflow"
    [[ -f "$real" ]] \
        || fail "tests/$harness guards '$command' in $workflow, which is not a workflow file in this checkout"
    case "$harness" in
        ci-platform-support.sh) seen_ci_platform_support=1 ;;
        executable-delivery.sh) seen_executable_delivery=1 ;;
    esac

    # The workflow as it ships is accepted, so a refusal below is the
    # comment-out and not a scan that refuses everything.
    run_require "$real" "$command" ${scope:+"$scope"}
    [[ "$require_status" == 0 ]] \
        || fail "tests/$harness guards '$command' in $workflow but the mechanism refuses the shipped workflow: $require_error"

    # Comment out every standing occurrence. The line count does not change, so
    # a caller that narrows by line number still narrows the same way.
    copy="$work/commented-$workflow"
    awk -v command="$command" '
        {
            line = $0
            before = line
            sub(/#.*/, "", before)
            if (index(before, command) > 0) { print "#" line; hit = 1 }
            else { print line }
        }
        END { if (!hit) { exit 3 } }
    ' "$real" >"$copy" \
        || fail "tests/$harness guards '$command' in $workflow, but no line of $workflow carries it in code, so the comment-out proved nothing"

    run_require "$copy" "$command" ${scope:+"$scope"}
    [[ "$require_status" != 0 ]] \
        || fail "commenting out '$command' in $workflow leaves the gate green, so the step it stands for can be deleted without tests/$harness noticing"
    [[ "$require_error" == *"$command"* ]] \
        || fail "the refusal for '$command' in $workflow does not name the command that stopped running: $require_error"
    [[ "$require_error" == *"$workflow"* ]] \
        || fail "the refusal for '$command' does not name $workflow, so a reader cannot tell which workflow stopped running it: $require_error"
done <"$ledger"

[[ "$seen_ci_platform_support" == 1 ]] \
    || fail 'tests/ci-platform-support.sh guards no workflow command through the shared mechanism'
[[ "$seen_executable_delivery" == 1 ]] \
    || fail 'tests/executable-delivery.sh guards no workflow command through the shared mechanism'

# --- 4. the commands that decide what ships are among them -----------------
#
# The ARM64 cross-compile and the release build are the two the gates exist
# for. The rest of this list is every other command those gates claim the
# workflows run today, counted out of `tests/ci-platform-support.sh` and the
# `package-linux.yml` block of `tests/executable-delivery.sh` on 2026-09-10.
# Each has to keep being guarded, so the anchoring cannot be bought by guarding
# less than the gates guard now.
for fragment in \
    'sudo apt-get update' \
    'sudo apt-get install --yes gcc-aarch64-linux-gnu' \
    'cargo check --locked --target aarch64-unknown-linux-gnu --package pangopup-cli' \
    'scripts/run-linux-tests-with-public-failure.sh' \
    'git merge-base --is-ancestor' \
    'git diff --cached --quiet' \
    'cargo fetch --locked' \
    'cargo build --locked --release --package pangopup-cli' \
    'cargo install --locked --version 0.5.9 cargo-cyclonedx' \
    'CARGO_NET_OFFLINE=true cargo cyclonedx --manifest-path' \
    'for round in one two' \
    'scripts/qualify-linux-release.sh' \
    'scripts/smoke-linux-release.sh'; do
    found=0
    while IFS="$unit" read -r _ _ command _; do
        case "$command" in
            *"$fragment"*) found=1 ;;
        esac
    done <"$ledger"
    [[ "$found" == 1 ]] \
        || fail "no gate anchors '$fragment' any more: a workflow the release depends on can stop running it behind a comment"
done

printf 'workflow command anchoring: %s guarded command(s) refuse a comment in place of the step\n' "$pairs"
