#!/usr/bin/env bash
set -euo pipefail

# A long Rust string literal is wrapped by hand: a `\` ends the line and the
# next line is indented under the opening quote, so the runtime text reads as
# one sentence. When the `\` goes missing the wrap collapses -- the indentation
# survives inside the literal as a run of internal spaces, and the message
# prints with a gap in the middle of it. It compiles, it asserts the same
# thing, and it reads wrong the moment anyone sees it. Five assertion messages
# in `crates/pangopup-cache/src/lib.rs` arrived that way and were repaired
# under ticket 0075, caught by eye.
#
# No gate saw them. `cargo fmt` does not reflow the contents of a literal,
# clippy has no lint for it, and the dependency policy reads no source text.
# The pattern is mechanical, so it belongs to a gate rather than to a reader:
#
#   three or more spaces standing between two word characters, inside a Rust
#   source file under crates/.
#
# Word characters on both sides is what keeps the rule the width of the thing
# it covers. Aligned trailing comments, aligned `=>` arms and a doc comment's
# indented block all put a run of spaces next to punctuation or at the start of
# a line, and none of them is a collapsed wrap.
#
# One deliberate case in the tree survives it, and it is exempted by name
# below rather than by shape.
#
# What this does not prove: that a wrapped literal reads well, or that a
# literal written as one long line says the right thing. It catches the one
# accident that leaves a mechanical trace.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

fail() { printf 'rust literal continuity: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

collapsed='[[:alnum:]_] {3,}[[:alnum:]_]'

# The one deliberate run of internal spaces in the crate tree: the usage text
# `pangopup-build` prints for its legacy actions, whose continuation lines are
# aligned under the first line's `Usage: `. The alignment is what the operator
# sees, so it is the text and not an accident of wrapping.
#
# The exemption is a path and the opening of a line, both compared as literal
# text. Neither half exempts on its own. A second collapsed literal anywhere
# else in that file is refused, and the same declaration copied into another
# file is refused, and section 1 proves both. A pattern like
# `LEGACY_USAGE` on its own would exempt every file that declared one.
exempt_path='crates/pangopup-build/src/main.rs'
exempt_opening='const LEGACY_USAGE: &str = '

# The collapsed-wrap lines in the files named, as "<path>\t<line>\t<text>",
# with the one exempted declaration dropped. Paths are relative to $1.
collapsed_lines() {
    local root=$1
    shift
    { grep -nE -- "$collapsed" "$@" || true; } \
        | awk -F: -v root="$root/" -v path="$exempt_path" -v opening="$exempt_opening" '
            {
                file = $1
                line = $2
                text = substr($0, length($1) + length($2) + 3)
                sub("^" root, "", file)
                stripped = text
                sub(/^[[:space:]]*/, "", stripped)
                if (file == path && index(stripped, opening) == 1) next
                printf "%s\t%s\t%s\n", file, line, text
            }
        '
}

# --- 1. the rule is the width of the thing it covers ------------------------
#
# Proved against fixtures before the tree is read, so the reading stays legible
# and the accepted half is held as firmly as the refused half. A rule that
# refused an aligned match arm would be unwritable against; one that exempted
# by shape rather than by name would let the accident back in wherever the
# shape recurred.
tree="$work/tree"
mkdir -p "$tree/$(dirname "$exempt_path")" "$tree/crates/other/src"

# The accident: a continuation line whose `\` went missing.
{
    printf 'fn probe() {\n'
    printf '    assert!(\n'
    printf '        ok,\n'
    printf '        "the recorded setup no longer matches   so the cache was discarded"\n'
    printf '    );\n'
    printf '}\n'
} >"$tree/crates/other/src/collapsed.rs"

# Shapes that are not the accident, each of which puts a run of spaces
# somewhere other than between two word characters.
{
    printf 'const A: u8 = 1;   // aligned trailing comment\n'
    printf '/// A doc comment with an indented block:\n'
    printf '///     indented sample text\n'
    printf 'fn arms(v: u8) -> u8 {\n'
    printf '    match v {\n'
    printf '        1   => 2,\n'
    printf '        _   => 0,\n'
    printf '    }\n'
    printf '}\n'
} >"$tree/crates/other/src/aligned.rs"

# The exempted declaration, and beside it the same shape under another name in
# the same file.
{
    printf 'const LEGACY_USAGE: &str = "Usage: pangopup-build inspect <DIR>\\n       pangopup-build open <ARTIFACT>";\n'
    printf 'const OTHER_USAGE: &str = "Usage: pangopup-build inspect <DIR>\\n       pangopup-build open <ARTIFACT>";\n'
} >"$tree/$exempt_path"

# The exempted declaration copied into a file it does not belong to.
printf 'const LEGACY_USAGE: &str = "Usage: elsewhere inspect <DIR>\\n       elsewhere open <ARTIFACT>";\n' \
    >"$tree/crates/other/src/borrowed.rs"

fixture_sources=()
while IFS= read -r source; do fixture_sources+=("$source"); done < <(
    find "$tree" -type f -name '*.rs' | sort
)
(( ${#fixture_sources[@]} == 4 )) \
    || fail "planted 4 fixture sources and found ${#fixture_sources[@]}, so the fixture proof below reads the wrong set"

fixture_found=$(collapsed_lines "$tree" "${fixture_sources[@]}")
fixture_paths=$(cut -f1 <<<"$fixture_found" | sort -u)

for expected in \
    'crates/other/src/collapsed.rs' \
    'crates/other/src/borrowed.rs' \
    "$exempt_path"; do
    grep -Fqx "$expected" <<<"$fixture_paths" \
        || fail "the scan did not refuse $expected, so it does not see the accident it exists to catch: $fixture_found"
done

if grep -Fqx 'crates/other/src/aligned.rs' <<<"$fixture_paths"; then
    fail "the scan refused an aligned comment, match arm or indented doc block, so it forbids more than a collapsed wrap: $fixture_found"
fi

# In the exempt file, the exempted declaration is gone from the report and the
# one beside it is not: the exemption covers the declaration, not the file.
exempt_hits=$(awk -F'\t' -v path="$exempt_path" '$1 == path' <<<"$fixture_found")
[[ "$(printf '%s' "$exempt_hits" | grep -c . || true)" == 1 ]] \
    || fail "the exemption covers more than the one declaration it names: $exempt_hits"
[[ "$exempt_hits" == *OTHER_USAGE* ]] \
    || fail "the exemption dropped the wrong line in $exempt_path: $exempt_hits"

# --- 2. the exemption still names something that is there -------------------
#
# An exemption for a declaration that has been renamed or removed is an
# exemption nobody reads, and the next line to match its opening inherits it.
grep -Fq -- "$exempt_opening" "$repository/$exempt_path" \
    || fail "the exemption names '$exempt_opening' in $exempt_path and that declaration is no longer there, so the exemption covers whatever matches it next -- remove the exemption or point it at the declaration that replaced it"

# --- 3. the crate tree carries the accident nowhere else --------------------
sources=()
while IFS= read -r source; do sources+=("$source"); done < <(
    find "$repository/crates" -type f -name '*.rs' -not -path '*/target/*' | sort
)
(( ${#sources[@]} > 0 )) \
    || fail 'found no Rust source under crates/, so this check inspected nothing'

found=$(collapsed_lines "$repository" "${sources[@]}")
if [[ -n "$found" ]]; then
    printf 'rust literal continuity: these lines carry three or more spaces between two word characters, which is what a hand-wrapped string literal leaves behind when its trailing `\\` goes missing:\n' >&2
    printf '%s\n' "$found" | awk -F'\t' '{ printf "  %s:%s: %s\n", $1, $2, $3 }' >&2
    printf 'Restore the continuation, or write the literal on one line.\n' >&2
    exit 1
fi

printf 'rust literal continuity: %s Rust source(s) under crates/, no collapsed wrap, 1 declaration exempted by name\n' \
    "${#sources[@]}"
