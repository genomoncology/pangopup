#!/usr/bin/env bash
set -euo pipefail

# The README size budget in `spec/readme-first-use.md` is a ceiling, and a
# ceiling raised above the file it measures stops refusing anything. Measured on
# 2026-09-11 on the tree before this harness: the README stood at 260 lines and
# 1766 words under a cap of 270 and 1770, so ten lines and four words could be
# added without any gate noticing, and the numbers recorded no tree anybody had
# seen.
#
# This file holds the budget to the file. Every pinned figure the README
# contract compares against -- the line cap, the word cap, the heading list and
# the image-mark count -- equals what the README measures right now, so the next
# added line, word, heading or image turns a gate red. Raising a cap is then a
# deliberate edit that states a new measurement, not slack somebody inherited.
#
# The image-mark count is pinned twice, in `spec/readme-first-use.md` and in
# `scripts/check-readme-images.sh`. Both are read here, because two pins that
# disagree leave one of them passing on a file the other refuses.
#
# Section 2 proves the refusals rather than asserting them: it grows a copy of
# the README by one line, by one word, by one heading and by one image, and
# requires each pinned figure to refuse its mutant.

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

# --- 1. every pinned figure is the measurement of the file it governs -------

pinned_figure() {
    local file=$1 pattern=$2 description=$3 figure
    figure=$(sed -nE "$pattern" "$file")
    [[ "$figure" =~ ^[0-9]+$ ]] \
        || fail "could not read the pinned $description out of $file"
    printf '%s\n' "$figure"
}

cap_lines=$(pinned_figure "$contract" \
    's/^test "\$\(wc -l < \.\.\/README\.md\)" -le ([0-9]+)$/\1/p' 'line cap')
cap_words=$(pinned_figure "$contract" \
    's/^test "\$\(wc -w < \.\.\/README\.md\)" -le ([0-9]+)$/\1/p' 'word cap')
contract_images=$(pinned_figure "$contract" \
    "s/^test \"\\\$\(grep -Fo '!\\[' \\.\\.\\/README\\.md \\| wc -l \\| tr -d '\\[:space:\\]'\)\" = ([0-9]+)\$/\\1/p" \
    'image-mark count')
gate_images=$(pinned_figure "$image_gate" \
    's/^\[\[ "\$markdown_image_count" == ([0-9]+) \]\] \\$/\1/p' 'image-mark count')

# The heading list is the printf continuation block under the `$headings`
# comparison, and it ends on the line that closes the substitution. Read by
# position rather than by shape, because the forbidden-phrase list further down
# the same file quotes a heading too.
pinned_headings=$(
    awk 'index($0, "test \"$headings\" = ") == 1 { on = 1; next }
         on { print; if (!/\\$/) exit }' "$contract" \
        | sed -E "s/^  '//; s/' \\\\$//; s/'\)\"$//"
)
[[ -n "$pinned_headings" ]] \
    || fail "could not read the pinned heading list out of $contract"

measure_lines() { wc -l < "$1" | tr -d '[:space:]'; }
measure_words() { wc -w < "$1" | tr -d '[:space:]'; }
measure_images() { grep -Fo '![' "$1" | wc -l | tr -d '[:space:]'; }
measure_headings() { grep '^## ' "$1"; }

equal 'the README line cap' "$(measure_lines "$readme")" "$cap_lines"
equal 'the README word cap' "$(measure_words "$readme")" "$cap_words"
equal 'the contract image-mark count' "$(measure_images "$readme")" "$contract_images"
equal 'the image gate image-mark count' "$(measure_images "$readme")" "$gate_images"
equal 'the pinned heading list' "$(measure_headings "$readme")" "$pinned_headings"

# --- 2. each pinned figure refuses the growth it is there to refuse ---------

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mutant="$work/README.md"

grow() {
    cp "$readme" "$mutant"
    printf '%s' "$1" >> "$mutant"
}

# A blank line adds a line and no word, so the line cap answers alone.
grow $'\n'
equal 'the word cap under one added line' "$cap_words" "$(measure_words "$mutant")"
(( $(measure_lines "$mutant") > cap_lines )) \
    || fail 'the line cap admits an added line'

# Text after the final newline adds a word and no line, so the word cap answers
# alone.
grow 'unbudgeted'
equal 'the line cap under one added word' "$cap_lines" "$(measure_lines "$mutant")"
(( $(measure_words "$mutant") > cap_words )) \
    || fail 'the word cap admits an added word'

# The sentence the ticket asks about trips both.
grow $'\nOne more sentence, added beyond the budget.\n'
(( $(measure_lines "$mutant") > cap_lines && $(measure_words "$mutant") > cap_words )) \
    || fail 'the size caps admit an added sentence'

grow $'\n## Added heading\n'
[[ "$(measure_headings "$mutant")" != "$pinned_headings" ]] \
    || fail 'the pinned heading list admits an added heading'

grow $'\n![Added image](docs/images/pangopup.svg)\n'
[[ "$(measure_images "$mutant")" != "$contract_images" ]] \
    || fail 'the contract image-mark count admits an added image'
[[ "$(measure_images "$mutant")" != "$gate_images" ]] \
    || fail 'the image gate image-mark count admits an added image'

printf 'README budget figures equal the file and refuse growth\n'
