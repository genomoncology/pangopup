#!/usr/bin/env bash
set -uo pipefail

# One counting form for every `cargo test` gate under spec/.
#
# `cargo test` exits 0 when a name filter selects no test, so a gate that reads
# only the exit code passes on nothing the moment the test it names is renamed
# or deleted. scripts/run-service-fixture-tests.sh reads the count back for
# CI's one feature-gated step; this does the same for the spec gates, and adds
# the two things a spec gate needs that step does not: a floor, so a gate whose
# filter selects a whole module cannot quietly shrink to one test, and a
# refusal that names both the filter and the spec file carrying the gate, so an
# operator can find the gate without searching for it.
#
# Usage, from the spec block's working directory:
#
#   ../scripts/spec-cargo-test.sh <floor> <cargo test args...>
#
# The floor is the number of tests the gate covered when it was written.
# Coverage may grow; it may not shrink. tests/spec-cargo-filter-evidence.sh
# proves this arithmetic and keeps every gate under spec/ on this script.

if (( $# < 2 )); then
    printf 'spec-cargo-test.sh: usage: spec-cargo-test.sh <floor> <cargo test args...>\n' >&2
    exit 1
fi

floor=$1
shift
filter=${!#}

output=$(cargo test "$@" 2>&1)
status=$?
if (( status != 0 )); then
    printf '%s\n' "$output" >&2
    exit "$status"
fi

# The gate lives in a fenced block in one of the spec files beside the working
# directory, so the filter finds it without any call site repeating its own
# name.
shopt -s nullglob
carriers=()
for candidate in ./*.md; do
    grep -qF -- "$filter" "$candidate" && carriers+=("${candidate#./}")
done
shopt -u nullglob
where=${carriers[*]-}
[[ -n "$where" ]] || where='an unidentified spec file'

refuse() {
    printf '%s\n' "$output" >&2
    printf 'spec-cargo-test.sh: %s\n' "$*" >&2
    exit 1
}

summaries=$(printf '%s\n' "$output" | grep -c '^test result:')
(( summaries > 0 )) || refuse "cargo reported no 'test result:' line for the $filter filter in $where, so this gate read no count and checked nothing"

passed=0
while read -r count; do
    passed=$(( passed + count ))
done < <(printf '%s\n' "$output" | sed -nE 's/^test result: [^0-9]*([0-9]+) passed.*/\1/p')

if (( passed < floor )); then
    refuse "the $filter filter in $where selected $passed passing test(s) against a floor of $floor, so this gate no longer covers what it was written to cover"
fi

printf '%s test(s) matched %s\n' "$passed" "$filter"
