#!/usr/bin/env bash
set -uo pipefail

# Refute a claim inside a spec block and stop the block when the refutation
# does not hold.
#
# A spec block used to write a refutation with a leading `!`:
#
#     ! printf '%s' "$readme" | rg -F -- 'a sentence that must not appear'
#
# `set -e` is defined to ignore a pipeline that begins with the `!` reserved
# word, and mustmatch reports the status of a block's last command, so such a
# line in the middle of a block changed nothing whatever it found. This script
# is the live form. It exits non-zero when the claim breaks, which `set -e`
# turns into a failed block on the line that broke.
#
# It also refuses the two ways a refutation passes on nothing: a haystack with
# no bytes in it, and a command that was never there to fail. Both read as
# green when nobody is looking at them.
#
# It never reads the caller's standard input except as the haystack of an
# --absent call that names no path, and it refuses that call when standard
# input is a terminal. A gate that inherits a terminal and reads it waits
# forever, and a gate has to fail rather than wedge.
#
# tests/spec-refutation-evidence.sh holds this contract and the spec files that
# depend on it.

program=$(basename "${BASH_SOURCE[0]}")

refuse() {
    printf '%s: %s\n' "$program" "$*" >&2
    exit 1
}

absent() {
    local flags=()
    while (($#)); do
        case $1 in
            --) shift; break ;;
            -*) flags+=("$1"); shift ;;
            *) break ;;
        esac
    done
    (($#)) || refuse '--absent names no pattern, so it refutes nothing'

    local pattern=$1
    shift
    local paths=("$@")

    local -a haystack_args=()
    local where
    if ((${#paths[@]})); then
        local path bytes=0 size
        for path in "${paths[@]}"; do
            [[ -e $path ]] || refuse "cannot read $path, so the refutation of '$pattern' searched nothing"
            [[ -r $path ]] || refuse "cannot read $path, so the refutation of '$pattern' searched nothing"
            if [[ -d $path ]]; then
                # A directory ripgrep will walk. It counts as searched when it
                # holds at least one file.
                if [[ -n $(find "$path" -type f -print -quit) ]]; then
                    size=1
                else
                    size=0
                fi
            else
                size=$(wc -c <"$path")
            fi
            bytes=$((bytes + size))
        done
        ((bytes > 0)) || refuse "every path given holds no bytes, so the refutation of '$pattern' searched nothing: ${paths[*]}"
        haystack_args=("${paths[@]}")
        where="${paths[*]}"
    else
        # A path argument that went missing leaves the haystack as whatever the
        # gate inherited. On a terminal that read never returns, so the pin
        # stops the gate instead of failing it. Refuse rather than wedge.
        if [[ -t 0 ]]; then
            refuse "no path is named and standard input is a terminal, so the refutation of '$pattern' would wait forever instead of searching anything"
        fi
        local text
        text=$(cat)
        [[ -n $text ]] || refuse "the text piped in holds no bytes, so the refutation of '$pattern' searched nothing"
        local scratch
        scratch=$(mktemp) || refuse "could not hold the piped text to search it for '$pattern'"
        # shellcheck disable=SC2064
        trap "rm -f '$scratch'" EXIT
        printf '%s' "$text" >"$scratch"
        haystack_args=("$scratch")
        where='the text piped in'
    fi

    local found status
    found=$(rg ${flags[@]+"${flags[@]}"} -- "$pattern" "${haystack_args[@]}" 2>&1)
    status=$?
    case $status in
        0) refuse "'$pattern' is present in $where, and this line refutes it: $(printf '%s' "$found" | head -3)" ;;
        1) return 0 ;;
        *) refuse "ripgrep could not search $where for '$pattern': $found" ;;
    esac
}

fails() {
    (($#)) || refuse '--fails names no command, so it refutes nothing'

    local text=$*
    local output status
    output=$("$@" 2>&1 </dev/null)
    status=$?
    case $status in
        0) refuse "the command succeeded, and this line refutes it: $text${output:+ -- $output}" ;;
        127) refuse "the command was never there to run, so its failure proves nothing: $text${output:+ -- $output}" ;;
        *) return 0 ;;
    esac
}

case ${1-} in
    --absent) shift; absent "$@" ;;
    --fails) shift; fails "$@" ;;
    '') refuse 'called with no arguments: name --absent or --fails and what it refutes' ;;
    *) refuse "unknown mode '$1': name --absent or --fails" ;;
esac
