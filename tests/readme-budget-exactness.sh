#!/usr/bin/env bash
set -euo pipefail

# The README size budget in `spec/readme-first-use.md` is a ceiling, and a
# ceiling standing above the file it measures has stopped refusing anything.
# Measured on 2026-09-11 on the tree before this harness: the README stood at
# 260 lines and 1766 words under a cap of 270 and 1770, so ten lines and four
# words could be added and no gate would say so. The numbers described no tree
# anybody had seen.
#
# This file holds the budget to the file. Every figure the README contract pins
# -- the line cap, the word cap, the heading list and the image-mark count --
# equals what `README.md` measures right now, so the next added line, word,
# heading or image turns a gate red. Raising a cap then means stating a new
# measurement of a real tree, rather than inheriting slack.
#
# The image-mark count is pinned twice, in `spec/readme-first-use.md` and in
# `scripts/check-readme-images.sh`. Both are read here, because two pins that
# disagree leave one of them passing on a README the other refuses.
#
# The figures are read out of those two files by the exact shape they are
# written in. A pin that is deleted, renamed, or rewritten into another
# comparison is not found, and a figure that is not found is refused by name
# rather than skipped -- a scan that passes on nothing is the defect this file
# exists to stop.
#
# Sections 2 and 3 prove the refusals instead of asserting them. Section 2 grows
# a copy of the README by one line, by one word, by one heading and by one
# image, each mutation leaving every other figure untouched, and requires the
# figure it grew past to refuse it by name. Section 3 raises a copy of the
# contract's line cap by one above the file and requires that to be refused too,
# which is the direction the slack came from.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# shellcheck source=tests/support/expected-text.sh
. "$repository/tests/support/expected-text.sh"

readme="$repository/README.md"
contract="$repository/spec/readme-first-use.md"
image_gate="$repository/scripts/check-readme-images.sh"

fail() { printf 'README budget exactness: %s\n' "$*" >&2; exit 1; }

for file in "$readme" "$contract" "$image_gate"; do
    [[ -f "$file" ]] || fail "missing $file"
done

measure_lines() { wc -l < "$1" | tr -d '[:space:]'; }
measure_words() { wc -w < "$1" | tr -d '[:space:]'; }
measure_images() { grep -Fo '![' "$1" | wc -l | tr -d '[:space:]'; }
measure_headings() { grep '^## ' "$1"; }

pinned_figure() {
    local file=$1 pattern=$2 description=$3 figure
    figure=$(sed -nE "$pattern" "$file")
    [[ "$figure" =~ ^[0-9]+$ ]] \
        || fail "could not read the pinned $description out of $file"
    printf '%s\n' "$figure"
}

# The heading list is the printf continuation block under the `$headings`
# comparison, and it ends on the first line that carries no continuation. Read
# by position rather than by shape, because the forbidden-phrase list further
# down the same file quotes a heading of its own.
pinned_headings() {
    local file=$1 headings
    headings=$(
        awk 'index($0, "test \"$headings\" = ") == 1 { on = 1; next }
             on { print; if (!/\\$/) exit }' "$file" \
            | sed -E "s/^  '//; s/' \\\\$//; s/'\)\"$//"
    )
    [[ -n "$headings" ]] \
        || fail "could not read the pinned heading list out of $file"
    printf '%s\n' "$headings"
}

# Refuses by name through `equal`, which exits, so every call site is either the
# real check or a subshell that expects the refusal.
check_figures() {
    local file=$1 spec=$2 gate=$3 marks
    marks=$(measure_images "$file")
    equal 'the README line cap' "$(measure_lines "$file")" \
        "$(pinned_figure "$spec" \
            's/^test "\$\(wc -l < \.\.\/README\.md\)" -le ([0-9]+)$/\1/p' 'line cap')"
    equal 'the README word cap' "$(measure_words "$file")" \
        "$(pinned_figure "$spec" \
            's/^test "\$\(wc -w < \.\.\/README\.md\)" -le ([0-9]+)$/\1/p' 'word cap')"
    equal 'the contract image-mark count' "$marks" \
        "$(pinned_figure "$spec" \
            "s/^test \"\\\$\(grep -Fo '!\\[' \\.\\.\\/README\\.md \\| wc -l \\| tr -d '\\[:space:\\]'\)\" = ([0-9]+)\$/\\1/p" \
            'image-mark count')"
    equal 'the image gate image-mark count' "$marks" \
        "$(pinned_figure "$gate" \
            's/^\[\[ "\$markdown_image_count" == ([0-9]+) \]\] \\$/\1/p' 'image-mark count')"
    equal 'the pinned heading list' "$(measure_headings "$file")" \
        "$(pinned_headings "$spec")"
}

# --- 1. every pinned figure is the measurement of the file it governs -------

check_figures "$readme" "$contract" "$image_gate"

# --- 2. a README grown past a figure is refused by that figure --------------

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mutant="$work/README.md"

refuses() {
    local named=$1 file=$2 spec=$3 gate=$4 what=$5 report
    if report=$(check_figures "$file" "$spec" "$gate" 2>&1); then
        fail "$what is admitted"
    fi
    grep -Fq -- "$named" <<<"$report" \
        || fail "$what was refused, but not by $named: $report"
}

# A blank line adds a line and no word, so the line cap answers alone.
cp "$readme" "$mutant"
printf '\n' >> "$mutant"
refuses 'the README line cap' "$mutant" "$contract" "$image_gate" 'an added line'

# Text after the final newline adds a word and no line, so the word cap answers
# alone.
cp "$readme" "$mutant"
printf 'unbudgeted' >> "$mutant"
refuses 'the README word cap' "$mutant" "$contract" "$image_gate" 'an added word'

# A renamed heading changes neither count, so the heading list answers alone.
sed 's/^## Docker$/## Containers/' "$readme" > "$mutant"
refuses 'the pinned heading list' "$mutant" "$contract" "$image_gate" 'a renamed heading'

# One word standing where one word stood, carrying an image mark, so the two
# image-mark counts answer alone.
sed 's/^<details>$/![added](docs\/images\/pangopup.svg)/' "$readme" > "$mutant"
refuses 'the contract image-mark count' "$mutant" "$contract" "$image_gate" 'an added image'

# The same mutant against a contract whose own count was raised to admit it, so
# that the second pin is the one left to refuse it.
admits="$work/contract-admits-the-image.md"
awk -v marks="$(measure_images "$mutant")" \
    'index($0, "test \"$(grep -Fo \x27![\x27 ../README.md | wc -l | tr -d \x27[:space:]\x27)\" = ") == 1 {
         printf "test \"$(grep -Fo \x27![\x27 ../README.md | wc -l | tr -d \x27[:space:]\x27)\" = %s\n", marks; next
     } { print }' "$contract" > "$admits"
refuses 'the image gate image-mark count' "$mutant" "$admits" "$image_gate" 'an added image'

# --- 3. a cap raised above the README is refused -----------------------------

raised="$work/contract-raised.md"
awk -v raise="$(( $(measure_lines "$readme") + 1 ))" \
    'index($0, "test \"$(wc -l < ../README.md)\" -le ") == 1 {
         printf "test \"$(wc -l < ../README.md)\" -le %s\n", raise; next
     } { print }' "$contract" > "$raised"
refuses 'the README line cap' "$readme" "$raised" "$image_gate" \
    'a line cap raised one above the README'

printf 'README budget figures equal the file and refuse growth\n'
