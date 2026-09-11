#!/usr/bin/env bash
set -euo pipefail

# A gate that asks whether a file matches something has to give the same answer
# every time it is asked.
#
# `tests/shell-spawn-cache-isolation.sh` asks its two questions as a pipeline
# ending in `grep -q`, under `set -euo pipefail`. `grep -q` exits on its first
# match and closes the pipe; the stage above it dies of SIGPIPE with status
# 141; `pipefail` makes 141 the pipeline's status, and the match grep just
# found is thrown away. Whether that happens depends on how the two processes
# are scheduled, so the gate refuses a tree it should accept -- measured at 7
# refusals in 40 runs of the whole gate on a loaded 16-core machine.
#
# This file asks the same two questions directly, 128 times each with sixteen
# askers running at once, so the scheduling the bug depends on actually
# happens. Seven questions at 128 asks is 896 runs of the shape under load.
# It asks them of a file large enough that the stage above `grep` still has
# more than a pipe buffer to write when the match is found, which is the
# condition that turns the race into a certainty rather than a coin flip:
# measured on this tree, 128 out of 128 asks of each question about that file
# came back wrong.
#
# It asks them of the repository's own files as well, where nothing is
# amplified: the same two argument runners and the same smoke caller the gate
# finds today must come back the same way every time. Measured on this tree,
# `scripts/run-production-qualification.sh` came back wrong on about a fifth of
# those asks, which is the rate the whole gate was measured flaking at. Against
# a rate that low, 128 asks leave a chance of missing it below one in ten
# thousand million.
#
# The questions are taken out of the gate rather than restated here, so that
# this file proves the shipped predicate and not a copy of it. It names the
# four functions and the eight settings it lifts, and refuses when one of them
# is missing: a lift that came back empty would define nothing, every ask would
# fail for the wrong reason, and a lift that came back empty after a rename
# would otherwise look like the bug it is watching for.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
gate=$repository/tests/shell-spawn-cache-isolation.sh

fail() { printf 'shell matching determinism: %s\n' "$*" >&2; exit 1; }

[[ -f "$gate" ]] \
    || fail "tests/shell-spawn-cache-isolation.sh is gone, so this file is holding a rule that no longer exists"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# --- the questions, lifted out of the gate ---------------------------------

settings=(
    smoke_relative smoke_words smoke_lead smoke_prefix smoke_path smoke_call
    argument_words argument_lead
)
for name in "${settings[@]}"; do
    line=$(grep -m1 -- "^$name=" "$gate") \
        || fail "the gate no longer assigns $name at file scope, so this file cannot ask its questions"
    eval "$line"
done

predicates=(logical_lines positional_variables hands_over runs_an_argument)
for name in "${predicates[@]}"; do
    body=$(sed -n "/^$name() {/,/^}/p" "$gate")
    [[ -n "$body" ]] \
        || fail "the gate no longer defines $name at file scope, so this file cannot ask its questions"
    [[ "$body" == *$'\n}' ]] \
        || fail "the lift of $name did not reach its closing brace, so this file would be asking half a question"
    eval "$body"
done

# --- the fixtures ----------------------------------------------------------

# A file both questions must answer yes about, with enough below the answer
# that the stage above `grep` is still writing when `grep` stops reading. The
# filler is what makes the wrong answer certain rather than occasional; the
# three lines above it are the ordinary shapes the gate recognises.
filler_lines=1200
filler="'$(printf 'a line the questions do not match, %.0s' {1..4})'"
{
    printf 'bash %s "$argument"\n' "$smoke_relative"
    printf 'binary=$1\n'
    printf '"$binary" --version\n'
    for ((i = 0; i < filler_lines; i++)); do
        printf 'printf %s %d\n' "$filler" "$i"
    done
} >"$work/matching.sh"

# The same size with neither shape in it. A question that answered yes here
# would be matching something other than what it claims to match.
for ((i = 0; i < filler_lines; i++)); do
    printf 'printf %s %d\n' "$filler" "$i"
done >"$work/quiet.sh"

# The repository's own files, by the answer each one must come back with. The
# two argument runners are the two the gate names. The smoke caller is the one
# other file in the repository that runs the smoke script rather than merely
# naming its path -- `tests/workflow-command-anchoring.sh` names it inside a
# workflow command it reads, and is deliberately not here.
argument_yes=("$smoke_relative" scripts/run-production-qualification.sh)
smoke_yes=(tests/executable-delivery.sh)
for relative in "${argument_yes[@]}" "${smoke_yes[@]}"; do
    [[ -f "$repository/$relative" ]] \
        || fail "$relative is gone, so this file is asking its questions about the wrong tree"
done

# --- asking ----------------------------------------------------------------
#
# Each worker asks every question `per_worker` times and writes one line per
# question naming how many asks came back with the wrong answer. Eight workers
# at once is the load: the bug needs two processes scheduled against each
# other, and a machine with nothing else to do hides it.

workers=16
per_worker=8
asks=$((workers * per_worker))
(( asks > 0 )) || fail 'the run count is zero, so nothing would be asked'

answers=$work/answers
mkdir -p "$answers"

# `$1` names the answer file, `$2` the expected answer, `$3` the question, and
# `$4` the file to ask it about. Counts the asks that came back wrong.
ask() {
    local record=$1 expected=$2 question=$3 subject=$4 wrong=0 got r
    for ((r = 0; r < per_worker; r++)); do
        if "$question" "$subject"; then got=yes; else got=no; fi
        [[ "$got" == "$expected" ]] || wrong=$((wrong + 1))
    done
    printf '%s %s %s %d\n' "$question" "${subject##*/}" "$expected" "$wrong" >>"$record"
}

for ((w = 0; w < workers; w++)); do
    (
        record=$answers/$w
        : >"$record"
        ask "$record" yes hands_over "$work/matching.sh"
        ask "$record" no hands_over "$work/quiet.sh"
        ask "$record" yes runs_an_argument "$work/matching.sh"
        ask "$record" no runs_an_argument "$work/quiet.sh"
        for relative in "${argument_yes[@]}"; do
            ask "$record" yes runs_an_argument "$repository/$relative"
        done
        for relative in "${smoke_yes[@]}"; do
            ask "$record" yes hands_over "$repository/$relative"
        done
    ) &
done
wait

# --- the answers -----------------------------------------------------------

summary=$work/summary
awk '
    { wrong[$1 " " $2 " " $3] += $4; asked[$1 " " $2 " " $3] += 1; lines++ }
    END { for (key in wrong) printf "%s %d %d\n", key, wrong[key], asked[key]; printf "lines %d\n", lines > "/dev/stderr" }
' "$answers"/* >"$summary" 2>"$work/lines"

read -r _ recorded <"$work/lines"
(( recorded > 0 )) \
    || fail 'no worker recorded an answer, so nothing was asked'

questions=0
wrong_total=0
while read -r question subject expected wrong asked; do
    questions=$((questions + 1))
    (( asked == workers )) \
        || fail "$question about $subject was recorded by $asked worker(s) rather than $workers, so some worker did not ask it"
    if (( wrong > 0 )); then
        printf '%s about %s answered %d of %d asks differently from the other asks; the fixture supports %s for every one of them\n' \
            "$question" "$subject" "$wrong" "$asks" "$expected" >&2
        wrong_total=$((wrong_total + wrong))
    fi
done <"$summary"

# Seven questions: two about the amplified fixture, two about the quiet one --
# which is the negative control, and without it a question that answered yes
# about everything would pass here -- and three about the repository's own
# files. A run that asked fewer asked less than this file claims to ask.
question_floor=7
(( questions >= question_floor )) \
    || fail "only $questions distinct question(s) were asked against a floor of $question_floor, so this run proved less than it claims"

(( wrong_total == 0 )) \
    || fail "$wrong_total of $((questions * asks)) asks came back with an answer the file does not support; a matching question that depends on scheduling is not a gate"

printf 'shell matching determinism: %s question(s), %s asks each across %s concurrent askers, every answer the same\n' \
    "$questions" "$asks" "$workers"
