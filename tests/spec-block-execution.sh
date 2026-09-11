#!/usr/bin/env bash
set -euo pipefail

# mustmatch runs a plain ```bash block only when the block calls a `mustmatch`
# command. A block that ends without one is reported `SKIP` and not one of its
# lines runs. Measured against mustmatch directly: a two-line block whose first
# line is `false` and whose second writes to standard error reported
# `1 passed, 1 skipped` and exited 0, having written nothing.
#
# That is a claim written into the spec that no gate reads. The summary counts
# the block as a skip beside the passes, so a gate log says a spec file passed
# while naming nothing that went unchecked. Ticket 0080 found
# `spec/runtime-transport.md:79`, the block holding the transport-tamper
# claims, which had never executed a line.
#
# This file holds one claim.
#
#   Every fenced block in spec/ whose info string is exactly `bash` calls a
#   `mustmatch` command, so mustmatch runs it.
#
# The rule reads `bash` exactly rather than by prefix. A ```bash fence carrying
# attributes -- `run id=... exit=...` -- brings its own expectation, so
# mustmatch runs it and checks its status whether or not it calls mustmatch.
# The plain fence is the only one with nothing to make mustmatch open it.
#
# What this does not prove: that the assertions inside those blocks hold.
# `make spec` runs them. Nor that a block's assertion is the right one --
# a block calling `mustmatch` on something unrelated satisfies this rule and
# is a review problem rather than a mechanical one.
#
# `tests/spec-refutation-evidence.sh` holds the narrower form of the same rule
# for blocks carrying a refutation. This one covers every block.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

fail() { printf 'spec block execution: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# One record per fenced block, as "<file>\t<fence line>\t<info>\t<has
# mustmatch>\t<body lines>". The info string is what stands after the three
# backticks; a fence with nothing after it closes a block rather than opening
# one.
#
# `</dev/null` matters: awk with no file operands reads standard input, so a
# scan handed no file would wait for ever instead of refusing.
block_records() {
    awk '
        FNR == 1 { inblock = 0 }
        /^[[:space:]]*```/ {
            if (inblock) {
                printf "%s\t%d\t%s\t%d\t%d\n", FILENAME, fence, info, has, body
                inblock = 0
                next
            }
            info = $0
            sub(/^[[:space:]]*```/, "", info)
            sub(/[[:space:]]+$/, "", info)
            if (info == "") next
            inblock = 1
            fence = FNR
            has = 0
            body = 0
            next
        }
        !inblock { next }
        {
            body++
            if ($0 ~ /(^|[^[:alnum:]_.\/-])mustmatch[[:space:]]/) has = 1
        }
        END { if (inblock) printf "%s\t%d\t%s\t%d\t%d\n", FILENAME, fence, info, has, body }
    ' "$@" </dev/null
}

# The blocks in the files named that mustmatch would skip whole: a plain `bash`
# fence with no mustmatch call in it.
skipped_blocks() {
    block_records "$@" | awk -F'\t' '$3 == "bash" && $4 == 0 { print $1 "\t" $2 "\t" $5 }'
}

# --- 1. the scan tells a run block from a skipped one -----------------------
#
# Proved against fixtures first, so the reading stays legible and does not
# depend on what the spec files happen to say today. The accepted half matters
# as much as the refused half: a scan that refused every plain block would make
# the repair impossible to write.
fixture="$work/fixture.md"
{
    printf '# fixture\n\n'
    printf '```bash\nfalse\n```\n\n'
    printf '```bash\nprintf x | mustmatch x\n```\n\n'
    printf '```bash run id=example exit=2 stream=stderr\nfalse\n```\n\n'
    printf '```text\nnot a command at all\n```\n'
} >"$fixture"

fixture_skipped=$(skipped_blocks "$fixture")
[[ "$(printf '%s' "$fixture_skipped" | grep -c . || true)" == 1 ]] \
    || fail "the scan read $(printf '%s' "$fixture_skipped" | grep -c . || true) skipped block(s) in a fixture holding exactly one, so it is not reading the shape it exists to refuse: $fixture_skipped"
[[ "$(cut -f2 <<<"$fixture_skipped")" == 3 ]] \
    || fail "the scan named line $(cut -f2 <<<"$fixture_skipped") as the skipped block in a fixture whose skipped block opens at line 3, so its refusal would send a reader to the wrong place"

fixture_blocks=$(block_records "$fixture" | grep -c . || true)
[[ "$fixture_blocks" == 4 ]] \
    || fail "the scan read $fixture_blocks of the 4 fenced blocks in its own fixture, so it does not see every block"

# --- 2. every plain bash block in spec/ runs --------------------------------
shopt -s nullglob
spec_files=("$repository"/spec/*.md)
shopt -u nullglob
(( ${#spec_files[@]} > 0 )) || fail 'found no spec/*.md files to inspect, so this check proved nothing'

records=$(block_records "${spec_files[@]}")
[[ -n "$records" ]] || fail 'read no fenced block out of spec/, so this check inspected nothing'

plain=$(printf '%s\n' "$records" | awk -F'\t' '$3 == "bash"' | grep -c . || true)
(( plain > 0 )) \
    || fail "read $(printf '%s\n' "$records" | grep -c . || true) fenced block(s) out of spec/ and not one plain \`\`\`bash block among them, so this rule held over nothing"

skipped=$(skipped_blocks "${spec_files[@]}")
if [[ -n "$skipped" ]]; then
    printf 'spec block execution: mustmatch skips a plain ```bash block that calls no mustmatch command, so not one line of these blocks runs and the gate log reports them beside the passes:\n' >&2
    printf '%s\n' "$skipped" \
        | awk -F'\t' -v root="$repository/" '{ sub("^" root, "", $1); printf "  %s:%d (%d line(s) never run)\n", $1, $2, $3 }' >&2
    printf 'Give each block a mustmatch call, or remove it and say in the file why the claim is gone.\n' >&2
    exit 1
fi

printf 'spec block execution: %s fenced block(s) in %s spec file(s), %s plain ```bash block(s), every one of them run\n' \
    "$(printf '%s\n' "$records" | grep -c . || true)" "${#spec_files[@]}" "$plain"
