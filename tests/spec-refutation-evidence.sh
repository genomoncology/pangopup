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
# This file holds five things shut.
#
#   1. No line inside a spec fenced block starts with `!`. That is the inert
#      form, and it is refused here rather than found years later.
#   2. Every block that carries a refutation also carries a mustmatch call, so
#      the block runs at all.
#   3. Every refutation that is not the receiving end of a pipe names a path.
#      One that names neither reads whatever standard input the gate inherited,
#      and on a terminal that read never returns.
#   4. A block that builds a FIFO runs every command in it under `timeout`. A
#      writerless FIFO can stop a command that opens it, and a stopped command
#      hangs the gate instead of failing it. See the settlement below.
#   5. Refutations go through scripts/spec-refutes.sh, which reports the ways a
#      refutation passes on nothing: a haystack with no bytes in it, a command
#      that was never there to fail, a command that is there and could not be
#      run, and a lost path argument that turns the haystack into an inherited
#      terminal the helper would read forever. That is arithmetic, so it is
#      proved here against fixtures.
#
# What this does not prove: that the refutations hold. `make spec` runs them.
#
# ---------------------------------------------------------------------------
# What the stage that makes this green has to write.
#
# scripts/spec-refutes.sh, executable, called from a spec block by exactly the
# relative path `../scripts/spec-refutes.sh` (spec blocks run in spec/). Two
# forms, each exiting non-zero when the claim breaks so `set -e` stops the
# block on the line that broke:
#
#   --absent <ripgrep args...>   The pattern must not be found. Every argument
#                                after --absent reaches ripgrep unchanged, so
#                                -F, -i, -n and -- keep meaning what ripgrep
#                                means by them. The haystack is a path argument
#                                or standard input. Refuse when the pattern is
#                                found, when nothing with bytes in it was
#                                searched, and when a named path could not be
#                                read. Name the pattern, and the path when
#                                there is one, in every refusal, and say that
#                                nothing was searched when nothing was.
#   --fails <command...>         The command must exit non-zero. Refuse when it
#                                succeeded, when it was never there to run
#                                (status 127), and when it was there and could
#                                not be run (status 126) -- a file without its
#                                execute bit, or a directory. Name the command
#                                in each refusal, and say which of the three it
#                                was: 126 and 127 are different accidents and a
#                                reader has to be able to tell a missing
#                                command from an unrunnable one.
#
# Neither form takes a bare call: a call with no mode, and --absent with no
# pattern, are refused.
#
# The settlement for the FIFO pin in spec/runtime-transport.md. Ticket 0080
# removed that pin because verify opened the member, the open blocked with no
# writer, and the command never returned, so a live pin hung the gate rather
# than failing it. Measured then: `pangopup-build runtime-transport verify` on
# such a transport was still running when a 20-second timeout killed it, while
# the corrupt, substituted and symlinked members were each refused in
# milliseconds. Ticket 0082 is the defect in verify, and it names the pin as
# what it restores.
#
# The pin is back, as an exact-output pin rather than a `--fails` refutation,
# and bounded by `timeout`. The two shapes matter. `--fails` reads a command
# killed by `timeout` as a satisfied refutation, so it would go green on a
# command that still hangs; an exact-output pin goes green only on the refusal
# text verify actually prints. `timeout` is what keeps the gate failing rather
# than waiting while verify is still wrong. Section 4 below holds that bound in
# place for any block that builds a FIFO.
# ---------------------------------------------------------------------------

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
helper="$repository/scripts/spec-refutes.sh"
helper_call='../scripts/spec-refutes.sh'

# Today's spec files carry 27 `--fails` and `--absent` refutations. The FIFO pin
# settled above is not among them: it came back as an exact-output pin, which
# this count does not read. The floor sits on 27 rather than under it. A floor
# with slack in it is a licence to delete refutations quietly: measured here, a
# floor of 20 let seven of the 27 be deleted outright with every gate still
# green. On the number, a deletion has to move this line, and moving it is a
# change someone reads.
refutation_floor=27

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

# Only a plain ```bash fence needs a mustmatch call. A ```bash run id=... fence
# carries its own `exit=` expectation, so mustmatch runs it and checks its
# status whether or not it calls mustmatch. That is why this reads $4 exactly
# rather than by prefix.
skipped=$(printf '%s\n' "$refutations" | awk -F'\t' '$4 == "bash" && $5 == 0 { print $1 "\t" $2 }' | sort -u)
if [[ -n "$skipped" ]]; then
    printf 'spec refutation evidence: these blocks carry a refutation and call no mustmatch command, so mustmatch skips the whole block and the refutation never runs:\n' >&2
    printf '%s\n' "$skipped" | awk -F'\t' -v root="$repository/" '{ sub("^" root, "", $1); printf "  %s:%d\n", $1, $2 }' >&2
    exit 1
fi

printf 'inspected %s spec file(s), %s refutation(s)\n' "${#spec_files[@]}" "$refutation_count"

# --- 3. a refutation that is not piped names its haystack -------------------
#
# The same hole in its last shape. A call whose path argument goes missing has
# no haystack but standard input, and standard input in a spec block is
# whatever the gate inherited. On a terminal that read never returns: the block
# stops instead of failing, and a stopped gate reports nothing either. The
# helper refuses such a call at run time. This keeps one out of spec/ in the
# first place, and it is checkable without handing a terminal to a background
# reader, which would stop this whole process group on SIGTTIN.
unhaystacked=$(printf '%s\n' "$all_lines" | awk -F'\t' -v call="$helper_call" '
    $4 !~ /^bash/ { next }
    {
        if (pending != "") {
            pending = pending " " $6
            if ($6 !~ /\\$/) { check(pendfile, pendline, pending); pending = "" }
            next
        }
        if (!index($6, call " --absent")) next
        if ($6 ~ ("[|][[:space:]]*" "\\.\\./scripts/spec-refutes\\.sh")) next
        if ($6 ~ /\\$/) { pending = $6; pendfile = $1; pendline = $3; next }
        check($1, $3, $6)
    }
    function check(f, l, text) {
        rest = substr(text, index(text, call " --absent") + length(call " --absent"))
        if (rest !~ /[[:space:]]\.\.\//) printf "%s\t%d\t%s\n", f, l, text
    }
')
if [[ -n "$unhaystacked" ]]; then
    printf 'spec refutation evidence: these refutations are not piped anything and name no path, so each one reads whatever standard input the gate inherited instead of a haystack:\n' >&2
    printf '%s\n' "$unhaystacked" \
        | awk -F'\t' -v root="$repository/" '{ sub("^" root, "", $1); printf "  %s:%d: %s\n", $1, $2, $3 }' >&2
    exit 1
fi

# --- 4. a block that builds a FIFO bounds what reads it ---------------------
#
# A FIFO member has no writer. A command that opens it and waits never returns,
# and mustmatch's block timeout does not reach a grandchild, so an unbounded
# command in such a block hangs the gate and reports nothing instead of failing.
# The pin on a FIFO member is live again, so the fixture is allowed; what is
# refused is an unbounded command beside it. Both shipped commands read
# transport members, so a `pangopup-build` or `pangopup` line in a block that
# builds a FIFO has to run under `timeout`, which turns a command that waits
# back into a block that fails.
fifo_blocks=$(printf '%s\n' "$all_lines" \
    | awk -F'\t' '$4 ~ /^bash/ && $6 ~ /(^|[^[:alnum:]_.\/-])mkfifo([[:space:]]|$)/ { print $1 "\t" $2 }' | sort -u)
if [[ -n "$fifo_blocks" ]]; then
    unbounded=$(printf '%s\n' "$all_lines" | awk -F'\t' -v blocks="$fifo_blocks" '
        BEGIN { n = split(blocks, rows, "\n"); for (i = 1; i <= n; i++) fifo[rows[i]] = 1 }
        $4 !~ /^bash/ { next }
        !(($1 "\t" $2) in fifo) { next }
        $6 !~ /(^|[^[:alnum:]_.\/-])(pangopup-build|pangopup)([[:space:]]|$)/ { next }
        $6 ~ /(^|[^[:alnum:]_.\/-])timeout[[:space:]]/ { next }
        { printf "%s\t%d\t%s\n", $1, $3, $6 }
    ')
    if [[ -n "$unbounded" ]]; then
        printf 'spec refutation evidence: these spec lines run a command unbounded inside a block that builds a FIFO, and a command that waits on a writerless FIFO hangs the gate instead of failing it:\n' >&2
        printf '%s\n' "$unbounded" \
            | awk -F'\t' -v root="$repository/" '{ sub("^" root, "", $1); printf "  %s:%d: %s\n", $1, $2, $3 }' >&2
        printf 'Put the command under `timeout` so that the block fails on a command that never returns.\n' >&2
        exit 1
    fi
fi

# --- 5. the helper refuses a refutation that proved nothing -----------------
[[ -f "$helper" ]] || fail "no $helper: the spec refutations have nothing to report an empty haystack from"
[[ -x "$helper" ]] || fail "$helper is not executable"

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT

printf 'a sentence that must not appear\nanother line\n' >"$fixture/present.txt"
printf 'another line\n' >"$fixture/absent.txt"
: >"$fixture/empty.txt"

# </dev/null: a helper that loses its path argument searches standard input
# instead, and on an inherited terminal that waits forever. The gate has to
# fail on such a helper, not hang on it.
run_helper() {
    set +e
    HELPER_OUTPUT=$(bash "$helper" "$@" 2>&1 </dev/null)
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
    *"$needle"*) ;;
    *) fail "the refusal does not name the pattern that searched nothing, so an operator cannot tell which pin went blind: $HELPER_OUTPUT" ;;
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
# The 127 message stays what it is. It is what the 126 refusal below is held
# apart from, so a repair that made the two read alike would leave both
# assertions passing on one sentence.
case "$HELPER_OUTPUT" in
    *'never there'*) ;;
    *) fail "the refusal for a command that does not exist no longer says it was never there to run: $HELPER_OUTPUT" ;;
esac

# A command that is there and cannot be run exits 126. Read as a plain non-zero
# status that is a pin passing on a command that never ran a line: measured
# before this case existed, a shell script left at mode 644 and named to
# --fails made the helper exit 0 and report nothing. `spec/runtime-transport.md`
# names `pangopup-build` on three --fails pins, so a build that left the
# executable without its execute bit turned all three green.
not_executable="$fixture/present-not-executable.sh"
printf '#!/bin/sh\nexit 0\n' >"$not_executable"
chmod 644 "$not_executable"
run_helper --fails "$not_executable"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted a command that is there and cannot be run, so an executable that lost its execute bit reads as a passing pin'
case "$HELPER_OUTPUT" in
    *present-not-executable.sh*) ;;
    *) fail "the refusal does not name the command that could not be run: $HELPER_OUTPUT" ;;
esac

# A command that was never installed and one whose execute bit is gone want
# different repairs. The 127 refusal says the command was never there, and
# saying that of a command that is there sends a reader looking for a build
# that already ran.
case "$HELPER_OUTPUT" in
    *'never there'*) fail "the helper reports a command that is there and could not be run as one that was never there, so a reader is sent to the wrong repair: $HELPER_OUTPUT" ;;
esac

# A directory named where a command was expected comes back with the same
# status, and it is the accident a lost path component produces.
mkdir -p "$fixture/a-directory"
run_helper --fails "$fixture/a-directory"
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted a directory named where a command was expected, so a pin whose path lost a component reads as green'
case "$HELPER_OUTPUT" in
    *a-directory*) ;;
    *) fail "the refusal does not name the directory it could not run: $HELPER_OUTPUT" ;;
esac

run_helper
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted a call with no arguments'

run_helper --absent
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted --absent with no pattern, so a gate that names nothing reads as green'

run_helper --fails
[[ "$HELPER_STATUS" != 0 ]] || fail 'the helper accepted --fails with no command, so a gate that names nothing reads as green'
