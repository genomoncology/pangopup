#!/usr/bin/env bash
set -euo pipefail

# Two rules read the `Makefile` and each of them has a hole the recipe it was
# written against does not show.
#
# --- a route added tomorrow inherits the entry limit ---
#
# `tests/recipe-spawn-cache-isolation.sh` refuses a recipe that reaches the
# built executable while inheriting a variable that names a cache location.
# Its `named_locations` array carries three names and
# `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` is not one of them, so a recipe added
# tomorrow that drops the three older names and keeps the entry limit is
# accepted. Measured on 2026-09-11: a throwaway recipe of exactly that shape,
# appended to this repository's own `Makefile`, left every gate green while
# handing a run of the built executable whatever limit the operator exported.
#
# The two names share a prefix, so the rule has to tell them apart in both
# directions. A recipe that drops only the longer name must still be refused
# for the shorter one, and a recipe that drops only the shorter one must still
# be refused for the longer one. The two fixtures below are that pair, and a
# rule written as a substring search fails one of them.
#
# --- the downloaded library sits where `cargo clean` deletes it ---
#
# `tests/spec-download-cache-durability.sh` refuses a recipe that points the
# downloaded ONNX Runtime library cache inside a directory the same recipe
# removes. `cargo clean` removes all of `target/` and is the command an
# operator runs before a rebuild, so a cache under `target/` is fetched again
# over the network for exactly the reason that rule exists. Nothing here
# downloads anything and nothing here runs `cargo clean`: the proof is where
# the recipe points that cache, which is what decides whether the download
# happens at all.
#
# Both rules are exercised by running the real gate scripts against a tree of
# this file's own making, so what is held is the gate an operator runs rather
# than a copy of its logic.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

recipe_gate=recipe-spawn-cache-isolation.sh
durability_gate=spec-download-cache-durability.sh

# The variable a fifth route may inherit, and the shorter name it contains.
entry_limit=PANGOPUP_MODEL_CACHE_MAX_ENTRIES
model_cache=PANGOPUP_MODEL_CACHE
cache_dir=PANGOPUP_CACHE_DIR
data_dir=PANGOPUP_DATA_DIR

fail() { printf 'recipe cache rule coverage: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# A tree the gates can read as a repository of their own: the spec files and
# the Python sources their other rules hold, and the two gates themselves. It
# is built once, and each case below rewrites only the `Makefile`.
tree="$work/tree"
mkdir -p "$tree/tests"
cp -R "$repository/spec" "$tree/spec"
cp -R "$repository/maintainers" "$tree/maintainers"
for gate in "$recipe_gate" "$durability_gate"; do
    [[ -f "$repository/tests/$gate" ]] \
        || fail "tests/$gate is gone, so this file is holding a rule that no longer exists"
    cp "$repository/tests/$gate" "$tree/tests/$gate"
done

GATE_OUTPUT=
GATE_STATUS=
run_gate() {
    local gate=$1
    set +e
    GATE_OUTPUT=$(bash "$tree/tests/$gate" 2>&1)
    GATE_STATUS=$?
    set -e
}

# The gate refuses, and its refusal carries the extended pattern $3 so that an
# operator can act on it. The pattern rather than a fixed text, because the two
# variable names share a prefix and a fixed search for the shorter one is
# answered by the longer one.
expect_refusal() {
    local gate=$1 why=$2 pattern=$3
    run_gate "$gate"
    (( GATE_STATUS != 0 )) || fail "$why -- the gate accepted it, and printed: $GATE_OUTPUT"
    grep -qE -- "$pattern" <<<"$GATE_OUTPUT" \
        || fail "the refusal does not match $pattern, so it does not name what was wrong: $GATE_OUTPUT"
}

expect_acceptance() {
    local gate=$1 why=$2
    run_gate "$gate"
    (( GATE_STATUS == 0 )) || fail "$why -- the gate refused it, and printed: $GATE_OUTPUT"
}

# --- 1. a recipe added tomorrow -------------------------------------------

# A throwaway recipe appended to this repository's own `Makefile`. It reaches
# the built executable by putting the build directory on `PATH`, which is how
# the `spec` recipe reaches it, and it satisfies every rule the scan already
# holds: both homes moved into the build directory, both toolchain homes
# pinned. What varies between the shapes below is only which cache variables it
# drops.
#
# The drops are spelled through arguments rather than written inline, so that
# neither this file nor the `Makefile` it writes is read as a run of the built
# executable by the sibling scans over `*.sh`.
plant_appended_recipe() {
    local drops=("$@") prefix='' name
    for name in "${drops[@]}"; do
        prefix+="-u $name "
    done
    {
        cat "$repository/Makefile"
        printf '\n'
        printf '.PHONY: throwaway\n'
        printf 'throwaway:\n'
        printf '\tenv %s' "$prefix"
        printf 'CARGO_HOME="$${CARGO_HOME:-$$HOME/.cargo}" '
        printf 'RUSTUP_HOME="$${RUSTUP_HOME:-$$HOME/.rustup}" '
        printf 'XDG_CACHE_HOME="$(CURDIR)/target/throwaway-cache" '
        printf 'HOME="$(CURDIR)/target/throwaway-cache" '
        printf 'PATH="$(CURDIR)/target/%s:$$PATH" pangopup --version\n' debug
    } >"$tree/Makefile"
}

# Dropping all four is the shape the rule exists to require.
plant_appended_recipe "$model_cache" "$cache_dir" "$data_dir" "$entry_limit"
expect_acceptance "$recipe_gate" \
    'the scan refused a recipe that moves both homes, pins both toolchain homes and drops all four inherited cache variables, so it refuses the shape it exists to require'

# The defect. Dropping the three older names and keeping the entry limit hands
# a run of the built executable whatever limit the operator exported, and the
# scan accepted it. The refusal has to name the variable, and it has to name
# where the recipe stands.
plant_appended_recipe "$model_cache" "$cache_dir" "$data_dir"
expect_refusal "$recipe_gate" \
    "the scan accepted a recipe that reaches the built executable while inheriting $entry_limit" \
    "$entry_limit"
grep -q 'Makefile:' <<<"$GATE_OUTPUT" \
    || fail "the refusal does not say where the recipe stands, so an operator cannot find it: $GATE_OUTPUT"

# The mirror, and the reason the pair is here. This recipe drops the longer
# name and nothing else. `$entry_limit` contains `$model_cache`, so a rule that
# asked whether the shorter name appears in the recipe text would read this
# line as having dropped it and accept. The refusal must name `$model_cache`
# standing on its own rather than as the head of the longer name.
plant_appended_recipe "$entry_limit"
expect_refusal "$recipe_gate" \
    "the scan accepted a recipe that drops only $entry_limit and inherits $model_cache" \
    "$model_cache([^_A-Z]|\$)"

# A makefile whose recipes reach the built executable nowhere has nothing to
# hold, and saying so is the difference between a rule that passed and a rule
# that read nothing.
printf 'noop:\n\t@true\n' >"$tree/Makefile"
expect_refusal "$recipe_gate" \
    'the scan accepted a makefile with no recipe reaching the built executable, so it can pass on an empty set' \
    'nothing'

# --- 2. the cache a routine removal collects -------------------------------

# A recipe of the shape this repository has today: a build line and a spec-suite
# line that both name the same downloaded-library cache, outside the directory
# the recipe itself removes -- and inside `target/`, which `cargo clean`
# removes whole.
plant_download_cache() {
    local cache=$1
    {
        printf 'spec:\n'
        printf '\tenv ORT_CACHE_DIR="%s" cargo build --locked\n' "$cache"
        printf '\trm -rf target/spec-cache\n'
        printf '\tenv ORT_CACHE_DIR="%s" ' "$cache"
        printf 'XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" '
        printf 'HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n'
    } >"$tree/Makefile"
}

plant_download_cache '$(CURDIR)/target/ort-cache'
expect_refusal "$durability_gate" \
    'the rule accepted a recipe that keeps the downloaded ONNX Runtime library inside target/, which cargo clean removes whole, so the next build after a routine clean fetches it again over the network' \
    'cargo clean'

# A cache outside `target/` survives a clean, and refusing it would be a rule
# wider than the thing it covers.
plant_download_cache '$(CURDIR)/.ort-cache'
expect_acceptance "$durability_gate" \
    'the rule refused a recipe whose downloaded ONNX Runtime library cache stands outside every directory a removal collects'

# The boundary decides, not the letters. `cargo clean` removes `target`, and a
# rule that read `target` as a prefix of the path would refuse a sibling
# directory of that name which no command here removes.
plant_download_cache '$(CURDIR)/target-library/ort-cache'
expect_acceptance "$durability_gate" \
    'the rule read target-library as the build directory, so it refuses a cache no removal collects'

printf 'recipe cache rule coverage: the recipe scan tells %s from %s in both directions, and the download-cache rule refuses a library cache a routine removal collects\n' \
    "$entry_limit" "$model_cache"
