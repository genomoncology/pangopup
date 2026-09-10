#!/usr/bin/env bash
set -euo pipefail

# A spec block states a claim two ways. A required sentence is pinned by running
# `rg` and letting `set -e` stop the block when the sentence is gone. A claim
# that something must NOT be there was written with a leading `!`:
#
#     ! printf '%s' "$readme" | rg -F -- 'a sentence that must not appear'
#
# `set -e` is defined to ignore a pipeline that begins with the `!` reserved
# word, and mustmatch reports the status of the block's last command. Every one
# of these lines sits in the middle of a block, so the sentence can come back
# and no gate says anything. The pin looks like a gate and checks nothing.
#
# A second shape of the same hole sits beside it. mustmatch skips a plain
# ```bash block that calls no `mustmatch` command, so a refutation moved into
# such a block is inert even when it is written correctly.
#
# This file holds three things shut.
#
#   1. No line inside a spec fenced block starts with `!`. That is the inert
#      form, and it is refused here rather than found years later.
#   2. Every block that carries a refutation also carries a mustmatch call, so
#      the block runs at all.
#   3. Refutations go through scripts/spec-refutes.sh, which reports the two
#      ways a refutation passes on nothing: a haystack with no bytes in it, and
#      a command that was never there to fail. That is arithmetic, so it is
#      proved here against fixtures.
#
# What this does not prove: that the refutations hold. `make spec` runs them.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
helper="$repository/scripts/spec-refutes.sh"
helper_call='../scripts/spec-refutes.sh'

# Today's spec files carry 28 refutations. The ticket that introduced this file
# allows one to be dropped when its reason is written down, so the floor sits
# below that count rather than on it. It exists so that refutation coverage
# cannot collapse to nothing while every check above still reports green.
refutation_floor=20

fail() { printf 'spec refutation evidence: %s\n' "$*" >&2; exit 1; }

shopt -s nullglob
spec_files=("$repository"/spec/*.md)
shopt -u nullglob
(( ${#spec_files[@]} > 0 )) || fail 'found no spec/*.md files to inspect, so this check proved nothing'

# Emits one record per fenced block line as
# "<file>\t<fence line>\t<line>\t<fence info string>\t<has mustmatch>\t<text>".
# The fence line and the mustmatch flag describe the whole block, so a single
# pass gives both the per-line checks and the per-block ones.
spec_block_lines() {
    awk '
        function flush(  i) {
            for (i = 1; i <= n; i++)
                printf "%s\t%d\t%d\t%s\t%d\t%s\n", FILENAME, fence, lines[i], info, has, texts[i]
            n = 0
        }
        FNR == 1 { inblock = 0; n = 0 }
        /^[[:space:]]*```/ {
            if (inblock) { flush(); inblock = 0; next }
            info = $0
            sub(/^[[:space:]]*```/, "", info)
            if (info == "") next
            inblock = 1
            fence = FNR
            has = 0
            n = 0
            next
        }
        !inblock { next }
        {
            if ($0 ~ /(^|[^[:alnum:]_.\/-])mustmatch[[:space:]]/) has = 1
            n++
            lines[n] = FNR
            texts[n] = $0
        }
        END { if (inblock) flush() }
    ' "$@"
}

all_lines=$(spec_block_lines "${spec_files[@]}")
[[ -n "$all_lines" ]] || fail 'read no fenced-block lines out of spec/, so this check inspected nothing'

# --- 1. the inert form is gone ---------------------------------------------
inert=$(printf '%s\n' "$all_lines" | awk -F'\t' '$4 ~ /^bash/ && $6 ~ /^[[:space:]]*![[:space:]]/')
if [[ -n "$inert" ]]; then
    printf 'spec refutation evidence: these spec lines start with `!`, which set -e ignores, so each one refutes nothing:\n' >&2
    printf '%s\n' "$inert" \
        | awk -F'\t' -v root="$repository/" '{ sub("^" root, "", $1); printf "  %s:%d: %s\n", $1, $3, $6 }' >&2
    exit 1
fi

# --- 2. every refutation sits in a block that runs --------------------------
#
# A refutation is a `!`-led line or a call to the helper. Section 1 has already
# refused the first kind, so after it passes this reads the helper calls; before
# it passes, the `!` lines still count, which keeps this section from inspecting
# an empty set while the conversion is under way.
refutations=$(printf '%s\n' "$all_lines" \
    | awk -F'\t' -v call="$helper_call" '$4 !~ /^bash/ { next } $6 ~ /^[[:space:]]*![[:space:]]/ || index($6, call)')
refutation_count=$(printf '%s' "$refutations" | grep -c . || true)
(( refutation_count >= refutation_floor )) || fail \
    "found $refutation_count refutation(s) across ${#spec_files[@]} spec file(s) against a floor of $refutation_floor, so the spec files no longer refute what this check was written to keep refuted"

skipped=$(printf '%s\n' "$refutations" | awk -F'\t' '$4 == "bash" && $5 == 0 { print $1 "\t" $2 }' | sort -u)
if [[ -n "$skipped" ]]; then
    printf 'spec refutation evidence: these blocks carry a refutation and call no mustmatch command, so mustmatch skips the whole block and the refutation never runs:\n' >&2
    printf '%s\n' "$skipped" | awk -F'\t' -v root="$repository/" '{ sub("^" root, "", $1); printf "  %s:%d\n", $1, $2 }' >&2
    exit 1
fi

printf 'inspected %s spec file(s), %s refutation(s)\n' "${#spec_files[@]}" "$refutation_count"

# --- 3. the helper refuses a refutation that proved nothing -----------------
[[ -f "$helper" ]] || fail "no $helper: the spec refutations have nothing to report an empty haystack from"
[[ -x "$helper" ]] || fail "$helper is not executable"

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT

printf 'a sentence that must not appear\nanother line\n' >"$fixture/present.txt"
printf 'another line\n' >"$fixture/absent.txt"
: >"$fixture/empty.txt"

run_helper() {
    set +e
    HELPER_OUTPUT=$(bash "$helper" "$@" 2>&1)
    HELPER_STATUS=$?
    set -e
}

run_helper_stdin() {
    local input=$1
    shift
    set +e
    HELPER_OUTPUT=$(printf '%s' "$input" | bash "$helper" "$@" 2>&1)
    HELPER_STATUS=$?
    set -e
}

needle='a sentence that must not appear'

# The pin's ordinary green: the sentence is absent from a file that has bytes.
run_helper --absent -F -- "$needle" "$fixture/absent.txt"
[[ "$HELPER_STATUS" == 0 ]] || fail "the helper refused a file that does not contain the sentence: $HELPER_OUTPUT"

# The regression the pin exists to catch.
run_helper --absent -F -- "$needle" "$fixture/present.txt"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted a file that contains the forbidden sentence, which is the whole hole'
case "$HELPER_OUTPUT" in
    *"$needle"*) ;;
    *) fail "the refusal does not name the sentence it found, so an operator cannot tell what came back: $HELPER_OUTPUT" ;;
esac

run_helper_stdin 'another line' --absent -F -- "$needle"
[[ "$HELPER_STATUS" == 0 ]] || fail "the helper refused piped text that does not contain the sentence: $HELPER_OUTPUT"

run_helper_stdin "$needle" --absent -F -- "$needle"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted piped text that contains the forbidden sentence'

# The silent green. Most of these pins search a shell variable filled by awk
# from a document. When the heading moves, the variable is empty, every needle
# is absent from it, and the pin reports green having read nothing.
run_helper_stdin '' --absent -F -- "$needle"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted an empty haystack, so a pin whose document went missing still reports green'
case "$HELPER_OUTPUT" in
    *searched*) ;;
    *) fail "the refusal does not say that nothing was searched: $HELPER_OUTPUT" ;;
esac

run_helper --absent -F -- "$needle" "$fixture/empty.txt"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted an empty file as a haystack'

run_helper --absent -F -- "$needle" "$fixture/no-such-file.txt"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted a haystack path that does not exist, so a renamed document reads as a passing pin'
case "$HELPER_OUTPUT" in
    *no-such-file.txt*) ;;
    *) fail "the refusal does not name the path it could not read: $HELPER_OUTPUT" ;;
esac

# rg's own flags reach rg. A case-insensitive pattern is one of the forms the
# spec files use, and it must keep meaning what rg means by it.
run_helper_stdin 'DOCKER' --absent -i 'docker'
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper did not pass -i through to ripgrep, so a case-insensitive refutation stopped refuting'

# --- the other refutation: a command that must fail -------------------------
run_helper --fails false
[[ "$HELPER_STATUS" == 0 ]] || fail "the helper refused a command that failed as the spec says it must: $HELPER_OUTPUT"

run_helper --fails true
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted a command that succeeded, which is the regression this form exists to catch'
case "$HELPER_OUTPUT" in
    *true*) ;;
    *) fail "the refusal does not name the command that succeeded: $HELPER_OUTPUT" ;;
esac

# A renamed or unbuilt executable exits 127. Read as a plain non-zero status
# that is a pin passing because the thing it tests is not there.
run_helper --fails "$fixture/no-such-command"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted a command that does not exist, so an unbuilt executable reads as a passing pin'
case "$HELPER_OUTPUT" in
    *no-such-command*) ;;
    *) fail "the refusal does not name the command it could not run: $HELPER_OUTPUT" ;;
esac

run_helper
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted a call with no arguments'

run_helper --absent
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted --absent with no pattern, so a gate that names nothing reads as green'
