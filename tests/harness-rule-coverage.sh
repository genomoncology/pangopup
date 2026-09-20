#!/usr/bin/env bash
set -euo pipefail

# Two rules read the shell harnesses under `tests/`, and each of them has a
# hole.
#
# --- the currency scan reads a mention as a build ---
#
# `tests/built-executable-currency.sh` holds that a harness reaching into the
# build directory builds first. It finds the build by matching the build
# script's path as text and taking the lowest matching line, so a harness whose
# line 2 names that path inside a `printf` -- a fixture generator writing the
# path into a file it is about to check -- is read as having built on line 2.
# A use on line 3 then stands above the real build, or above no build at all,
# and the scan accepts it.
#
# `tests/shell-spawn-cache-isolation.sh` already answers the same question
# about a different path: it asks where the path stands, not whether the file
# contains it. A path standing as an argument hands nothing to anybody. The
# fixtures below hold the currency scan to that distinction, in both
# directions, because a rule that read an assignment of the path as a mention
# would refuse a harness that builds perfectly well.
#
# --- a harness that says nothing when it fails ---
#
# `tests/production-release-qualification.sh` asserts with bare commands under
# `set -euo pipefail`. A failure stops it with status 1 and no output naming
# what was wanted. Ticket 0085 made `tests/support/expected-text.sh` for
# exactly this and scoped its gate to one other harness; this file widens that
# gate to cover this one, and holds the other half of the repair at the same
# time. Converting an assertion rewrites the statement around the expectation,
# so `tests/fixtures/production-release-expectations.txt` records the
# expectations themselves and every one of them has to survive: a harness that
# says what it wanted must still refuse every input it refuses today.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

currency_gate=built-executable-currency.sh
strength_gate=negative-assertion-strength.sh
qualification=production-release-qualification.sh
expectations="$repository/tests/fixtures/production-release-expectations.txt"

# Spelled through variables so that neither this file nor the fixtures it
# writes are read as a run of the built executable, or as a build, by the
# sibling scans over `*.sh`.
builder='scripts/require-built-commands.sh'
profile=debug
built="target/$profile"

fail() { printf 'harness rule coverage: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# --- 1. a mention is not a build -------------------------------------------

# The shared coverage harness owns the mutation matrix for direct and bound
# calls, uncalled assignments and functions, heredoc text, reassignment, late
# invocation, and a target/debug path used as a command argument. Running it
# here keeps this older coverage gate connected to the new shared mechanism.
shared_coverage="$repository/tests/shell-scanner-coverage.sh"
[[ -x "$shared_coverage" ]] || fail "no executable $shared_coverage"
coverage_output=$(bash "$shared_coverage") \
    || fail 'the shared shell scanner coverage harness failed'
case "$coverage_output" in
    *' named shape and repository-mode checks passed'*) ;;
    *) fail "the shared coverage harness returned an unknown result: $coverage_output" ;;
esac

currency_output=$(bash "$repository/tests/$currency_gate") \
    || fail 'the repository builder-currency gate failed'
case "$currency_output" in
    *'4 harness(es) reaching into target/debug, each building first'*) ;;
    *) fail "the builder-currency gate returned an unknown result: $currency_output" ;;
esac

# --- 2. an assertion that says what it wanted ------------------------------

harness="$repository/tests/$qualification"
[[ -f "$harness" ]] || fail "tests/$qualification is gone, so this file is holding the wrong harness"

grep -qE '^[^#]*support/expected-text\.sh' "$harness" \
    || fail "tests/$qualification does not source tests/support/expected-text.sh, so its assertions have no way to say what they wanted when they fail"

# The scan that refuses a silent assertion lives in
# `tests/$strength_gate` and names the harnesses it reads. This
# harness has to be one of them: that is what makes the conversion a gate
# rather than a one-time edit.
listed=$(sed -n 's/^assertion_harnesses=(\(.*\))[[:space:]]*$/\1/p' "$repository/tests/$strength_gate")
[[ -n "$listed" ]] \
    || fail "tests/$strength_gate no longer declares assertion_harnesses, so this scan is reading the wrong file"
case " $listed " in
    *" $qualification "*) ;;
    *) fail "tests/$strength_gate reads $listed and not $qualification, so nothing refuses a silent assertion in that harness" ;;
esac

# Every expectation the harness required before the conversion still stands in
# it. A converted assertion keeps the text it searches for; a dropped or
# loosened one does not.
[[ -f "$expectations" ]] \
    || fail "no $expectations, so nothing records what this harness refused before its assertions were rewritten"

# Measured on 2026-09-11: the 66 bare assertions in this harness searched for
# 57 distinct quoted texts. The floor is that number rather than under it, so
# a truncated inventory fails here instead of passing on what is left.
#
# Read against the harness's code lines rather than the whole file. An
# expectation standing in a comment is a sentence about an assertion and not an
# assertion, so a conversion that dropped a check and left its text in the
# commentary beside it would otherwise satisfy every line of this list.
expectation_floor=57
harness_code="$work/qualification-code"
{ grep -vE '^[[:space:]]*#' "$harness" || true; } >"$harness_code"
[[ -s "$harness_code" ]] \
    || fail "tests/$qualification has no code lines, so this scan is reading the wrong file"
checked=0
while IFS= read -r wanted; do
    case "$wanted" in
        '#'*|'') continue ;;
    esac
    checked=$((checked + 1))
    grep -Fq -- "$wanted" "$harness_code" \
        || fail "tests/$qualification no longer requires on a code line the text it required before its assertions were rewritten, so it accepts an input it refused on 2026-09-11: $wanted"
done <"$expectations"
[[ "$checked" -ge "$expectation_floor" ]] \
    || fail "read $checked expectation(s) out of $expectations against a floor of $expectation_floor, so the record of what this harness refused has been cut down"

# The expectations above are the texts. Four of the bare assertions searched
# for no distinctive text of their own -- a non-empty variable, two counts of
# lines in a log, one count of an image name -- and what holds those is that
# none of the harness's assertions disappears. Measured on 2026-09-11: 70
# assertions stand at the left margin of this harness. Rewriting one keeps it
# at the left margin, so the count does not fall unless an assertion is
# removed.
assertion_floor=70
assertions=$({ grep -cE '^(grep |\[\[ |require_text |require_line |require_pattern |equal |refuse_text )' "$harness" || true; })
[[ "$assertions" -ge "$assertion_floor" ]] \
    || fail "tests/$qualification carries $assertions assertion(s) at the left margin against a floor of $assertion_floor, so the conversion dropped one rather than rewriting it"

printf 'harness rule coverage: the currency scan tells a call of %s from a mention of it, and %s assertion(s) in tests/%s say what they wanted while keeping %s recorded expectation(s)\n' \
    "$builder" "$assertions" "$qualification" "$checked"
