#!/usr/bin/env bash
set -euo pipefail

# `mustmatch EXPECTED` compares canonical JSON when both sides parse as JSON, so
# a field the product adds to a record fails the pin. `mustmatch like EXPECTED`
# compares a subset, so an added field passes and the block stays green while
# the transcript beside it stops being what the command prints.
#
# Measured against mustmatch 0.1.0 on 2026-09-11:
#
#   printf '{"a":1,"b":2}' | mustmatch      '{"a":1}'   exits 1
#   printf '{"a":1,"b":2}' | mustmatch like '{"a":1}'   exits 0
#
# Measured over this repository on 2026-09-11, one pin at a time so that no
# block's early exit hid another: spec/ carries 24 JSON-object pins written
# with `like`. Nineteen of them already name every field the command prints and
# pass unchanged under the strict form. Two -- spec/snv-lookup.md and
# spec/model-routing.md -- pin a whole lookup record whose `provenance` lacks
# the `software_version` the product has emitted since 2026-09-09, and a reader
# takes those blocks as the bytes the command prints. Three stand in one block
# of spec/full-bundle.md and are deliberate three-of-eleven-field excerpts
# beneath a paragraph about determinism, where the eight omitted counts are
# beside the point.
#
# This file holds one claim.
#
#   Every JSON-object pin in spec/ compares the complete record, unless the
#   block says in the file that the pin is partial and why.
#
# The declaration is a comment line standing immediately above the pin, inside
# the same fenced block:
#
#     # partial: <why this pin names only part of the record>
#     cmd | mustmatch like '{...}'
#
# It is a comment so that mustmatch runs the block unchanged, and it stands in
# the spec file so that the reader who meets the excerpt meets the reason for
# it in the same place.
#
# Four things follow, and each is refused here:
#
#   1. A `like` object pin with no declaration above it. That is the defect:
#      a transcript presented as a whole record that a new field slips past.
#   2. A declaration whose reason is missing or is a stub. A stub is one of two
#      shapes: shorter than three words, or built only out of words that
#      restate the declaration. "It is partial" and "this pin is partial" both
#      clear three words and say nothing a reader could not see from the `like`
#      on the line below them. Whether a longer reason is true is a review
#      matter and not something a scan can settle.
#   3. A declaration standing above a strict pin. A declaration that exempts
#      nothing is a reader misled about what the block checks.
#   4. A declaration outside spec/full-bundle.md, or more than the three that
#      block carries. An exemption is the thing this rule exists to bound, so
#      the set of files allowed to hold one is named rather than discovered.
#      The bound is a ceiling and not a floor: making one of those three pins
#      complete and dropping its declaration is a stronger repair than the one
#      this ticket asks for and must stay possible.
#
# And the scan refuses to pass on nothing: spec/ carried 24 JSON-object pins
# when this was written, counted from the tree, so a scan reading fewer than
# that is reading a repository where pins were deleted rather than completed.
#
# What this does not prove: that a strict pin names the right record, or that a
# declared reason is true. `make spec` proves the first by running every block;
# the second is a review matter. Nor does this run any spec block -- it reads
# spec/ as text, so it costs no build and no model.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# Where a partial pin may stand, and how many it may hold. Measured from the
# tree on 2026-09-11: one block of spec/full-bundle.md carries three pins of
# `pangopup-build build` output naming three of the eleven fields it prints.
declared_file='spec/full-bundle.md'
declared_ceiling=3

# The number of JSON-object pins spec/ carried when this rule was written,
# strict and `like` together. A repair that deletes pins instead of completing
# them drops below it.
minimum_object_pins=24

# A declared reason has to be a reason. Three words is the shortest thing that
# can be one, and a reason assembled only out of the words below restates the
# declaration instead of giving one: "it is partial" and "this pin is partial"
# both clear the word count and tell the reader nothing the `like` on the next
# line does not. A reason has to carry at least one word from outside this set,
# which is what naming the omitted fields, or the point of the block, does.
minimum_reason_words=3
stock_reason_words='it|is|are|was|a|an|the|this|that|these|those|pin|pins|pinned|partial|partially|excerpt|excerpts|subset|part|parts|here|above|below|only|of|and|some|record|records|block|blocks|field|fields'

fail() { printf 'spec record pin completeness: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# One record per JSON-object pin, as
# "<file>\t<line>\t<kind>\t<declared>\t<reason>", where <kind> is `like` or
# `strict` and <declared> is 1 when the line immediately above the pin, inside
# the same fenced block, is a `# partial: <reason>` comment.
#
# A pin is a line carrying `mustmatch '{` or `mustmatch like '{`: the argument
# opens a JSON object, which is the shape an added field slips into. A pin of a
# fragment -- a run of keys with no enclosing brace, narrowed by an `rg -o`
# earlier in the pipeline -- is a fragment by construction and is not what this
# rule is about.
#
# `</dev/null` matters: awk with no file operands reads standard input, so a
# scan handed no file would wait for ever instead of refusing.
pin_records() {
    awk '
        FNR == 1 { inblock = 0; previous = "" }
        /^[[:space:]]*```/ {
            inblock = !inblock
            previous = ""
            next
        }
        !inblock { previous = ""; next }
        {
            line = $0
            declared = 0
            reason = ""
            if (match(previous, /^[[:space:]]*#[[:space:]]*partial:/)) {
                declared = 1
                reason = previous
                sub(/^[[:space:]]*#[[:space:]]*partial:[[:space:]]*/, "", reason)
                sub(/[[:space:]]+$/, "", reason)
            }
            if (index(line, "mustmatch like \047{") > 0)
                printf "%s\t%d\tlike\t%d\t%s\n", FILENAME, FNR, declared, reason
            else if (index(line, "mustmatch \047{") > 0)
                printf "%s\t%d\tstrict\t%d\t%s\n", FILENAME, FNR, declared, reason
            previous = line
        }
    ' "$@" </dev/null
}

# --- 1. the scan reads a declaration, and reads its absence -----------------
#
# Proved against a fixture first, so the reading stays legible and does not
# depend on what spec/ happens to say today. The accepted half matters as much
# as the refused half: a scan that read every pin as undeclared would make the
# repair impossible to write, and one that read every pin as declared would
# hold nothing.
fixture="$work/fixture.md"
{
    printf '# fixture\n\n'
    printf '```bash\n'
    printf "cmd | mustmatch '{\"a\":1}'\n"
    printf '# partial: the eight counts below are beside the point here\n'
    printf "cmd | mustmatch like '{\"a\":1}'\n"
    printf "cmd | mustmatch like '{\"b\":2}'\n"
    printf '# partial: x\n'
    printf "cmd | mustmatch like '{\"c\":3}'\n"
    printf '# partial: this declaration exempts nothing at all\n'
    printf "cmd | mustmatch '{\"d\":4}'\n"
    printf '```\n\n'
    printf '# partial: a declaration outside a block exempts nothing\n'
    printf '```bash\n'
    printf "cmd | mustmatch like '{\"e\":5}'\n"
    printf '```\n\n'
    printf '```bash\n'
    printf "cmd | rg -o '\"f\":6' | mustmatch like '\"f\":6'\n"
    printf '```\n'
}>"$fixture"

fixture_records=$(pin_records "$fixture")
fixture_seen=$(printf '%s\n' "$fixture_records" | grep -c . || true)
[[ "$fixture_seen" == 6 ]] \
    || fail "the scan read $fixture_seen of the 6 JSON-object pins in its own fixture, so it does not see every pin; the seventh line pins a fragment and is not one"

fixture_shape=$(printf '%s\n' "$fixture_records" | awk -F'\t' '{ printf "%d:%s:%d ", $2, $3, $4 }')
[[ "$fixture_shape" == '4:strict:0 6:like:1 7:like:0 9:like:1 11:strict:1 16:like:0 ' ]] \
    || fail "the scan read its own fixture as \"$fixture_shape\", and the six pins are: line 4 strict and undeclared, line 6 like and declared, line 7 like and undeclared, line 9 like and declared with a stub reason, line 11 strict and declared, line 16 like and undeclared because the declaration above it stands outside the block"

fixture_reason=$(printf '%s\n' "$fixture_records" | awk -F'\t' '$2 == 6 { print $5 }')
[[ "$fixture_reason" == 'the eight counts below are beside the point here' ]] \
    || fail "the scan read the declared reason on line 6 of its own fixture as \"$fixture_reason\", so it cannot tell a reason from a stub"

# A reason has to say something the `like` on the next line does not. Both
# halves are proved here, because a rule that read every reason as a stub would
# make the declaration impossible to write.
is_stub() {
    local candidate=$1 count
    count=$(printf '%s\n' "$candidate" | wc -w | tr -d ' ')
    (( count >= minimum_reason_words )) || return 0
    printf '%s\n' "$candidate" | tr -cs "[:alnum:]_" '\n' \
        | grep -Eqiv "^($stock_reason_words)$" && return 1
    return 0
}
for meaningless in 'it is partial' 'this pin is partial' 'partial pin here' \
    'only part of the record' 'x y'
do
    is_stub "$meaningless" \
        || fail "the scan reads \"$meaningless\" as a reason, so a pin can be exempted by restating the declaration"
done
for genuine in 'the eight counts below are beside the point here' \
    'the block proves determinism and the counts are proved above' \
    'only the build status and identity matter to this comparison'
do
    is_stub "$genuine" \
        && fail "the scan reads \"$genuine\" as a stub, so no declaration could ever be written"
done

# --- 2. every JSON-object pin in spec/ --------------------------------------
shopt -s nullglob
spec_files=("$repository"/spec/*.md)
shopt -u nullglob
(( ${#spec_files[@]} > 0 )) || fail 'found no spec/*.md files to inspect, so this check proved nothing'

records=$(pin_records "${spec_files[@]}")
total=$(printf '%s\n' "$records" | grep -c . || true)
(( total >= minimum_object_pins )) \
    || fail "read $total JSON-object pin(s) out of ${#spec_files[@]} spec file(s), fewer than the $minimum_object_pins this repository carried when the rule was written; a pin deleted is a record nothing holds, which is what the rule exists to stop"

undeclared=
stub=
pointless=
declared_count=0
declared_elsewhere=
while IFS=$'\t' read -r file line kind declared reason; do
    [[ -n "$file" ]] || continue
    relative=${file#"$repository"/}
    if [[ "$kind" == like && "$declared" == 0 ]]; then
        undeclared+="  $relative:$line"$'\n'
        continue
    fi
    [[ "$declared" == 1 ]] || continue
    if [[ "$kind" == strict ]]; then
        pointless+="  $relative:$line"$'\n'
        continue
    fi
    words=$(printf '%s\n' "$reason" | wc -w | tr -d ' ')
    if (( words < minimum_reason_words )); then
        stub+="  $relative:$line: \"$reason\""$'\n'
        continue
    fi
    if ! printf '%s\n' "$reason" | tr -cs "[:alnum:]_" '\n' \
        | grep -Eqiv "^($stock_reason_words)$"; then
        stub+="  $relative:$line: \"$reason\""$'\n'
        continue
    fi
    declared_count=$((declared_count + 1))
    [[ "$relative" == "$declared_file" ]] || declared_elsewhere+="  $relative:$line"$'\n'
done <<<"$records"

if [[ -n "$undeclared" ]]; then
    printf 'spec record pin completeness: these blocks present a whole record as the bytes a command prints and pin it with `mustmatch like`, which compares a subset, so a field the product adds to that record passes unnoticed and the transcript stops being what the command prints:\n' >&2
    printf '%s' "$undeclared" >&2
    printf 'Write each as `mustmatch` so the complete record is compared, or put `# partial: <why>` on the line above it inside the block.\n' >&2
    exit 1
fi

if [[ -n "$stub" ]]; then
    printf 'spec record pin completeness: these pins are declared partial without saying why, and a reader who meets the excerpt still cannot tell what was left out or on what grounds:\n' >&2
    printf '%s' "$stub" >&2
    printf 'Write at least %s words saying what the pin leaves out and why that is beside the point of the block, in words that restate neither `partial` nor the excerpt itself.\n' "$minimum_reason_words" >&2
    exit 1
fi

if [[ -n "$pointless" ]]; then
    printf 'spec record pin completeness: these pins compare the complete record and carry a `# partial:` declaration above them, so the file tells a reader the block checks less than it does:\n' >&2
    printf '%s' "$pointless" >&2
    printf 'Remove the declaration.\n' >&2
    exit 1
fi

if [[ -n "$declared_elsewhere" ]]; then
    printf 'spec record pin completeness: a pin may be declared partial only in %s, and these stand elsewhere:\n' "$declared_file" >&2
    printf '%s' "$declared_elsewhere" >&2
    printf 'Pin the complete record, or bring the exemption to this file so that a second place holding one is a change the rule is told about.\n' >&2
    exit 1
fi

(( declared_count <= declared_ceiling )) \
    || fail "$declared_count pin(s) in $declared_file are declared partial, more than the $declared_ceiling that file carried when the rule was written; an exemption is what this rule exists to bound, so a new one is a change the rule has to be told about"

strict=$(printf '%s\n' "$records" | awk -F'\t' '$3 == "strict"' | grep -c . || true)
(( strict > 0 )) \
    || fail "read $total JSON-object pin(s) in spec/ and not one of them compares a complete record, so this rule held over nothing"

printf 'spec record pin completeness: %s JSON-object pin(s) in %s spec file(s), %s comparing the complete record, %s declared partial in %s\n' \
    "$total" "${#spec_files[@]}" "$strict" "$declared_count" "$declared_file"
