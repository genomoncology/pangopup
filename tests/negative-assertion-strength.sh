#!/usr/bin/env bash
set -euo pipefail

# A shell gate that forbids something by writing `! grep ...` under
# `set -euo pipefail` reports nothing when it finds what it forbids. `set -e`
# does not exit on a pipeline whose status is inverted with `!`, so the command
# returns 1, the shell carries on, and the run ends 0. Thirteen assertions in
# `tests/executable-delivery.sh` and `tests/production-release-qualification.sh`
# were written that way. Among what they forbid: `contents: write`, `attest`,
# `release create` and `release upload` in the packaging workflow's permissions,
# `make lint`, `make test` and `make spec` in that workflow, `ubuntu-22.04` as
# the runner, and a glibc ceiling of 2.35 -- every one of them reintroducible
# with the gate silent.
#
# `! cmd || fail ...` and `! cmd || return 1` are a different shape and are
# unaffected, because the `||` supplies the exit. This file has to tell the two
# apart, and section 2 proves it does, in both directions.
#
# Two things are pinned here.
#
# --- the shape cannot come back ---
#
#   No shell gate in this repository holds a statement led by a bare `!` whose
#   exit nothing consumes. A statement is led by a bare `!` when its first
#   non-blank characters are `!` followed by whitespace -- which is the only
#   way the shell spells negation, since `!foo` is a word and not an operator.
#   Its exit is consumed when a `||` stands on the same line outside quotes.
#   That narrowness is deliberate: `if ! cmd; then`, `while ! cmd; do`,
#   `[[ "$a" != "$b" ]]`, an awk `!hit` pattern and a `!=` comparison are all
#   legitimate and none of them lead a line with `! `.
#
# --- what the gates forbid is still forbidden, and refusing it works ---
#
#   `tests/support/forbidden-text.sh` defines `refuse_text`. It is sourced --
#   sourcing it runs nothing.
#
#       refuse_text <file> <text> <description>
#
#   It refuses when <text> stands anywhere in <file>, as literal text rather
#   than as a pattern, and the refusal names the text, the file and the
#   description so a reader knows what to remove. It exits non-zero itself, so
#   a caller that writes nothing after it still stops.
#
#   Arguments 2 and 3 are written in the calling harness as single-quoted
#   literals, and argument 1 either names a path under the repository or is a
#   variable the same harness assigns such a path to. That is what lets
#   section 4 enumerate the forbidden texts out of the harnesses themselves
#   rather than out of a hand-written list that goes stale.
#
# What this does not prove: that the forbidden text is the right thing to
# forbid, or that a gate forbids everything it should. Section 5 holds the
# list of what today's gates forbid, so the repair cannot be bought by
# forbidding less.
#
# Nothing here writes to a file in the checkout. Every reintroduction happens
# on a throwaway file under a temporary directory.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
support="$repository/tests/support/forbidden-text.sh"
harnesses=(executable-delivery.sh production-release-qualification.sh)

fail() { printf 'negative assertion strength: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# --- 1. every shell gate, scanned ------------------------------------------
#
# The scan reads `tests/`, `scripts/` and `install.sh`, which is every shell
# file this repository ships. It reports how many files and statements it read,
# and refuses a count of zero, because a scan that matched nothing would
# otherwise pass by finding nothing.

scan='
FNR == 1 { file_count++ }
{
    statement = $0
    sub(/^[[:space:]]*/, "", statement)
    if (statement == "" || statement ~ /^#/) { next }
    statements++
    if (statement !~ /^![[:space:]]/) { next }
    negations++
    code = $0
    gsub(/\047[^\047]*\047/, "", code)
    gsub(/"[^"]*"/, "", code)
    if (index(code, "||") > 0) { next }
    printf "%s:%d: %s\n", FILENAME, FNR, statement > "/dev/stderr"
    bare++
}
END { printf "%d %d %d %d\n", file_count, statements, negations, bare }
'

shell_files=()
while IFS= read -r path; do shell_files+=("$path"); done < <(
    find "$repository/tests" "$repository/scripts" -type f -name '*.sh' | sort
    printf '%s\n' "$repository/install.sh"
)

# `</dev/null` matters: awk with no file operands reads standard input, so a
# scan that found no file would block for ever instead of refusing.
scan_files() { awk "$scan" "$@" </dev/null 2>"$work/bare"; }

read -r scanned statements negations bare < <(scan_files "${shell_files[@]}")

[[ "$scanned" -gt 0 ]] \
    || fail 'the scan found no shell files, so it proved nothing'
[[ "$statements" -gt 0 ]] \
    || fail 'the scan read no statements, so it proved nothing'

# Named rather than discovered, because these two are the gates the guarantee is
# about. Another file adopting the shape is caught by the scan; either of these
# two leaving the scan is the drift this file exists to stop.
for name in "${harnesses[@]}"; do
    harness="$repository/tests/$name"
    [[ -f "$harness" ]] || fail "tests/$name is gone, so this scan is checking the wrong files"
    printf '%s\n' "${shell_files[@]}" | grep -Fqx "$harness" \
        || fail "tests/$name is not among the files the scan reads"
done

if [[ "$bare" != 0 ]]; then
    fail "$bare negative assertion(s) are led by a bare '!' and carry on when they find what they forbid: $(tr '\n' ' ' <"$work/bare")"
fi

# --- 2. the scan tells the two shapes apart --------------------------------
#
# Proved against fixtures, so these cases stay readable and do not depend on
# what the gates happen to say today. The accepted half matters as much as the
# refused half: a scan that refuses `! cmd || fail` would make the repair
# impossible to write.
# Written with printf rather than a heredoc on purpose. The scan reads every
# shell file in the repository, this one included, and it has no exemption for
# heredoc bodies -- an exemption there would let the shape come back inside one.
refused="$work/refused.sh"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    "! grep -Fq 'forbidden' file" \
    '! cmp -s one two' \
    "! grep -Eq 'left||right' file" \
    >"$refused"

accepted="$work/accepted.sh"
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    "! grep -Fq 'forbidden' file || fail 'found the forbidden thing'" \
    "! grep -Fq 'forbidden' file || return 1" \
    "if ! grep -Fq 'forbidden' file; then exit 1; fi" \
    "while ! grep -Fq 'ready' file; do sleep 1; done" \
    '[[ "$a" != "$b" ]]' \
    '[[ ! -e "$path" ]]' \
    "# ! grep -Fq 'forbidden' file" \
    "awk '!hit { next } END { if (!hit) { exit 3 } }' file" \
    >"$accepted"

read -r _ _ fixture_negations fixture_bare < <(scan_files "$refused")
[[ "$fixture_bare" == 3 ]] \
    || fail "the scan found $fixture_bare of the 3 bare negations in its own fixture, so it cannot see the shape it exists to refuse"
[[ "$(grep -Fc 'left||right' "$work/bare")" == 1 ]] \
    || fail 'the scan read a `||` standing inside a quoted pattern as an exit the statement consumes, so a bare negation carrying one is exempted'

read -r _ _ accepted_negations accepted_bare < <(scan_files "$accepted")
[[ "$accepted_bare" == 0 ]] \
    || fail "the scan refused a legitimate shape: $(tr '\n' ' ' <"$work/bare")"
[[ "$accepted_negations" -ge 2 ]] \
    || fail "the scan saw $accepted_negations negations in a fixture holding two consumed negations, so it is not reading the shape it claims to exempt"

# --- 3. the mechanism ------------------------------------------------------
[[ -f "$support" ]] \
    || fail "no tests/support/forbidden-text.sh: nothing gives the gates a way to forbid something that stops the run when it finds it"

clean="$work/clean.txt"
printf 'runs-on: ubuntu-24.04\nglibc maximum 2.39\n' >"$clean"

dirty="$work/dirty.txt"
printf 'runs-on: ubuntu-24.04\n# runs-on: ubuntu-22.04\n' >"$dirty"

near="$work/near.txt"
printf 'runs-on: ubuntu-22X04\n' >"$near"

refuse_status=0
refuse_error=
run_refuse() {
    set +e
    ( set -euo pipefail; source "$support"; refuse_text "$@" ) >"$work/out" 2>"$work/err"
    refuse_status=$?
    set -e
    refuse_error=$(cat "$work/err")
}

run_refuse "$clean" 'runs-on: ubuntu-22.04' 'a runner image older than the one the release is built on'
[[ "$refuse_status" == 0 ]] \
    || fail "the mechanism refused a file that does not hold the forbidden text: $refuse_error"

run_refuse "$dirty" 'runs-on: ubuntu-22.04' 'a runner image older than the one the release is built on'
[[ "$refuse_status" != 0 ]] \
    || fail 'the mechanism accepted a file that holds the forbidden text'
[[ "$refuse_error" == *'runs-on: ubuntu-22.04'* ]] \
    || fail "the refusal does not name what was found: $refuse_error"
[[ "$refuse_error" == *'dirty.txt'* ]] \
    || fail "the refusal does not name the file it was found in: $refuse_error"
[[ "$refuse_error" == *'a runner image older than the one the release is built on'* ]] \
    || fail "the refusal does not say what the forbidden text means: $refuse_error"

run_refuse "$near" 'runs-on: ubuntu-22.04' 'a runner image older than the one the release is built on'
[[ "$refuse_status" == 0 ]] \
    || fail "the mechanism read the forbidden text as a pattern rather than as text, so it forbids more than it names: $refuse_error"

# The whole point. A caller that writes nothing after the call still stops, so
# the repair cannot be undone by dropping a `|| fail` from one site.
carries_on="$work/carries-on.sh"
cat >"$carries_on" <<FIXTURE
#!/usr/bin/env bash
set -euo pipefail
source "$support"
refuse_text "$dirty" 'runs-on: ubuntu-22.04' 'an old runner image'
echo REACHED
FIXTURE
if carried=$(bash "$carries_on" 2>/dev/null); then
    fail 'a gate that forbids something and writes nothing after it still exits 0 when it finds it'
fi
[[ "$carried" != *REACHED* ]] \
    || fail 'a gate carried on past a refusal, so the statements after it ran against what it forbids'

# --- 4. every forbidden thing, reintroduced in turn ------------------------
#
# The entries are read out of the harnesses, so a forbidden text that stops
# being guarded stops being proved here too and section 5 notices.
ledger="$work/ledger"
unparsed="$work/unparsed"
unit=$(printf '\037')
: >"$ledger"
: >"$unparsed"
for name in "${harnesses[@]}"; do
    awk -v harness="$name" -v unparsed="$unparsed" '
        /^[^#]*refuse_text/ {
            quote = sprintf("%c", 39)
            unit = sprintf("%c", 31)
            n = split($0, field, quote)
            if (n < 5) { printf "%s:%d: %s\n", harness, NR, $0 >> unparsed; next }
            printf "%s%s%s%s%s%s%s\n", harness, unit, field[1], unit, field[2], unit, field[4]
        }
    ' "$repository/tests/$name" >>"$ledger"
done

# A call this scan cannot decompose is the way a forbidden thing goes unproved
# while the run still reports a total: the scan would skip it and say nothing.
if [[ -s "$unparsed" ]]; then
    fail "a refuse_text call is written in a form this scan cannot read, so the thing it forbids would never be reintroduced here: $(tr '\n' ' ' <"$unparsed")"
fi

entries=$(wc -l <"$ledger" | tr -d ' ')
[[ "$entries" -gt 0 ]] \
    || fail 'the scan found no forbidden texts in either harness, so it proved nothing'

for name in "${harnesses[@]}"; do
    grep -qE '^[^#]*support/forbidden-text\.sh' "$repository/tests/$name" \
        || fail "tests/$name does not source tests/support/forbidden-text.sh, so its negative assertions do not go through the one mechanism that stops the run"
done

# The file the call reads, named on the call line or through a variable the same
# harness assigns. A path a harness builds while it runs does not resolve, and
# only the reintroduction below is proved for it. `target/` is refused even when
# a previous run left something there, so this reads what the repository ships
# and reads the same thing on a machine that has never built.
resolve_target() {
    local harness=$1 head=$2 expression variable value rounds=0
    expression=$({ grep -oE '"[^"]*"[[:space:]]*$' <<<"$head" || true; } | tail -n 1)
    expression=$(sed -E 's/[[:space:]]+$//' <<<"$expression")
    expression=${expression#\"}
    expression=${expression%\"}
    [[ -n "$expression" ]] || return 1
    while [[ "$expression" == *'$'* ]]; do
        rounds=$((rounds + 1))
        [[ "$rounds" -le 8 ]] || return 1
        variable=$({ grep -oE '[$]\{?[A-Za-z_][A-Za-z0-9_]*' <<<"$expression" || true; } | head -n 1 | tr -d '${')
        [[ -n "$variable" ]] || return 1
        case "$variable" in
            repo|repository) value=$repository ;;
            *)
                value=$({ grep -E "^[[:space:]]*$variable=" "$repository/tests/$harness" || true; } \
                    | head -n 1 | sed -E 's/^[[:space:]]*[A-Za-z_][A-Za-z0-9_]*=//; s/^"//; s/"$//')
                [[ -n "$value" ]] || return 1
                ;;
        esac
        expression=${expression//\$\{$variable\}/$value}
        expression=${expression//\$$variable/$value}
    done
    case "$expression" in
        "$repository"/target/*) return 1 ;;
    esac
    printf '%s\n' "$expression"
}

resolved_targets="$work/targets"
: >"$resolved_targets"
while IFS="$unit" read -r harness head text description; do
    [[ -n "$text" ]] \
        || fail "tests/$harness has a refuse_text call with an empty forbidden text"
    [[ -n "$description" ]] \
        || fail "tests/$harness forbids '$text' without saying what it means, so a refusal tells a reader nothing"

    # A copy of the file the gate reads, with the forbidden thing put back at
    # the end of it. A forbidden text whose file is built by the harness at run
    # time has no file to copy, so it is put back into a file of its own.
    reintroduced="$work/reintroduced"
    if target=$(resolve_target "$harness" "$head") && [[ -f "$target" ]]; then
        printf '%s%s%s\n' "$text" "$unit" "${target#"$repository"/}" >>"$resolved_targets"

        # The file as it ships is accepted, so the refusal below is the
        # reintroduction and not a mechanism that refuses everything.
        run_refuse "$target" "$text" "$description"
        [[ "$refuse_status" == 0 ]] \
            || fail "tests/$harness forbids '$text' in ${target#"$repository"/}, which holds it as it ships: $refuse_error"

        cp "$target" "$reintroduced"
        printf '%s\n' "$text" >>"$reintroduced"
    else
        printf '%s%s\n' "$text" "$unit" >>"$resolved_targets"
        printf 'a line before\n%s\na line after\n' "$text" >"$reintroduced"
    fi

    run_refuse "$reintroduced" "$text" "$description"
    [[ "$refuse_status" != 0 ]] \
        || fail "tests/$harness forbids '$text', but putting it back leaves the gate green"
    [[ "$refuse_error" == *"$text"* ]] \
        || fail "the refusal for '$text' does not name what was found: $refuse_error"
    [[ "$refuse_error" == *"$reintroduced"* ]] \
        || fail "the refusal for '$text' does not name the file it was found in: $refuse_error"
done <"$ledger"

# --- 5. the gates still forbid what they forbid today ----------------------
#
# Every text the two harnesses forbade on 2026-09-10, beside the file it is
# forbidden in, so the repair cannot be bought by forbidding less -- or by
# leaving a text guarded while pointing it at a file nobody cares about.
# Pointing `runs-on: ubuntu-22.04` at `AGENTS.md` leaves the packaging
# workflow's runner image unguarded while every count still adds up, so the
# pair is what is held here rather than the text on its own.
#
# Two texts have no shipped file to name: their gate reads a file the harness
# builds while it runs, and section 4 proves only the refusal for those. They
# are written with an empty file, so that exemption is counted rather than
# implied and a third one appearing is a change this list has to be told about.
for pair in \
    $'contents: write\t.github/workflows/package-linux.yml' \
    $'attest\t.github/workflows/package-linux.yml' \
    $'release create\t.github/workflows/package-linux.yml' \
    $'release upload\t.github/workflows/package-linux.yml' \
    $'GRCh38:chr12:6801301:G:A\t.github/workflows/package-linux.yml' \
    $'GRCh38:chr1:5051:A:AC\t.github/workflows/package-linux.yml' \
    $'make lint\t.github/workflows/package-linux.yml' \
    $'make test\t.github/workflows/package-linux.yml' \
    $'make spec\t.github/workflows/package-linux.yml' \
    $'Install gate prerequisites\t.github/workflows/package-linux.yml' \
    $'mustmatch\t.github/workflows/package-linux.yml' \
    $'cargo-deny\t.github/workflows/package-linux.yml' \
    $'ripgrep\t.github/workflows/package-linux.yml' \
    $'runs-on: ubuntu-22.04\t.github/workflows/package-linux.yml' \
    $'"$maximum" 2.35\tscripts/qualify-linux-release.sh' \
    $'orgs/genomoncology/packages/container/pangopup\tplanning/artifacts/055-public-v0.3.0.md' \
    $'GH_TOKEN=\tplanning/artifacts/050-public-linux-release.md' \
    $'GITHUB_TOKEN=\tplanning/artifacts/050-public-linux-release.md' \
    $'Authorization:\tplanning/artifacts/050-public-linux-release.md' \
    $'public executable publication remains a separate ticket\tAGENTS.md' \
    $'Add Pangopup to PATH\t' \
    $'curl -fsSL\t'; do
    forbidden=${pair%%$'\t'*}
    in_file=${pair#*$'\t'}
    found=0
    while IFS="$unit" read -r text target; do
        case "$text" in
            *"$forbidden"*) [[ "$target" == "$in_file" ]] && found=1 ;;
        esac
    done <"$resolved_targets"
    if [[ "$found" != 1 ]]; then
        if [[ -n "$in_file" ]]; then
            fail "no gate forbids '$forbidden' in $in_file any more, so it can come back there without a gate noticing"
        fi
        fail "no gate forbids '$forbidden' any more, so it can come back without a gate noticing"
    fi
done

# The shipped files the gates protect. A repair that pointed every forbidden
# text at a file the harness builds while it runs would pass section 4 without
# ever reading what ships.
for protected in \
    '.github/workflows/package-linux.yml' \
    'scripts/qualify-linux-release.sh' \
    'planning/artifacts/050-public-linux-release.md' \
    'planning/artifacts/055-public-v0.3.0.md' \
    'AGENTS.md'; do
    cut -d"$unit" -f2 "$resolved_targets" | grep -Fqx "$protected" \
        || fail "no gate forbids anything in $protected any more, so nothing reads what that file ships"
done

printf 'negative assertion strength: %s statement(s) in %s shell file(s), %s negation(s), 0 bare; %s forbidden text(s) refused when reintroduced\n' \
    "$statements" "$scanned" "$negations" "$entries"
