#!/usr/bin/env bash
set -euo pipefail

# mustmatch runs a ```bash or ```sh block only when the block calls a
# `mustmatch` command or its fence carries a `run` attribute. A block with
# neither is reported `SKIP` and not one of its lines runs. Measured against mustmatch directly: a two-line block whose first
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
#   Every fenced block in spec/ that mustmatch would run a shell over calls a
#   `mustmatch` command or carries a `run` attribute, so mustmatch runs it.
#
# What mustmatch runs a shell over, and what makes it open the block, were
# measured against mustmatch directly on 2026-09-11:
#
#   ```bash            skipped     ```bash run             run
#   ```sh              skipped     ```bash run exit=1      run
#   ```Bash            skipped     ```sh run id=x ...      run
#   ```bash foo        skipped     ```bash + a mustmatch   run
#   ```bash ignore     skipped       call in the body
#   ```bash exit=1     skipped
#   ```zsh             not a block at all, and neither is ```text
#
# So the language is `bash` or `sh`, read without regard to case, and the one
# attribute that brings its own expectation is `run`. Every other attribute
# leaves the block exactly as skipped as a bare fence. Reading the info string
# as the literal `bash` instead would leave `sh`, `Bash` and `bash ignore`
# outside the rule, and a block could be excused from it by one added word.
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

# The blocks in the files named that mustmatch would skip whole: a shell fence
# carrying no `run` attribute and no mustmatch call in its body.
skipped_blocks() {
    block_records "$@" | awk -F'\t' '
        $4 == 0 {
            words = split($3, word, /[[:space:]]+/)
            language = tolower(word[1])
            if (language != "bash" && language != "sh") next
            for (i = 2; i <= words; i++) if (word[i] == "run") next
            print $1 "\t" $2 "\t" $5
        }
    '
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
    printf '```text\nnot a command at all\n```\n\n'
    printf '```sh\nfalse\n```\n\n'
    printf '```Bash\nfalse\n```\n\n'
    printf '```bash ignore\nfalse\n```\n\n'
    printf '```sh run id=other exit=2 stream=stderr\nfalse\n```\n'
} >"$fixture"

# The four fences mustmatch would skip whole open at these lines, and the four
# beside them are the accepted half: a mustmatch call in the body, a `run`
# attribute on either language, and a `text` fence that is no shell block at
# all. A scan that refused those would make the repair impossible to write.
fixture_skipped=$(skipped_blocks "$fixture")
fixture_skipped_lines=$(cut -f2 <<<"$fixture_skipped" | paste -sd, -)
[[ "$fixture_skipped_lines" == '3,19,23,27' ]] \
    || fail "the scan named the skipped blocks of its own fixture at line(s) $fixture_skipped_lines, and they open at lines 3 (\`\`\`bash), 19 (\`\`\`sh), 23 (\`\`\`Bash) and 27 (\`\`\`bash ignore) -- every one a fence mustmatch runs a shell over, skips whole, and reports beside the passes: $fixture_skipped"

fixture_blocks=$(block_records "$fixture" | grep -c . || true)
[[ "$fixture_blocks" == 8 ]] \
    || fail "the scan read $fixture_blocks of the 8 fenced blocks in its own fixture, so it does not see every block"

# --- 2. every shell block in spec/ runs -----------------------------------------
shopt -s nullglob
spec_files=("$repository"/spec/*.md)
shopt -u nullglob
(( ${#spec_files[@]} > 0 )) || fail 'found no spec/*.md files to inspect, so this check proved nothing'

records=$(block_records "${spec_files[@]}")
[[ -n "$records" ]] || fail 'read no fenced block out of spec/, so this check inspected nothing'

shell=$(printf '%s\n' "$records" \
    | awk -F'\t' '{ split($3, word, /[[:space:]]+/); language = tolower(word[1]); if (language == "bash" || language == "sh") print }' \
    | grep -c . || true)
(( shell > 0 )) \
    || fail "read $(printf '%s\n' "$records" | grep -c . || true) fenced block(s) out of spec/ and not one \`\`\`bash or \`\`\`sh block among them, so this rule held over nothing"

skipped=$(skipped_blocks "${spec_files[@]}")
if [[ -n "$skipped" ]]; then
    printf 'spec block execution: mustmatch runs a shell over these ```bash and ```sh blocks, and skips each one whole because it carries no `run` attribute and calls no mustmatch command, so not one of their lines runs and the gate log reports them beside the passes:\n' >&2
    printf '%s\n' "$skipped" \
        | awk -F'\t' -v root="$repository/" '{ sub("^" root, "", $1); printf "  %s:%d (%d line(s) never run)\n", $1, $2, $3 }' >&2
    printf 'Give each block a mustmatch call, or remove it and say in the file why the claim is gone.\n' >&2
    exit 1
fi

printf 'spec block execution: %s fenced block(s) in %s spec file(s), %s shell block(s), every one of them run\n' \
    "$(printf '%s\n' "$records" | grep -c . || true)" "${#spec_files[@]}" "$shell"
