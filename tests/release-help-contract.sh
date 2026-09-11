#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# The `cargo run` below builds and runs in one step, so it cannot be moved
# before the cache home. Build first, so that step links the ONNX Runtime
# library already under the operator's cache rather than downloading another
# copy under the fresh one, and only then take a cache home of this harness's
# own.
"$repo/scripts/require-built-commands.sh"
. "$repo/tests/support/private-cache-home.sh"

actual=$(cargo run --locked --quiet --manifest-path "$repo/Cargo.toml" --package pangopup-cli --bin pangopup -- lookup --help | sed -n '1p')
spec_line=$(grep -F 'pangopup lookup --help | head -1 | mustmatch like ' "$repo/spec/cli.md")
spec_expected=${spec_line#*mustmatch like \'}
spec_expected=${spec_expected%\'}

[[ "$actual" == "$spec_expected" ]] || {
  printf 'focused lookup help differs from spec/cli.md\n' >&2
  exit 1
}
[[ $(grep -Fc 'check_focused_help lookup ' "$repo/scripts/qualify-container.sh") == 1 ]] || {
  printf 'container qualification must retain one lookup help check\n' >&2
  exit 1
}
grep -Fxq "check_focused_help lookup '$actual' lookup" "$repo/scripts/qualify-container.sh" || {
  printf 'container qualification lookup help differs from the shipped contract\n' >&2
  exit 1
}
[[ $(grep -Fc '        "lookup": ' "$repo/scripts/check-production-qualification.py") == 1 ]] || {
  printf 'production qualification must retain one lookup help check\n' >&2
  exit 1
}
grep -Fxq "        \"lookup\": \"$actual\"," "$repo/scripts/check-production-qualification.py" || {
  printf 'production qualification lookup help differs from the shipped contract\n' >&2
  exit 1
}

printf 'release focused-help contracts agree\n'
