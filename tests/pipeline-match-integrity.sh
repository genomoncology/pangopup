#!/usr/bin/env bash
set -euo pipefail

# Under `set -o pipefail`, a pipeline that feeds `grep -q` can throw away the
# match it just found.
#
# `grep -q` stops at its first match and closes the pipe. The stage above it is
# still writing, so it dies of SIGPIPE with status 141, and `pipefail` reports
# the pipeline as 141 rather than the 0 that says a match was found. Nothing is
# printed, because that is what `-q` means. The caller reads "no match" and
# carries on. Which way it goes depends on how the two processes happen to be
# scheduled and on how much the upper stage still has to write, so a gate built
# this way answers one way on an idle machine and the other way under load.
#
# Measured on this repository: `tests/shell-spawn-cache-isolation.sh` asks two
# of its questions this way and refused a tree it should accept on 7 of 40
# runs. `tests/shell-matching-determinism.sh` holds the two questions
# themselves. This file holds the shape, everywhere it stands.
#
# --- what counts ------------------------------------------------------------
#
# A pipeline stage that runs `grep` with a quiet option -- `-q`, a cluster
# containing `q`, `--quiet` or `--silent`. Quiet is the form whose exit status
# is the entire answer: there is no output to read, so a discarded status is a
# discarded answer and nothing says so.
#
# What is deliberately outside it, because each of these is a different case
# rather than a smaller one:
#
#   * `grep` without a quiet option, and `grep -c`. Both read to end of input,
#     so neither closes the pipe early and there is nothing to race. A rule
#     that swept them in would be about pipelines rather than about lost
#     matches. `tests/shell-spawn-cache-isolation.sh` holds one of the near
#     misses in the tree: `$(printf ... | grep -c .) -eq 2`, where the option
#     scan has to stop at the pattern or read the `-eq` of the comparison as a
#     quiet option.
#   * `head`, and every other reader that stops early without reporting a
#     match. `head` exits 0 whether it read anything or not, so no answer is
#     carried in its status and none can be lost. What can go wrong there is
#     the *upstream* status being consumed, which is a different defect.
#   * `grep -q` reading a file, a here-string or a process substitution. There
#     is no pipeline, so `pipefail` has nothing to report and the status is the
#     match. These are the forms the repair is written in, and a rule that
#     refused them would leave no way to ask the question at all.
#   * `a || grep -q b`. `||` is not a pipe.
#
# Quoted text is cut out before the scan reads a line, so a file that writes
# the shape into a fixture, or names it in a message, is not refused for
# talking about it. This file writes every one of its own fixtures with
# `printf` and single-quoted arguments for exactly that reason.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

fail() { printf 'pipeline match integrity: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# --- the scan ---------------------------------------------------------------

scan='
# Whether the options standing at the front of $rest -- the text just past a
# `grep` and the space after it -- ask grep to answer with its exit status
# alone. The option scan stops at the first word that is not an option, so a
# pattern is never read as one.
function quiet(rest,   w, word, j) {
    w = split(rest, word, /[[:space:]]+/)
    for (j = 1; j <= w; j++) {
        if (word[j] == "") { continue }
        if (word[j] == "--") { return 0 }
        if (word[j] !~ /^-/) { return 0 }
        if (word[j] ~ /^--(quiet|silent)$/) { return 1 }
        if (word[j] ~ /^-[A-Za-z]*q/) { return 1 }
    }
    return 0
}
FNR == 1 { files++ }
{
    line = $0
    sub(/^[[:space:]]*/, "", line)
    if (line == "" || line ~ /^#/) { next }
    statements++
    code = $0
    gsub(/\047[^\047]*\047/, "", code)
    gsub(/"[^"]*"/, "", code)
    gsub(/\|\|/, "\001", code)

    # Every quiet grep on the line, wherever it stands: in a pipeline, or
    # reading a file, a here-string or a process substitution. This is the
    # count of questions asked, which the repair leaves where it is and a
    # deletion does not.
    rest = code
    while (match(rest, /(^|[^A-Za-z0-9_.\/-])grep[[:space:]]/)) {
        rest = substr(rest, RSTART + RLENGTH)
        if (quiet(rest)) { questions++ }
    }

    n = split(code, segment, "|")
    for (i = 2; i <= n; i++) {
        stages++
        head = segment[i]
        sub(/^[&[:space:]]*/, "", head)
        while (head ~ /^[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+/) {
            sub(/^[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+/, "", head)
        }
        if (head !~ /^grep([[:space:]]|$)/) { continue }
        sub(/^grep/, "", head)
        if (!quiet(head)) { continue }
        printf "%s:%d: %s\n", FILENAME, FNR, line > "/dev/stderr"
        discarded++
    }
}
END { printf "%d %d %d %d %d\n", files, statements, stages, discarded, questions }
'

# `</dev/null` matters: awk with no file operands reads standard input, so a
# scan that was handed no file would block for ever instead of refusing.
SCAN_FILES=0
SCAN_STATEMENTS=0
SCAN_STAGES=0
SCAN_DISCARDED=0
SCAN_QUESTIONS=0
SCAN_SITES=
scan_files() {
    local report=$work/sites
    read -r SCAN_FILES SCAN_STATEMENTS SCAN_STAGES SCAN_DISCARDED SCAN_QUESTIONS \
        < <(awk "$scan" "$@" </dev/null 2>"$report")
    SCAN_SITES=$(<"$report")
}

# --- 1. the scan tells the shapes apart ------------------------------------
#
# Proved against fixtures rather than against whatever the tree happens to say
# today, and in both directions. The accepted half matters as much as the
# refused half: a scan that refused the repair would make the repair
# impossible to write.

accepted=$work/accepted.sh
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'grep -q pattern "$file"' \
    'grep -q pattern <<<"$text"' \
    'grep -qE pattern < <(produce)' \
    'produce | grep -c pattern' \
    'produce | grep -E pattern' \
    'produce | grep -F -- "$needle" >/dev/null' \
    'produce | head -n1' \
    '[[ $(printf "%s\n" "$text" | grep -c .) -eq 2 ]]' \
    'absent || grep -q pattern "$file"' \
    "printf '%s' 'produce | grep -q pattern' >\"\$fixture\"" \
    '# produce | grep -q pattern' \
    >"$accepted"

scan_files "$accepted"
(( SCAN_DISCARDED == 0 )) \
    || fail "the scan refused a file holding only shapes that keep their answer: $SCAN_SITES"
(( SCAN_STAGES == 5 )) \
    || fail "the scan read $SCAN_STAGES pipeline stage(s) in a fixture holding five, so it is not reading the pipelines it claims to read"
# Four questions stand in that fixture: three asked in the forms the repair is
# written in, and one asked after a `||`. The count has to see them, because it
# is what stands between rewriting a question and deleting it. The quoted one
# and the commented one are not questions and are not counted.
(( SCAN_QUESTIONS == 4 )) \
    || fail "the scan counted $SCAN_QUESTIONS quiet grep question(s) in a fixture asking four, so it is not counting the questions a repair must keep"

refused=$work/refused.sh
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'produce | grep -q pattern' \
    'produce | grep -qF -- "$needle" || fail no match' \
    'produce | filter | grep --quiet pattern' \
    'LC_ALL=C produce | grep -q pattern' \
    >"$refused"

scan_files "$refused"
(( SCAN_QUESTIONS == 4 )) \
    || fail "the scan counted $SCAN_QUESTIONS quiet grep question(s) in a fixture asking four, so a refused pipeline is not counted as a question asked"
(( SCAN_DISCARDED == 4 )) \
    || fail "the scan found $SCAN_DISCARDED discarded-answer pipeline(s) in a fixture holding four: $SCAN_SITES"
for line in 2 3 4 5; do
    case "$SCAN_SITES" in
        *"$refused:$line:"*) ;;
        *) fail "the refusal does not name $refused line $line, so a reader is not told where to look: $SCAN_SITES" ;;
    esac
done

# The single letter has to decide. `-c` and `-q` differ by one character and by
# whether the reader stops early, and a scan that read the option cluster
# loosely would refuse both or neither.
one_letter=$work/one-letter.sh
printf '%s\n' '#!/usr/bin/env bash' 'produce | grep -cE pattern' >"$one_letter"
scan_files "$one_letter"
(( SCAN_DISCARDED == 0 )) \
    || fail 'the scan refused a pipeline into grep -cE, which reads to end of input and loses no answer'
printf '%s\n' '#!/usr/bin/env bash' 'produce | grep -qE pattern' >"$one_letter"
scan_files "$one_letter"
(( SCAN_DISCARDED == 1 )) \
    || fail 'the scan accepted a pipeline into grep -qE, so the quiet option is not what it is reading'

# --- 2. the shape really does lose the answer -------------------------------
#
# The rule is not a style preference. Both fixtures below are run, against a
# match that stands at the top of far more output than a pipe buffer holds.
# The refused shape reports 141 and says nothing about the match it found; the
# accepted shape reports the match.

producer=$work/producer.sh
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'printf "%s\n" "the-match"' \
    'for ((i = 0; i < 4000; i++)); do printf "%s %d\n" "filler filler filler filler filler filler" "$i"; done' \
    >"$producer"

lossy=$work/lossy.sh
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'bash "$1" | grep -q the-match' \
    >"$lossy"
kept=$work/kept.sh
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'grep -q the-match < <(bash "$1")' \
    >"$kept"

lossy_status=0
bash "$lossy" "$producer" || lossy_status=$?
kept_status=0
bash "$kept" "$producer" || kept_status=$?
(( kept_status == 0 )) \
    || fail "the shape this rule accepts failed to report a match that stands in its input (status $kept_status), so the fixture, not the rule, is wrong"
(( lossy_status != 0 )) \
    || fail 'a pipeline into grep -q reported the match it found even with a pipe buffer of output behind it, so this machine cannot show the defect the rule is about and the rule would be resting on nothing'

# --- 3. no such pipeline stands in this repository --------------------------

shell_files=()
while IFS= read -r path; do shell_files+=("$path"); done < <(
    find "$repository" -type f -name '*.sh' -not -path '*/target/*' | sort
)
(( ${#shell_files[@]} > 0 )) \
    || fail 'found no shell files to read, so this rule held over nothing'

scan_files "${shell_files[@]}"

# Measured on this tree: 56 shell files, 312 pipeline stages and 289 quiet grep
# questions. The file floor is what the tree holds, so a scan that stopped
# reading part of it is refused.
#
# The stage floor allows for the repair. Rewriting one of the 39 sites below as
# a here-string or a process substitution removes the stage it stood in, and a
# rewrite that folds the producer in as well removes another, so the floor sits
# a little under 311 less those 39 rather than exactly on it.
#
# The question floor is what the stage floor cannot do. Deleting all 39 sites
# outright leaves the stage count inside its floor -- measured: 269 stages, and
# the whole file green -- so the stage count cannot tell a repair from a
# deletion. The question count can: a rewrite keeps the question and a deletion
# drops it. The ticket's boundary is the same rule in words -- this work changes
# whether a gate returns the answer it found, and never what it asks.
file_floor=56
stage_floor=264
question_floor=289
(( SCAN_FILES >= file_floor )) \
    || fail "the scan read $SCAN_FILES shell file(s) against a floor of $file_floor, so it did not read the tree"
(( SCAN_STATEMENTS > 0 )) \
    || fail 'the scan read no statements, so it proved nothing'
(( SCAN_STAGES >= stage_floor )) \
    || fail "the scan read $SCAN_STAGES pipeline stage(s) against a floor of $stage_floor, so it matched less than the repository holds"
(( SCAN_QUESTIONS >= question_floor )) \
    || fail "the repository asks $SCAN_QUESTIONS quiet grep question(s) against a floor of $question_floor, so a question has been deleted rather than rewritten; a pipeline that feeds a quiet grep is repaired by reading its input with < <(...) or a here-string, which keeps the question and removes only the pipeline"

if (( SCAN_DISCARDED != 0 )); then
    printf '%s\n' "$SCAN_SITES" >&2
    fail "$SCAN_DISCARDED pipeline(s) above feed a quiet grep under pipefail, so each throws away the match it finds whenever the stage above it is still writing; read the input with < <(...) or a here-string instead, so there is no pipeline for pipefail to report"
fi

printf 'pipeline match integrity: %s shell file(s), %s pipeline stage(s), %s quiet grep question(s), none of them asked through a pipeline\n' \
    "$SCAN_FILES" "$SCAN_STAGES" "$SCAN_QUESTIONS"
