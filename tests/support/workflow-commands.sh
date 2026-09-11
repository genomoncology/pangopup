# The one way a gate proves a workflow runs a command. Sourced, never run.
#
# Searching a workflow file for a command's text reads a YAML comment as
# happily as a step, so the step can be replaced with `run: "true"` while the
# original line stays above it behind a `#` and the gate stays green. Ticket
# 0050 closed that shape inside `tests/built-executable-currency.sh` by
# requiring a match to stand in code. This file holds the same answer for
# `tests/ci-platform-support.sh` and `tests/executable-delivery.sh`, in one
# place so the two cannot drift apart again.
#
#     require_workflow_command <workflow-file> <command> [<step name>]
#
# The command stands in code when a line of the workflow carries it before any
# `#`, so a full-line comment and a trailing comment beside a no-op are both
# refused while a comment repeating a line that still runs is not. The command
# is matched as text, not as a pattern: a `.` in a version number matches a `.`
# and nothing else, so the rule never exempts more than the command it names.
#
# A third argument narrows the search to one step -- the lines from
# `- name: <step name>` up to the next list entry. Without it the whole file is
# read.
#
# On refusal the message names the command and the workflow file, and the
# caller exits non-zero.
#
# `tests/workflow-command-anchoring.sh` holds both sides: that both gates
# source this file, and that every command they guard refuses a comment in
# place of the step.

require_workflow_command() {
    local workflow=$1
    local command=$2
    local scope=${3:-}

    if awk -v command="$command" -v scope="$scope" '
        BEGIN { inside = (scope == "") }
        {
            if (scope != "") {
                entry = $0
                sub(/^[[:space:]]*/, "", entry)
                if (entry ~ /^- /) { inside = (entry == "- name: " scope) }
                if (!inside) { next }
            }
            code = $0
            sub(/#.*/, "", code)
            if (index(code, command) > 0) { found = 1; exit }
        }
        END { exit (found ? 0 : 1) }
    ' "$workflow"; then
        return 0
    fi

    printf 'workflow command does not stand in code: %s\n' "$command" >&2
    printf '  no line of %s runs it outside a comment%s\n' \
        "$workflow" "${scope:+, in the step named $scope}" >&2
    exit 1
}
