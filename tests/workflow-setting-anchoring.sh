#!/usr/bin/env bash
set -euo pipefail

# Ticket 0053 gave the workflow gates `require_workflow_command`, so a gate can
# no longer read a YAML comment as proof that a step runs. Five of the six lines
# it named moved across. Two did not: the `env:` keys of the ARM64
# cross-compile step in `.github/workflows/ci.yml`, still held by a `case`
# substring test over a joined slice of the file. Commenting either key out
# leaves the gate green.
#
# The two are settings rather than commands, and the answer has to read as one.
# A refusal that says a command does not stand in code, over a compiler
# environment key, sends a reader looking for a step that is not missing.
#
# The contract this file pins:
#
#   `tests/support/workflow-commands.sh` defines `require_workflow_setting`
#   beside `require_workflow_command`. It is sourced -- sourcing it runs
#   nothing.
#
#       require_workflow_setting <workflow-file> <setting> [<step name>]
#
#   Argument 1 is the path of the workflow file to read. Argument 2 is the
#   setting text that must stand there. Argument 3, when the caller gives one,
#   narrows the search to the step of that name; a step name the workflow does
#   not hold refuses rather than passing over the empty range it selects.
#
#   Arguments 2 and 3 are written in the calling harness as single-quoted
#   literals, and argument 1 either ends in the workflow's file name or is a
#   variable the same harness assigns such a path to. That is what lets
#   section 3 enumerate the guarded settings out of the harness itself rather
#   than out of a hand-written list that goes stale.
#
#   A setting stands in code when a line of the workflow carries it before any
#   `#`, so a full-line comment and a trailing comment beside a no-op are both
#   refused while a comment repeating a line that still holds is not. The
#   setting text is matched literally, so it never exempts more than the
#   setting it names. On refusal the message names the setting and the workflow
#   file, calls it a setting, and the status is non-zero.
#
# What this does not prove: that the workflow is valid YAML, that the step the
# setting sits in is reached, or that the setting has any effect on the build.
#
# Nothing here writes to a workflow file. Every comment-out happens on a
# throwaway copy under a temporary directory.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
support="$repository/tests/support/workflow-commands.sh"
harness="$repository/tests/ci-platform-support.sh"

fail() { printf 'workflow setting anchoring: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

[[ -f "$support" ]] \
    || fail 'no tests/support/workflow-commands.sh: nothing gives the workflow gates a way to read a step rather than a comment about one'
[[ -f "$harness" ]] \
    || fail 'tests/ci-platform-support.sh is gone, so this scan is checking the wrong file'
grep -qE '^[^#]*support/workflow-commands\.sh' "$harness" \
    || fail 'tests/ci-platform-support.sh does not source tests/support/workflow-commands.sh'

( set -euo pipefail; source "$support"; declare -F require_workflow_setting >/dev/null ) \
    || fail 'tests/support/workflow-commands.sh does not define require_workflow_setting, so a workflow setting is still proved by searching the file for its text'

# --- 1. the mechanism ------------------------------------------------------
#
# Proved against a fixture workflow, so these cases stay readable and never
# depend on what `ci.yml` happens to say today.
fixture="$work/fixture.yml"
cat >"$fixture" <<'YML'
name: fixture
jobs:
  gate:
    steps:
      - name: Cross compile
        env:
          CC_target: cross-gcc-4.9
        run: cargo check
      - name: Ship
        env:
          SHIP_MODE: dry-run
        run: scripts/ship.sh
YML

commented="$work/commented.yml"
sed 's/^          CC_target: cross-gcc-4[.]9$/#          CC_target: cross-gcc-4.9/' "$fixture" >"$commented"

trailing="$work/trailing.yml"
sed 's/^          CC_target: cross-gcc-4[.]9$/          OTHER: value # CC_target: cross-gcc-4.9/' "$fixture" >"$trailing"

beside="$work/beside.yml"
sed 's/^          CC_target: cross-gcc-4[.]9$/          # CC_target: cross-gcc-4.9\n          CC_target: cross-gcc-4.9/' "$fixture" >"$beside"

# A near miss for a setting carrying a regex metacharacter. The `.` of the
# compiler version, read as a pattern rather than as text, matches this line,
# which exempts a compiler the gate never named. A near miss that changed only
# an ordinary character would be refused by either reading, so it would prove
# nothing about which one the mechanism does.
near="$work/near.yml"
sed 's/^          CC_target: cross-gcc-4[.]9$/          CC_target: cross-gcc-4X9/' "$fixture" >"$near"

setting_status=0
setting_error=
run_setting() {
    set +e
    ( set -euo pipefail; source "$support"; require_workflow_setting "$@" ) \
        >"$work/out" 2>"$work/err"
    setting_status=$?
    set -e
    setting_error=$(cat "$work/err")
}

run_setting "$fixture" 'CC_target: cross-gcc-4.9'
[[ "$setting_status" == 0 ]] \
    || fail "the mechanism refused a setting that stands in code: $setting_error"

run_setting "$fixture" 'CC_other: cross-gcc-4.9'
[[ "$setting_status" != 0 ]] \
    || fail 'the mechanism accepted a setting the workflow does not hold at all'

run_setting "$commented" 'CC_target: cross-gcc-4.9'
[[ "$setting_status" != 0 ]] \
    || fail 'the mechanism accepted a setting that only appears as a YAML comment'
[[ "$setting_error" == *'CC_target: cross-gcc-4.9'* ]] \
    || fail "the refusal does not name the setting that stopped standing: $setting_error"
[[ "$setting_error" == *'commented.yml'* ]] \
    || fail "the refusal does not name the workflow file the setting is missing from: $setting_error"
# The wording is the half ticket 0053's mechanism gets wrong over these two
# lines. A reader told a command does not stand in code goes looking for a step
# that is not missing.
[[ "$setting_error" == *setting* ]] \
    || fail "the refusal for a setting does not call it a setting: $setting_error"
[[ "$setting_error" != *command* ]] \
    || fail "the refusal for a setting describes it as a command: $setting_error"

run_setting "$trailing" 'CC_target: cross-gcc-4.9'
[[ "$setting_status" != 0 ]] \
    || fail 'the mechanism accepted a setting that only appears as a trailing comment'

run_setting "$beside" 'CC_target: cross-gcc-4.9'
[[ "$setting_status" == 0 ]] \
    || fail "the mechanism refused a setting that stands in code beside a comment repeating it: $setting_error"

run_setting "$near" 'CC_target: cross-gcc-4.9'
[[ "$setting_status" != 0 ]] \
    || fail 'the mechanism read the setting as a pattern rather than as text, so it accepts a value the gate never named'

# --- 2. a narrowed search is answered by its own step ----------------------
run_setting "$fixture" 'CC_target: cross-gcc-4.9' 'Cross compile'
[[ "$setting_status" == 0 ]] \
    || fail "the mechanism refused a setting that stands in the step it was narrowed to: $setting_error"

run_setting "$fixture" 'CC_target: cross-gcc-4.9' 'Ship'
[[ "$setting_status" != 0 ]] \
    || fail 'a setting standing in a different step satisfied a narrowed search, so the narrowing reads the whole file'

run_setting "$fixture" 'SHIP_MODE: dry-run' 'Cross compile'
[[ "$setting_status" != 0 ]] \
    || fail 'a narrowed search was answered by a later step, so the narrowing never ends'

run_setting "$fixture" 'CC_target: cross-gcc-4.9' 'Recross compile'
[[ "$setting_status" != 0 ]] \
    || fail 'a scope naming a step the workflow does not hold passed, so a renamed step leaves every guard narrowed to it holding over nothing'
[[ "$setting_error" == *'Recross compile'* ]] \
    || fail "the refusal for an absent step does not name the step that was looked for: $setting_error"

# --- 3. every guarded setting, commented out in turn -----------------------
#
# The entries are read out of the harness, so a setting that stops being
# guarded stops being proved here too and section 4 notices.
ledger="$work/ledger"
unparsed="$work/unparsed"
unit=$(printf '\037')
awk -v unparsed="$unparsed" '
    /^[^#]*require_workflow_setting/ {
        quote = sprintf("%c", 39)
        unit = sprintf("%c", 31)
        n = split($0, field, quote)
        if (n < 3) { printf "%d: %s\n", NR, $0 >> unparsed; next }
        printf "%s%s%s%s%s\n", field[1], unit, field[2], unit, (n >= 5) ? field[4] : ""
    }
' "$harness" >"$ledger"

if [[ -s "$unparsed" ]]; then
    fail "a require_workflow_setting call is written in a form this scan cannot read, so the setting it guards would never be commented out here: $(tr '\n' ' ' <"$unparsed")"
fi

guarded=$(wc -l <"$ledger" | tr -d ' ')
[[ "$guarded" -gt 0 ]] \
    || fail 'tests/ci-platform-support.sh guards no workflow setting through the shared mechanism, so its ARM64 compiler keys are still proved by searching the file for their text'

resolve_workflow() {
    local head=$1 name variable
    name=$({ grep -oE '[A-Za-z0-9._-]+\.yml' <<<"$head" || true; } | tail -n 1)
    if [[ -z "$name" ]]; then
        variable=$({ grep -oE '[$]\{?[A-Za-z_][A-Za-z0-9_]*' <<<"$head" || true; } | tail -n 1 | tr -d '${')
        [[ -n "$variable" ]] || return 1
        name=$({ grep -E "^[[:space:]]*$variable=" "$harness" || true; } \
            | { grep -oE '[A-Za-z0-9._-]+\.yml' || true; } | tail -n 1)
    fi
    [[ -n "$name" ]] || return 1
    printf '%s\n' "$name"
}

while IFS="$unit" read -r head setting scope; do
    workflow=$(resolve_workflow "$head") \
        || fail "tests/ci-platform-support.sh guards '$setting' without naming a workflow file the scan can resolve"
    [[ -n "$setting" ]] \
        || fail 'tests/ci-platform-support.sh has a require_workflow_setting call with an empty setting'
    real="$repository/.github/workflows/$workflow"
    [[ -f "$real" ]] \
        || fail "tests/ci-platform-support.sh guards '$setting' in $workflow, which is not a workflow file in this checkout"

    run_setting "$real" "$setting" ${scope:+"$scope"}
    [[ "$setting_status" == 0 ]] \
        || fail "tests/ci-platform-support.sh guards '$setting' in $workflow but the mechanism refuses the shipped workflow: $setting_error"

    copy="$work/commented-$workflow"
    awk -v setting="$setting" '
        {
            line = $0
            before = line
            sub(/#.*/, "", before)
            if (index(before, setting) > 0) { print "#" line; hit = 1 }
            else { print line }
        }
        END { if (!hit) { exit 3 } }
    ' "$real" >"$copy" \
        || fail "tests/ci-platform-support.sh guards '$setting' in $workflow, but no line of $workflow carries it in code"

    run_setting "$copy" "$setting" ${scope:+"$scope"}
    [[ "$setting_status" != 0 ]] \
        || fail "commenting out '$setting' in $workflow leaves the gate green, so the setting can be removed without tests/ci-platform-support.sh noticing"
    [[ "$setting_error" == *"$setting"* ]] \
        || fail "the refusal for '$setting' does not name the setting that stopped standing: $setting_error"
    [[ "$setting_error" == *"$workflow"* ]] \
        || fail "the refusal for '$setting' does not name $workflow: $setting_error"
done <"$ledger"

# --- 4. the two ARM64 keys are among them ----------------------------------
#
# These are the two the ticket is about: the compiler the bundled C dependency
# is built with and the linker the Rust target is linked with, both read from
# the ARM64 cross-compile step's `env:` block.
for required in \
    'CC_aarch64_unknown_linux_gnu: aarch64-linux-gnu-gcc' \
    'CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc'; do
    found=0
    while IFS="$unit" read -r _ setting _; do
        [[ "$setting" == "$required" ]] && found=1
    done <"$ledger"
    [[ "$found" == 1 ]] \
        || fail "no gate anchors '$required' any more: the ARM64 cross-compile can lose it behind a comment"
done

printf 'workflow setting anchoring: %s guarded setting(s) refuse a comment in place of the setting\n' "$guarded"
