#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
timeout="$repository/scripts/timeout"

[[ -x "$timeout" ]]

set +e
"$timeout" -k 0.2 0.2 sh -c 'exit 7'
status=$?
set -e
[[ "$status" -eq 7 ]] || { echo "timeout changed the command exit status: $status" >&2; exit 1; }

set +e
"$timeout" -k 0.2 0.2 sh -c 'sleep 5'
status=$?
set -e
[[ "$status" -eq 124 ]] || { echo "timeout failed to stop a hung command: $status" >&2; exit 1; }

set +e
"$timeout" -k 0.2 0.2 command-that-does-not-exist
status=$?
set -e
[[ "$status" -eq 127 ]] || { echo "timeout hid a missing command: $status" >&2; exit 1; }

echo 'spec timeout: command, deadline, and missing command statuses passed'
