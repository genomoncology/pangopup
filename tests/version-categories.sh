#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
checker="$repo/scripts/check-version-categories.py"
root="$repo/target/version-categories-test"

fail() { printf 'version categories test: %s\n' "$*" >&2; exit 1; }

rm -rf -- "$root"
mkdir -p "$root"
[[ -x "$checker" ]] || fail "checker is missing or not executable: $checker"
"$checker" "$repo" >"$root-pass.out"
grep -Fq 'version categories passed' "$root-pass.out"

rm -rf -- "$root"
mkdir -p "$root"
while IFS= read -r path; do
  mkdir -p "$root/$(dirname "$path")"
  cp "$repo/$path" "$root/$path"
done < <("$checker" --paths "$repo")

expect_rejected() {
  local label=$1 expected=$2
  if "$checker" "$root" >"$root/$label.out" 2>"$root/$label.err"; then
    fail "$label drift unexpectedly passed"
  fi
  grep -Fq "$expected" "$root/$label.err" || {
    cat "$root/$label.err" >&2
    fail "$label refusal did not name its file and category"
  }
}

python3 - "$root/.github/workflows/publish-container.yml" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
path.write_text(path.read_text().replace("  VERSION: 0.4.0", "  VERSION: 9.9.9", 1))
PY
expect_rejected candidate \
  '.github/workflows/publish-container.yml: application-candidate expected'
cp "$repo/.github/workflows/publish-container.yml" \
  "$root/.github/workflows/publish-container.yml"

python3 - "$root/CITATION.cff" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
path.write_text(path.read_text().replace("version: 0.3.0", "version: 9.9.9", 1))
PY
expect_rejected public 'CITATION.cff: current-public expected'
cp "$repo/CITATION.cff" "$root/CITATION.cff"

python3 - "$root/planning/artifacts/054-release-notes.md" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
path.write_text(path.read_text().replace("v0.3.0", "v9.9.9", 1))
PY
expect_rejected history \
  'planning/artifacts/054-release-notes.md: historical-record expected'
cp "$repo/planning/artifacts/054-release-notes.md" \
  "$root/planning/artifacts/054-release-notes.md"

python3 - "$root/tests/container-tag-absence.sh" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
path.write_text(path.read_text().replace("0.3.0", "9.9.9", 1))
PY
expect_rejected fixture \
  'tests/container-tag-absence.sh: fixed-fixture expected'

printf 'version category drift checks passed\n'
