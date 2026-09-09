#!/usr/bin/env bash
set -euo pipefail

# The model cache is on by default and its directory comes from
# `XDG_CACHE_HOME`, or from `HOME` when that is unset. A test that spawns the
# shipped `pangopup` executable without redirecting both reaches the cache of
# whoever is running the suite. Since ticket 0058 a cache whose recorded setup
# no longer matches is discarded rather than ignored, so such a spawn destroys
# the operator's cache instead of merely adding fixture rows to it.
#
# Ticket 0059 gave `crates/pangopup-cli/tests/model_routing.rs` a private cache
# home per run and proved it for that one file. Nothing held any other file to
# it, and whether a stray spawn reached a cache depended on which subcommand it
# happened to call. This makes the rule structural instead: the name of the
# shipped executable appears in exactly one place, a shared spawn helper that
# redirects both variables, so a test cannot spawn `pangopup` without one.
#
# What this does not prove: that the helper redirects anything. That is a
# runtime property, and `crates/pangopup-cli/tests/spawn_isolation.rs` proves
# it. This file proves only that every spawn goes through the helper -- the two
# together are what hold the rule.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
helper_relative='crates/pangopup-cli/tests/support/mod.rs'

# The shipped executable is `pangopup`. `CARGO_BIN_EXE_pangopup-build` names a
# different binary that has no model cache, so the closing quote is part of the
# token rather than an accident of spelling.
token='CARGO_BIN_EXE_pangopup"'

# Cargo's token is not the only way a test can reach the shipped executable. A
# source can walk to it -- `std::env::current_exe()` and back up out of
# `deps/`, or `target/debug/pangopup` spelled from the manifest directory --
# and name the token nowhere. A gate blind to those measures the calling side
# it can see rather than the calling side that exists, which is how a check
# stays green while a spawn reaches the operator's cache. Refuse those
# spellings outside the helper too.
detour='current_exe|target/(debug|release)/pangopup'

# Two paths the detour scan does not read, each named with the reason it is an
# honest use rather than a way around the helper. Everything else stays in
# scope -- every crate's `tests/`, every `benches/`, every harness -- because a
# spawn that reaches the shipped executable by path reaches the cache home of
# whoever runs it wherever that spawn lives, and a scan narrowed to one
# directory measures the calling side it chose rather than the calling side
# that exists.
#
# `/src/` is product code: `src/uninstall.rs` reports the running executable's
# own path, and refusing that would demand a product change this check has no
# business demanding. `transport_resources.rs` re-executes its own test binary
# under `PANGOPUP_RESOURCE_MODE`; `pangopup-build` builds no `pangopup`, so no
# route to the shipped executable runs through it.
exempt='/src/|/crates/pangopup-build/tests/transport_resources\.rs$'

fail() { printf 'cli spawn cache isolation: %s\n' "$*" >&2; exit 1; }

# Refuse the tree rooted at $1 whose spawn helper is the file at $2, relative to
# that root. Prints its counts on acceptance and its reason on refusal.
examine() {
    local root=$1 helper=$2
    local sources=() suite=() occurrences strays detours scanned

    while IFS= read -r source; do sources+=("$source"); done < <(
        find "$root" -type f -name '*.rs' -not -path '*/target/*' | sort
    )
    scanned=${#sources[@]}
    if (( scanned == 0 )); then
        printf 'found no Rust source under %s, so this check examined nothing\n' "$root" >&2
        return 1
    fi

    occurrences=$(grep -nF -- "$token" "${sources[@]}" || true)
    if [[ -z "$occurrences" ]]; then
        printf 'examined %s Rust source(s) under %s and found no %s at all, so this check read no spawn and proved nothing\n' \
            "$scanned" "$root" "$token" >&2
        return 1
    fi

    strays=$(printf '%s\n' "$occurrences" | grep -vF -- "$root/$helper" || true)
    if [[ -n "$strays" ]]; then
        printf 'these spawns name the shipped executable outside %s, so each one inherits the cache home of whoever runs the suite and can discard that person'"'"'s model cache:\n' \
            "$helper" >&2
        printf '%s\n' "$strays" | sed -E 's#^'"$root"'/#  #; s/:([0-9]+):.*/:\1/' >&2
        return 1
    fi

    for source in "${sources[@]}"; do
        [[ $source =~ $exempt ]] || suite+=("$source")
    done
    detours=$(
        ((${#suite[@]})) && grep -nE -- "$detour" "${suite[@]}" | grep -vF -- "$root/$helper" || true
    )
    if [[ -n "$detours" ]]; then
        printf 'these sources reach the shipped executable by path instead of by name, so they spawn it outside %s and inherit the cache home of whoever runs the suite:\n' \
            "$helper" >&2
        printf '%s\n' "$detours" | sed -E 's#^'"$root"'/#  #; s/:([0-9]+):.*/:\1/' >&2
        return 1
    fi

    printf 'examined %s Rust source(s), %s spawn(s), all through %s\n' \
        "$scanned" "$(printf '%s\n' "$occurrences" | grep -c .)" "$helper"
}

# --- the scanner refuses what it exists to refuse ---------------------------
#
# A check that reports nothing wrong is worth exactly as much as its ability to
# report something wrong, so each refusal is exercised against a fixture tree
# before the real tree is read.

fixtures=$(mktemp -d)
trap 'rm -rf "$fixtures"' EXIT

plant() {
    local tree=$1 path=$2 spawn=$3
    mkdir -p "$tree/$(dirname "$path")"
    case "$spawn" in
        spawns) printf 'fn go() { Command::new(env!("%s)); }\n' "$token" >"$tree/$path" ;;
        # The directory is spelled through an argument rather than inline:
        # `tests/built-executable-currency.sh` reads a literal `target/debug/`
        # in code as a harness running a built executable, and this line plants
        # a fixture rather than running anything.
        detours) printf 'fn go() { Command::new(root().join("target/%s/pangopup")); }\n' debug >"$tree/$path" ;;
        *) printf 'fn go() {}\n' >"$tree/$path" ;;
    esac
}

expect_refusal() {
    local tree=$1 wanted=$2 output status
    set +e
    output=$(examine "$tree" "$helper_relative" 2>&1)
    status=$?
    set -e
    (( status != 0 )) || fail "the scanner accepted a tree it must refuse: $wanted"
    case "$output" in
        *"$wanted"*) ;;
        *) fail "the refusal does not name $wanted, so an operator cannot act on it: $output" ;;
    esac
}

empty="$fixtures/empty"
mkdir -p "$empty"
expect_refusal "$empty" 'examined nothing'

# A helper that names no executable is the same refusal as a tree with no
# spawn in it: the scan read no spawn, so it checked nothing.
silent="$fixtures/silent"
plant "$silent" "$helper_relative" quiet
plant "$silent" 'crates/pangopup-cli/tests/citation.rs' quiet
expect_refusal "$silent" 'proved nothing'

stray="$fixtures/stray"
plant "$stray" "$helper_relative" spawns
plant "$stray" 'crates/pangopup-cli/tests/macos_cli_boundary.rs' spawns
expect_refusal "$stray" 'crates/pangopup-cli/tests/macos_cli_boundary.rs:1'

# A source that walks to `target/debug/pangopup` names the token nowhere, so the
# stray check above cannot see it. It is still a spawn of the shipped
# executable outside the helper.
detour_tree="$fixtures/detour"
plant "$detour_tree" "$helper_relative" spawns
plant "$detour_tree" 'crates/pangopup-cli/tests/macos_cli_boundary.rs' detours
expect_refusal "$detour_tree" 'crates/pangopup-cli/tests/macos_cli_boundary.rs:1'

# A benchmark, or any other harness outside the test tree, reaches the same
# executable and inherits the same cache home. The scan reads it too.
bench_tree="$fixtures/bench"
plant "$bench_tree" "$helper_relative" spawns
plant "$bench_tree" 'crates/pangopup-cli/benches/snv_regression.rs' detours
expect_refusal "$bench_tree" 'crates/pangopup-cli/benches/snv_regression.rs:1'

clean="$fixtures/clean"
plant "$clean" "$helper_relative" spawns
plant "$clean" 'crates/pangopup-cli/tests/citation.rs' quiet
examine "$clean" "$helper_relative" >/dev/null \
    || fail 'the scanner refused a tree whose only spawn is the helper, so it refuses the shape it exists to require'

# The product resolves its own executable path and must keep doing so, so the
# exemption is exercised rather than only described.
exempted="$fixtures/exempted"
plant "$exempted" "$helper_relative" spawns
plant "$exempted" 'crates/pangopup-cli/src/uninstall.rs' detours
examine "$exempted" "$helper_relative" >/dev/null \
    || fail 'the scanner refused product code for resolving its own executable path, which this check has no business demanding'

# --- the real tree ----------------------------------------------------------
examine "$repository" "$helper_relative" || exit 1
