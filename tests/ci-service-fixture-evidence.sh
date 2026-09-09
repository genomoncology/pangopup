#!/usr/bin/env bash
set -euo pipefail

# CI runs one step that no local gate reproduces: the feature-gated service
# lifecycle. `make test` builds no `service-test-fixtures`, so that step is the
# only place a real `pangopup serve` runs against a runtime installed from
# repository fixtures.
#
# It used to be a bare `cargo test ... --test http_service_lifecycle
# installed_success`. The trailing word is a name filter, and `cargo test` exits
# 0 when a filter matches nothing: renaming the module took the step from twelve
# assertions to zero and the job stayed green. Two things hold that shut now,
# and this file checks both.
#
#   1. The filter must name a module that exists, gated on the feature the step
#      passes. That is a source coupling, so it is checked here, statically.
#   2. The run must report at least one passed test. That is a runtime fact
#      only CI can observe, so `scripts/run-service-fixture-tests.sh` observes
#      it there and this file proves the script's arithmetic here, against a
#      fake `cargo` -- including the exact `0 passed; 15 filtered out` shape the
#      real command printed when the filter matched nothing.
#
# What this does not prove: that the twelve tests pass. Only CI runs them.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
script="$repository/scripts/run-service-fixture-tests.sh"
workflow="$repository/.github/workflows/ci.yml"

fail() { printf 'ci service fixture evidence: %s\n' "$*" >&2; exit 1; }

[[ -f "$script" ]] || fail "no $script: CI still runs a cargo filter that reports success when it matches nothing"
[[ -x "$script" ]] || fail "$script is not executable"

# --- 1. the step's own definition names a target, a feature and a filter -----
#
# Read all three back out of the script rather than repeating them here, so this
# file follows the step wherever it points instead of pinning yesterday's names.
read_setting() {
    local name=$1 value count
    value=$(sed -nE "s/^$name=([A-Za-z0-9_-]+)\$/\1/p" "$script")
    count=$(printf '%s' "$value" | grep -c . || true)
    [[ "$count" == 1 ]] || fail "expected exactly one '$name=' line in $script, found $count"
    printf '%s' "$value"
}

target=$(read_setting target)
feature=$(read_setting feature)
filter=$(read_setting filter)

source_file="$repository/crates/pangopup-cli/tests/$target.rs"
[[ -f "$source_file" ]] || fail "the step names test target '$target', but $source_file does not exist"

# The filter has to be a module in that target, or the step selects nothing.
gated_module=$(grep -B1 -E "^mod $filter \{\$" "$source_file" || true)
[[ -n "$gated_module" ]] || fail "the step filters on '$filter', which is not a module in $source_file, so the filter selects nothing and the step still exits 0"
case "$gated_module" in
    *"#[cfg(feature = \"$feature\")]"*) ;;
    *) fail "module '$filter' is not gated on feature '$feature', so the step's --features flag no longer selects it" ;;
esac

grep -Fq "$feature = [" "$repository/crates/pangopup-cli/Cargo.toml" \
    || fail "feature '$feature' is not declared in crates/pangopup-cli/Cargo.toml"

# --- 2. both CI jobs run the script, and neither runs the bare cargo line ----
invocations=$(grep -Ec '^ *(run: )?scripts/run-service-fixture-tests\.sh$' "$workflow" || true)
[[ "$invocations" == 2 ]] || fail "expected the Linux and macOS jobs to run scripts/run-service-fixture-tests.sh, found $invocations invocation(s) in $workflow"

bare=$(grep -Ec "cargo test .*--test $target" "$workflow" || true)
[[ "$bare" == 0 ]] || fail "$workflow still runs a bare 'cargo test --test $target' line, which exits 0 when its name filter matches nothing"

# --- 3. the script refuses a run that asserted nothing ----------------------
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

CALLS="$fixture/cargo-calls"
run_script() {
    local cargo_status=$1 output=$2
    : >"$CALLS"
    set +e
    SCRIPT_OUTPUT=$(PATH="$fake_bin:$PATH" CARGO_CALLS="$CALLS" \
        CARGO_STATUS="$cargo_status" CARGO_OUTPUT="$output" bash "$script" 2>&1)
    SCRIPT_STATUS=$?
    set -e
}

ok_result='   Compiling pangopup-cli v0.5.0
     Running tests/'"$target"'.rs

running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 4.21s
'
# The measured silent green: `installed_successXYZ` printed exactly this and
# `cargo test` exited 0.
empty_result='     Running tests/'"$target"'.rs

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.00s
'

run_script 0 "$ok_result"
[[ "$SCRIPT_STATUS" == 0 ]] || fail "the script rejected a run that passed 12 tests: $SCRIPT_OUTPUT"
[[ "$(wc -l <"$CALLS" | tr -d ' ')" == 1 ]] || fail 'the script did not invoke cargo exactly once'
expected_call="test --locked --package pangopup-cli --features $feature --test $target $filter"
[[ "$(cat "$CALLS")" == "$expected_call" ]] || fail "the script ran 'cargo $(cat "$CALLS")' instead of 'cargo $expected_call'"

run_script 0 "$empty_result"
[[ "$SCRIPT_STATUS" != 0 ]] || fail "the script accepted a run that selected no test, which is the whole hole: $SCRIPT_OUTPUT"
case "$SCRIPT_OUTPUT" in
    *"$filter"*) ;;
    *) fail "the refusal does not name the filter that selected nothing: $SCRIPT_OUTPUT" ;;
esac

run_script 101 "$ok_result"
[[ "$SCRIPT_STATUS" == 101 ]] || fail "the script reported $SCRIPT_STATUS instead of cargo's own 101"

run_script 0 'no test result line at all
'
[[ "$SCRIPT_STATUS" != 0 ]] || fail 'the script accepted output carrying no test result line, so a changed cargo report would read as zero assertions'

run_script 0 "$ok_result$ok_result"
[[ "$SCRIPT_STATUS" != 0 ]] || fail 'the script accepted two test result lines, so it cannot say which run it counted'
