#!/usr/bin/env bash
set -euo pipefail

# `cargo test` exits 0 when a name filter selects no test. A gate under spec/
# that runs `cargo test` with a filter and reads only the exit code therefore
# passes on nothing the moment the test it names is renamed or deleted. Ticket
# 0050 closed that shape for CI's one feature-gated step. This file closes it
# for the spec gates.
#
# Two things hold it shut, and this file checks both.
#
#   1. No gate under spec/ runs `cargo test` directly. Every one goes through
#      scripts/spec-cargo-test.sh with a floor on the number of tests that must
#      pass, so a gate added later that reads only the exit code is refused
#      here rather than discovered years later.
#   2. The helper reads cargo's count back, refuses a run below its floor, and
#      names both the filter that selected nothing and the spec file that
#      carries the gate, so an operator can find the gate without searching.
#      That is arithmetic, so it is proved here against a fake `cargo`.
#
# What this does not prove: that the gates pass. `make spec` runs them.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
helper="$repository/scripts/spec-cargo-test.sh"
helper_call='../scripts/spec-cargo-test.sh'

fail() { printf 'spec cargo filter evidence: %s\n' "$*" >&2; exit 1; }

# --- 1. every cargo gate under spec/ goes through the helper ----------------
#
# Read the gates out of the spec files rather than listing them here, so this
# check follows spec/ wherever it grows instead of pinning today's inventory.

# Emits one logical line per fenced-block line, with shell line continuations
# joined, as "<file>\t<line>\t<text>".
spec_block_lines() {
    awk '
        FNR == 1 { inblock = 0; pending = ""; pendingline = 0 }
        /^[[:space:]]*```/ { inblock = !inblock; pending = ""; next }
        !inblock { next }
        {
            text = $0
            if (pending != "") { line = pendingline } else { line = FNR }
            joined = pending text
            if (text ~ /\\$/) {
                sub(/\\$/, " ", joined)
                pending = joined
                pendingline = line
                next
            }
            pending = ""
            printf "%s\t%d\t%s\n", FILENAME, line, joined
        }
    ' "$@"
}

shopt -s nullglob
spec_files=("$repository"/spec/*.md)
shopt -u nullglob
(( ${#spec_files[@]} > 0 )) || fail 'found no spec/*.md files to inspect, so this check proved nothing'

all_lines=$(spec_block_lines "${spec_files[@]}")

# A gate that still calls cargo directly is the hole this ticket closes.
# The helper's own path carries no space, so it never matches 'cargo test'. Do
# not exempt a line for mentioning the helper: a gate that calls the helper and
# then also runs cargo directly still exits 0 on an empty filter.
raw=$(printf '%s\n' "$all_lines" | grep -F 'cargo test' || true)
if [[ -n "$raw" ]]; then
    printf 'spec cargo filter evidence: these spec gates run cargo test directly, so each one exits 0 when its name filter selects no test:\n' >&2
    printf '%s\n' "$raw" | sed -E 's#^'"$repository"'/#  #; s/\t/:/; s/\t/: /' >&2
    exit 1
fi

gates=$(printf '%s\n' "$all_lines" | grep -F "$helper_call" || true)
gate_count=$(printf '%s' "$gates" | grep -c . || true)
(( gate_count > 0 )) || fail "inspected ${#spec_files[@]} spec file(s) and found no $helper_call gate, so this check inspected nothing"

# Each gate declares a floor of at least one test and ends in a name filter.
while IFS=$'\t' read -r file line text; do
    [[ -n "${text:-}" ]] || continue
    where="${file#"$repository"/}:$line"
    read -r -a words <<<"${text#*"$helper_call"}"
    # Today's gates end in `>/dev/null 2>&1`, and a converted one may keep it.
    # Cut the argument list at the first redirection or pipeline token, so the
    # filter check reads the last argument the helper receives rather than the
    # last word on the line.
    args=()
    for word in "${words[@]}"; do
        case "$word" in
            '|' | '||' | '&&' | ';' | '&' | '#' | *'>'* | *'<'*) break ;;
        esac
        args+=("$word")
    done
    floor=${args[0]-}
    [[ "$floor" =~ ^[0-9]+$ ]] || fail "$where: the gate's first argument is '${floor:-}', not a number of tests that must pass"
    (( floor >= 1 )) || fail "$where: the gate accepts $floor passing tests, which is the silent-green shape this check exists to refuse"
    (( ${#args[@]} >= 2 )) || fail "$where: the gate passes cargo no arguments after its floor, so it names no test to run"
    filter=${args[${#args[@]}-1]}
    [[ -n "$filter" && "$filter" != -* ]] || fail "$where: the gate's last argument is '$filter', not a test-name filter"
done <<<"$gates"

printf 'inspected %s spec file(s), %s cargo gate(s)\n' "${#spec_files[@]}" "$gate_count"

# --- 2. the helper refuses a run that asserted nothing ----------------------
[[ -f "$helper" ]] || fail "no $helper: the spec gates have nowhere to read cargo's count back from"
[[ -x "$helper" ]] || fail "$helper is not executable"

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT

fake_bin="$fixture/bin"
mkdir "$fake_bin"
cat >"$fake_bin/cargo" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"${CARGO_CALLS:?}"
printf '%s' "${CARGO_OUTPUT-}"
exit "${CARGO_STATUS:?}"
SH
chmod +x "$fake_bin/cargo"

# The helper locates the gate by looking beside itself in the working
# directory, which is where mustmatch runs a spec block from.
gate_dir="$fixture/spec"
mkdir "$gate_dir"
cat >"$gate_dir/pinned-behaviour.md" <<'MD'
# pinned behaviour

```bash
../scripts/spec-cargo-test.sh 12 --locked --quiet --package p --test t real_executable_
```
MD

CALLS="$fixture/cargo-calls"
run_helper() {
    local cargo_status=$1 output=$2
    shift 2
    : >"$CALLS"
    set +e
    HELPER_OUTPUT=$(cd "$gate_dir" && PATH="$fake_bin:$PATH" CARGO_CALLS="$CALLS" \
        CARGO_STATUS="$cargo_status" CARGO_OUTPUT="$output" bash "$helper" "$@" 2>&1)
    HELPER_STATUS=$?
    set -e
}

summary() { printf 'test result: ok. %s passed; 0 failed; 0 ignored; 0 measured; %s filtered out; finished in 0.01s\n' "$1" "$2"; }
# An assignment's right-hand side is a quoted context, so the pair must be split
# here: `summary ${pair/:/ }` would hand the whole `12 3` to one parameter and
# emit `12 3 passed; ...;  filtered out`, which no correct helper can read.
runs() {
    local out= pair
    for pair in "$@"; do out+=$(summary "${pair%%:*}" "${pair#*:}")$'\n'; done
    printf '%s' "$out"
}

full=$(runs 12:3)
# The measured silent green: a renamed filter printed exactly this shape and
# `cargo test` exited 0.
empty=$(runs 0:15)

gate_args=(12 --locked --quiet --package p --test t real_executable_)

run_helper 0 "$full" "${gate_args[@]}"
[[ "$HELPER_STATUS" == 0 ]] || fail "the helper rejected a run that passed 12 tests against a floor of 12: $HELPER_OUTPUT"
expected_call='test --locked --quiet --package p --test t real_executable_'
[[ "$(cat "$CALLS")" == "$expected_call" ]] || fail "the helper ran 'cargo $(cat "$CALLS")' instead of 'cargo $expected_call'"

run_helper 0 "$empty" "${gate_args[@]}"
[[ "$HELPER_STATUS" != 0 ]] || fail "the helper accepted a run that selected no test, which is the whole hole: $HELPER_OUTPUT"
case "$HELPER_OUTPUT" in
    *real_executable_*) ;;
    *) fail "the refusal does not name the filter that selected nothing, so an operator cannot tell which gate emptied: $HELPER_OUTPUT" ;;
esac
case "$HELPER_OUTPUT" in
    *pinned-behaviour.md*) ;;
    *) fail "the refusal does not name the spec file carrying the gate, so an operator has to search for it: $HELPER_OUTPUT" ;;
esac

# A gate whose coverage shrinks below its floor is the same failure as an empty
# one: the gate no longer covers what it was written to cover.
run_helper 0 "$(runs 4:11)" "${gate_args[@]}"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted 4 passing tests against a floor of 12, so a gate may quietly shrink'
case "$HELPER_OUTPUT" in
    *real_executable_*) ;;
    *) fail "the shrink refusal does not name the filter: $HELPER_OUTPUT" ;;
esac

# A filter with no target selector runs several cargo targets and prints one
# summary line each. The floor is on the whole gate, not on one binary.
run_helper 0 "$(runs 5:2 7:9)" "${gate_args[@]}"
[[ "$HELPER_STATUS" == 0 ]] || fail "the helper did not total 5 and 7 passing tests across two cargo targets against a floor of 12: $HELPER_OUTPUT"

run_helper 101 "$full" "${gate_args[@]}"
[[ "$HELPER_STATUS" == 101 ]] || fail "the helper reported $HELPER_STATUS instead of cargo's own 101"

run_helper 0 'no test result line at all' "${gate_args[@]}"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted output carrying no summary line, so a changed cargo report would read as an unchecked gate'
