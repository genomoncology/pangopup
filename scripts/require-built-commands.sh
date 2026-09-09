#!/usr/bin/env bash
set -euo pipefail

# Bring the commands a shell harness runs up to date with the source beside
# them. A harness comparing what the tool prints against an oracle is making a
# claim about this checkout, and an executable left over from an older source
# answers that claim with bytes from the past: the stub and the comparison
# beside it reach the same stale renderer, agree with each other, and the
# harness goes green. Building first is what `make spec` already does before it
# uses these same two commands.
#
# One script for every caller, so two harnesses cannot answer this differently.
# tests/built-executable-currency.sh holds both halves.
repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
built=$repository/target/debug

if ! cargo build --locked --quiet --manifest-path "$repository/Cargo.toml" \
    --package pangopup-cli --package pangopup-build >&2; then
    printf 'could not build the commands this harness runs: cargo build --locked --package pangopup-cli --package pangopup-build\n' >&2
    exit 1
fi

for command in pangopup pangopup-build; do
    if [[ ! -x "$built/$command" || -L "$built/$command" ]]; then
        printf 'the build left no %s executable at %s\n' "$command" "$built/$command" >&2
        exit 1
    fi
done
