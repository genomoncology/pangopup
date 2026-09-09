#!/usr/bin/env bash
set -euo pipefail

# CI's only run of the feature-gated service lifecycle. `make test` builds no
# `service-test-fixtures`, so this is the only place a real `pangopup serve`
# runs against a runtime installed from repository fixtures.
#
# The trailing word below is a test-name filter, not a test name: it selects the
# module by prefix. `cargo test` exits 0 when a name filter matches nothing, so
# renaming that module took this step from twelve assertions to zero without
# reporting anything. Read the count back and refuse a run that asserted
# nothing. tests/ci-service-fixture-evidence.sh proves this arithmetic and
# checks that the filter still names a module.
target=http_service_lifecycle
feature=service-test-fixtures
filter=installed_success

log=$(mktemp)
trap 'rm -f "$log"' EXIT

set +e
cargo test --locked --package pangopup-cli --features "$feature" \
    --test "$target" "$filter" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
set -e
[[ "$status" == 0 ]] || exit "$status"

results=$(grep -c '^test result:' "$log" || true)
if [[ "$results" != 1 ]]; then
    printf 'expected one summary line from tests/%s.rs, read %s: cargo no longer reports a count this step can check\n' "$target" "$results" >&2
    exit 1
fi

passed=$(sed -nE 's/^test result: [^0-9]*([0-9]+) passed.*/\1/p' "$log")
if [[ ! "$passed" =~ ^[0-9]+$ ]] || [[ "$passed" -lt 1 ]]; then
    printf 'the %s filter selected no test in tests/%s.rs, so this step exercised nothing and cargo still exited 0\n' "$filter" "$target" >&2
    exit 1
fi
printf '%s tests matched %s in tests/%s.rs\n' "$passed" "$filter" "$target"
