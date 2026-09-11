#!/usr/bin/env bash
set -euo pipefail

# Four variables name something about the pangopup model cache, and every one
# of them is read before `XDG_CACHE_HOME` and `HOME` are consulted:
#
#   PANGOPUP_MODEL_CACHE              the model cache file itself
#   PANGOPUP_CACHE_DIR                the download cache directory
#   PANGOPUP_DATA_DIR                 the installed bundle directory
#   PANGOPUP_MODEL_CACHE_MAX_ENTRIES  how many rows that cache keeps
#
# Ticket 0090 gave every route to the built executable a cache home of its own
# and dropped the first three at each one. The fourth was left inherited, and it
# is read by `resolve_model_cache_options` like the rest. An operator who
# exported it runs the whole suite under it: an unparsable value refuses every
# modelled lookup outright, and a small valid one evicts on a schedule the
# operator chose, under harnesses that then score what the cache handed back.
#
# The fourth name is a superstring of the first. `PANGOPUP_MODEL_CACHE` stands
# inside `PANGOPUP_MODEL_CACHE_MAX_ENTRIES`, so a check that looks for the
# shorter name as plain text is satisfied by a route that drops only the longer
# one, and reports a route that inherits the model cache path as held. Section 1
# exercises both directions before any route is read.
#
# Sections 2 and 3 read the routes. The shell helper is read by sourcing it and
# by running the miniature modelled lookup under it, because that run is the
# operator's own failure. The `spec` recipe, the Rust spawn helper and the
# maintainer measurement are read as the lists they are: running them means a
# whole spec suite, a whole test binary and a whole measurement, and what can go
# wrong in each is a name missing from a list.
#
# Section 4 holds the other side of the boundary. The helpers drop what a
# harness inherited; the product keeps reading all four for an operator running
# it. Proving only the first half would let an implementation that deleted the
# variable outright pass.

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

limit=PANGOPUP_MODEL_CACHE_MAX_ENTRIES
names=(PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR "$limit")

fail() { printf 'model cache limit inheritance: %s\n' "$*" >&2; exit 1; }

# --- the two matchers -------------------------------------------------------

# Text $1 drops $2 by option: `env -u NAME`, long or short. The trailing class
# is what keeps `-u PANGOPUP_MODEL_CACHE_MAX_ENTRIES` from reading as a drop of
# `PANGOPUP_MODEL_CACHE`.
drops_by_option() {
    grep -qE -- "(-u[[:space:]=]+|--unset=)$2([^A-Za-z0-9_]|\$)" <<<"$1"
}

# Text $1 carries $2 as a quoted name in a list, the way both the Rust helper
# and the maintainer measurement spell theirs. The closing quote is part of the
# pattern for the same reason.
listed() {
    grep -qE -- "\"$2\"" <<<"$1"
}

# --- 1. the matchers tell the two names apart, in both directions -----------

matcher_checks=0

expect_match() {
    local matcher=$1 text=$2 name=$3 why=$4
    matcher_checks=$((matcher_checks + 1))
    "$matcher" "$text" "$name" || fail "$why"
}

expect_no_match() {
    local matcher=$1 text=$2 name=$3 why=$4
    matcher_checks=$((matcher_checks + 1))
    if "$matcher" "$text" "$name"; then
        fail "$why"
    fi
}

only_long_option="env -u PANGOPUP_MODEL_CACHE_MAX_ENTRIES true"
only_short_option="env -u PANGOPUP_MODEL_CACHE true"
both_options="env -u PANGOPUP_MODEL_CACHE -u PANGOPUP_MODEL_CACHE_MAX_ENTRIES true"

expect_match drops_by_option "$only_long_option" "$limit" \
    'the option matcher does not see a route that drops the entry limit, so no route could ever satisfy this file'
expect_no_match drops_by_option "$only_long_option" PANGOPUP_MODEL_CACHE \
    'the option matcher reads a route that drops only PANGOPUP_MODEL_CACHE_MAX_ENTRIES as dropping PANGOPUP_MODEL_CACHE, so a route that still inherits the operator model cache path would pass'
expect_match drops_by_option "$only_short_option" PANGOPUP_MODEL_CACHE \
    'the option matcher does not see a route that drops PANGOPUP_MODEL_CACHE'
expect_no_match drops_by_option "$only_short_option" "$limit" \
    'the option matcher reads a route that drops only PANGOPUP_MODEL_CACHE as dropping the entry limit, which is the gap this file exists to close'
expect_match drops_by_option "$both_options" PANGOPUP_MODEL_CACHE \
    'the option matcher misses PANGOPUP_MODEL_CACHE in a route that drops both names'
expect_match drops_by_option "$both_options" "$limit" \
    'the option matcher misses the entry limit in a route that drops both names'

only_long_list='for name in ["PANGOPUP_MODEL_CACHE_MAX_ENTRIES"]'
only_short_list='for name in ["PANGOPUP_MODEL_CACHE"]'
both_listed='for name in ["PANGOPUP_MODEL_CACHE", "PANGOPUP_MODEL_CACHE_MAX_ENTRIES"]'

expect_match listed "$only_long_list" "$limit" \
    'the list matcher does not see a list carrying the entry limit, so no route could ever satisfy this file'
expect_no_match listed "$only_long_list" PANGOPUP_MODEL_CACHE \
    'the list matcher reads a list carrying only PANGOPUP_MODEL_CACHE_MAX_ENTRIES as carrying PANGOPUP_MODEL_CACHE, so a helper that still inherits the operator model cache path would pass'
expect_match listed "$only_short_list" PANGOPUP_MODEL_CACHE \
    'the list matcher does not see a list carrying PANGOPUP_MODEL_CACHE'
expect_no_match listed "$only_short_list" "$limit" \
    'the list matcher reads a list carrying only PANGOPUP_MODEL_CACHE as carrying the entry limit, which is the gap this file exists to close'
expect_match listed "$both_listed" PANGOPUP_MODEL_CACHE \
    'the list matcher misses PANGOPUP_MODEL_CACHE in a list carrying both names'
expect_match listed "$both_listed" "$limit" \
    'the list matcher misses the entry limit in a list carrying both names'

(( matcher_checks > 0 )) \
    || fail 'the matchers were never exercised, so nothing is known about what they can tell apart'

# --- 2. the routes that are lists -------------------------------------------

routes=0

# Hold the one line of Makefile recipe $1 that matches $2. Read from the recipe
# rather than from the whole file, because the drop has to stand on the line
# that starts the run; $3 says what that run is, so a refusal names it.
hold_recipe() {
    local target=$1 marker=$2 what=$3 recipe_line name
    recipe_line=$(
        awk -v target="$target:" -v marker="$marker" '
            index($0, target) == 1 { inside = 1; next }
            inside && /^\t/ {
                line = substr($0, 2)
                if (line ~ marker) { print line; exit }
                next
            }
            inside && /^[[:space:]]*$/ { next }
            inside { exit }
        ' "$repo/Makefile"
    )
    [[ -n "$recipe_line" ]] \
        || fail "the Makefile $target recipe has no line matching $marker, so this rule read nothing about it"
    routes=$((routes + 1))
    for name in "${names[@]}"; do
        drops_by_option "$recipe_line" "$name" \
            || fail "the Makefile $target recipe $what with $name inherited: it is read ahead of XDG_CACHE_HOME, so make $target reaches what the operator who ran it named"
    done
}

# `spec` runs the built executable, and the line giving that run a cache home
# is the line the drop has to stand on.
hold_recipe spec 'XDG_CACHE_HOME=' 'runs the built executable'

# `test` starts the test process itself. The spawn helper covers every child
# that process starts; it cannot cover the process. The unit tests inside
# `pangopup-cli` parse a lookup in that process, and
# `resolve_model_cache_options` reads the entry limit out of it, so an
# inherited value the product cannot parse panics them and the suite cannot be
# run at all by the operator who exported it.
hold_recipe test 'cargo test' 'starts the test process'

# The Rust spawn helper's drop list, and the maintainer measurement's. Each is
# a list of names in a loop that removes them, and each is read as the list it
# is rather than as the whole file: a name mentioned anywhere else in either
# file has dropped nothing. The search starts at the function that builds the
# child's environment, because both files carry other loops over other lists --
# `measure.py` has a `for name in ("manifest", "notice")` three hundred lines
# earlier, and a rule that read the first list it found would hold the wrong
# one.
hold_list() {
    local relative=$1 marker=$2 opening=$3 closing=$4 block name
    local path="$repo/$relative"
    [[ -f "$path" ]] \
        || fail "no $relative: a route this rule holds is gone, so it would hold over nothing"
    block=$(
        awk -v marker="$marker" -v opening="$opening" -v closing="$closing" '
            !started { if (index($0, marker) > 0) { started = 1 }; next }
            !inside { if (index($0, opening) > 0) { inside = 1 } else { next } }
            { print }
            index($0, closing) > 0 { exit }
        ' "$path"
    )
    [[ -n "$block" ]] \
        || fail "$relative no longer spells the names it drops as \`$opening ... $closing\` after \`$marker\`, so this rule cannot tell which names it drops and would hold over nothing"
    routes=$((routes + 1))
    for name in "${names[@]}"; do
        listed "$block" "$name" \
            || fail "$relative hands a run of the built executable $name as it inherited it: that variable is read ahead of XDG_CACHE_HOME, so the run reaches what whoever started it named"
    done
}

hold_list 'crates/pangopup-cli/tests/support/mod.rs' 'pub fn pangopup()' 'for name in [' ']'
hold_list 'maintainers/ticket-053/measure.py' 'def child_environment(' 'for name in (' ')'

(( routes > 0 )) \
    || fail 'no route was read, so this rule held over nothing'

# --- 3. the shell helper, read by running under it --------------------------

# Build before taking a cache home: the ONNX Runtime library the build resolves
# lands under XDG_CACHE_HOME, so a build under a fresh one downloads it again.
"$repo/scripts/require-built-commands.sh"
# Spelled in full rather than through a variable: the sibling scan reads a
# literal source line, and naming the helper in a variable establishes nothing.
. "$repo/tests/support/private-cache-home.sh"

executable="$repo/target/debug/pangopup"
helper="$repo/tests/support/private-cache-home.sh"

# The miniature modelled lookup. A modelled lookup is what opens the model
# cache, so it is the run an inherited limit refuses.
model_args=(
    lookup --model-only --variant GRCh38:chr1:5051:A:C
    --model-bundle "$repo/tests/fixtures/pangolin-model-kernel-mini/bundle"
    --reference-bundle "$repo/tests/fixtures/reference-route-test/bundle"
    --mask "$repo/tests/fixtures/route-mask/domains.pgm"
)

probes=0

# Every one of the four, read back from a shell that exported it at a value the
# product cannot use. Removal is the requirement: an empty value is a value the
# product still reads.
for name in "${names[@]}"; do
    probes=$((probes + 1))
    left=$(
        env "$name=not-a-usable-value" \
            bash -c '
                set -euo pipefail
                . "$1"
                if [[ -n "${!2+set}" ]]; then
                    printf "%s" "${!2}"
                fi
            ' bash "$helper" "$name"
    ) || fail "sourcing the helper with $name exported failed"
    [[ -z "$left" ]] \
        || fail "the helper leaves $name=[$left] exported after a harness sources it, so every run under that harness reads it and reaches what whoever ran the harness named"
done

# The operator's own failure, run whole. With the limit exported at a value the
# product cannot parse, every modelled lookup under the helper is refused, so
# the suite cannot be run at all by an operator who exported it.
probes=$((probes + 1))
status=0
reported=$(
    env "$limit=not-a-limit" \
        bash -c '
            set -euo pipefail
            . "$1"
            shift 2
            exec "$@"
        ' bash "$helper" "$limit" "$executable" "${model_args[@]}" 2>&1
) || status=$?
(( status == 0 )) \
    || fail "a modelled lookup under the helper was refused because $limit was exported in the shell that started the harness, so an operator who exported it cannot run the suite at all: $reported"
case "$reported" in
    *'"status":"found"'*) ;;
    *) fail "the modelled lookup under the helper scored nothing, so finding it unrefused proves nothing about what it read: $reported" ;;
esac

# --- 4. the product keeps reading the limit for an operator -----------------
#
# The helper drops an inherited value. It does not take the variable away.

probes=$((probes + 1))
status=0
reported=$(env "$limit=not-a-limit" "$executable" "${model_args[@]}" 2>&1) || status=$?
(( status != 0 )) \
    || fail "the product accepted $limit=not-a-limit, so an operator's own setting stopped being read: $reported"
case "$reported" in
    *'model cache maximum'*) ;;
    *) fail "the product refused $limit=not-a-limit without saying what is wrong with it, so an operator cannot act on it: $reported" ;;
esac

probes=$((probes + 1))
status=0
reported=$(env "$limit=1" "$executable" "${model_args[@]}" 2>&1) || status=$?
(( status == 0 )) \
    || fail "the product refused $limit=1, which is a limit an operator may choose: $reported"

(( probes > 0 )) \
    || fail 'no run was made, so nothing is known about what a run under the helper reads'

printf 'read %s route(s) that drop %s inherited cache variable(s), told PANGOPUP_MODEL_CACHE and %s apart over %s matcher case(s), and made %s run(s) under and beside the shell helper\n' \
    "$routes" "${#names[@]}" "$limit" "$matcher_checks" "$probes"
