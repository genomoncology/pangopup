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

# A tree the currency gate can read as a repository of its own. The four
# harnesses the gate's own floor expects are planted here in their simplest
# correct shape, so that what the gate refuses below is the harness this file
# added and nothing else.
tree="$work/tree"
mkdir -p "$tree/tests" "$tree/scripts"
[[ -f "$repository/tests/$currency_gate" ]] \
    || fail "tests/$currency_gate is gone, so this file is holding a rule that no longer exists"
cp "$repository/tests/$currency_gate" "$tree/tests/$currency_gate"
install -m 755 "$repository/$builder" "$tree/$builder"

# A harness that calls the build script and then uses what it built.
plant_correct() {
    cat >"$tree/tests/$1" <<HARNESS
#!/usr/bin/env bash
bash "\$repo/$builder"
"\$repo/$built/pangopup" --version
HARNESS
}

for name in executable-delivery.sh "$qualification" \
    inherited-cache-variables.sh model-cache-limit-inheritance.sh; do
    plant_correct "$name"
done

# The path assigned to a variable and run through it. This is a call, spelled
# the way `tests/executable-delivery.sh` spells the smoke script, and a rule
# that demanded the path stand as the command word itself would refuse it.
cat >"$tree/tests/assigned-then-run.sh" <<HARNESS
#!/usr/bin/env bash
builder="\$repo/$builder"
"\$builder"
"\$repo/$built/pangopup" --version
HARNESS

GATE_OUTPUT=
GATE_STATUS=
run_currency_gate() {
    set +e
    GATE_OUTPUT=$(bash "$tree/tests/$currency_gate" 2>&1)
    GATE_STATUS=$?
    set -e
}

run_currency_gate
(( GATE_STATUS == 0 )) \
    || fail "the currency gate refused a tree of harnesses that each build before their first use, so the refusals below could not be read as the fixture's fault: $GATE_OUTPUT"

# The defect. Line 2 names the build script inside a `printf` argument, line 3
# uses the built executable, and nothing builds at all. The scan reads line 2
# as the build and accepts.
cat >"$tree/tests/mention-only.sh" <<HARNESS
#!/usr/bin/env bash
printf '$builder\n' >"\$listing"
"\$repo/$built/pangopup" --version
HARNESS

run_currency_gate
(( GATE_STATUS != 0 )) \
    || fail "the currency gate accepted a harness that names $builder in a printf argument and never runs it, so a harness that builds nothing is read as having built: $GATE_OUTPUT"
grep -q 'mention-only\.sh' <<<"$GATE_OUTPUT" \
    || fail "the refusal does not name the harness it refused: $GATE_OUTPUT"
rm -f "$tree/tests/mention-only.sh"

# The same shape with a real build below the first use. The build has to answer
# a missing or stale executable before that executable is used, and reading the
# mention on line 2 as the build hides that the real one stands on line 4.
cat >"$tree/tests/mention-then-build.sh" <<HARNESS
#!/usr/bin/env bash
printf '$builder\n' >"\$listing"
"\$repo/$built/pangopup" --version
bash "\$repo/$builder"
HARNESS

run_currency_gate
(( GATE_STATUS != 0 )) \
    || fail "the currency gate accepted a harness whose only build stands below its first use, because it read a mention above that use as the build: $GATE_OUTPUT"
grep -q 'mention-then-build\.sh' <<<"$GATE_OUTPUT" \
    || fail "the refusal does not name the harness it refused: $GATE_OUTPUT"
rm -f "$tree/tests/mention-then-build.sh"

# Removing the two refused harnesses leaves the tree the gate accepted before,
# so neither refusal above came from something else this file planted.
run_currency_gate
(( GATE_STATUS == 0 )) \
    || fail "the currency gate refuses the tree even with both offending harnesses removed, so the refusals above are not attributable to them: $GATE_OUTPUT"

# The other direction of the same rule, and the reason it needs a fixture of its
# own. The refusals above are answered by narrowing what counts as a build, and
# the narrowest way to write that narrowing is one filter over both patterns --
# which also narrows what counts as a use. A harness that names the built
# executable in a `printf` and builds nothing is refused today, and the floor of
# four still reads four after such a filter, so nothing else here would notice
# the harness quietly leaving the set. The build side and the use side are not
# the same question: a mention hands nobody a build, and a mention of the
# executable is read as a use because refusing a harness that runs nothing is
# the direction this gate is allowed to be wrong in.
cat >"$tree/tests/printf-use.sh" <<HARNESS
#!/usr/bin/env bash
printf '%s\n' "\$repo/$built/pangopup"
HARNESS

run_currency_gate
(( GATE_STATUS != 0 )) \
    || fail "the currency gate accepted a harness that names the built executable in a printf argument and builds nothing, so narrowing what counts as a build has narrowed what counts as a use and a harness can leave the held set without anything saying so: $GATE_OUTPUT"
grep -q 'printf-use\.sh' <<<"$GATE_OUTPUT" \
    || fail "the refusal does not name the harness it refused: $GATE_OUTPUT"
rm -f "$tree/tests/printf-use.sh"

# The rule is stated where the check stands, so a reader who has to change the
# pattern knows what it is for. One whole-line comment naming a mention, within
# reach of a line that carries the build script's path.
mention_comment=$({ grep -niE '^[[:space:]]*#.*mention' "$repository/tests/$currency_gate" || true; } | head -n 1 | cut -d: -f1)
[[ -n "$mention_comment" ]] \
    || fail "tests/$currency_gate states nowhere that a mention of $builder is not a build, so the next reader has only the pattern to go on"
nearest=
while IFS= read -r number; do
    [[ -n "$number" ]] || continue
    distance=$(( number > mention_comment ? number - mention_comment : mention_comment - number ))
    if [[ -z "$nearest" || "$distance" -lt "$nearest" ]]; then
        nearest=$distance
    fi
done < <(grep -nE -- 'require-built-commands' "$repository/tests/$currency_gate" | cut -d: -f1)
[[ -n "$nearest" ]] \
    || fail "tests/$currency_gate names $builder nowhere, so this scan is reading the wrong file"
[[ "$nearest" -le 25 ]] \
    || fail "the sentence in tests/$currency_gate about a mention stands $nearest lines from the nearest line naming $builder, so it is not beside the check it explains"

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
