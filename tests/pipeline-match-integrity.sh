#!/usr/bin/env bash
set -euo pipefail

# Under pipefail, a reader that closes early can replace the producer's useful
# status with SIGPIPE. The shared shell scanner refuses quiet grep, head, sed q,
# exit-on-first-match awk, grep -m/-l, and bare read when they consume a
# pipeline. Reading a completed file or using a redirection remains valid.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
scanner="$repository/tests/support/shell_scan.py"

fail() { printf 'pipeline match integrity: %s\n' "$*" >&2; exit 1; }

[[ -f "$scanner" ]] || fail "no $scanner"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# One local mutation keeps this owning gate from going green if repository mode
# becomes a command that always succeeds. The complete reader matrix and quoted,
# heredoc, and substitution controls live in shell-scanner-coverage.sh.
fixture="$work/early.sh"
printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail' 'produce | head -n 1' >"$fixture"
fixture_report=$(python3 "$scanner" file "$fixture") \
    || fail 'the shared scanner could not parse the local early-reader fixture'
fixture_findings=$(python3 -c 'import json,sys; print(len(json.load(sys.stdin)["pipeline_findings"]))' <<<"$fixture_report")
[[ "$fixture_findings" == 1 ]] \
    || fail "the shared scanner found $fixture_findings of one pipeline into head"

if ! report=$(python3 "$scanner" pipelines "$repository" 2>"$work/findings"); then
    fail "an early-closing reader still consumes a repository pipeline: $(tr '\n' ' ' <"$work/findings")"
fi
read -r files pipelines findings < <(
    python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["files"], d["pipelines"], d["findings"])' <<<"$report"
)
(( files > 0 && pipelines > 0 && findings == 0 )) \
    || fail 'the shared scanner returned an empty or inconsistent repository result'

printf 'pipeline match integrity: %s tracked shell file(s), %s command list(s), no pipeline feeds an early-closing reader\n' \
    "$files" "$pipelines"
