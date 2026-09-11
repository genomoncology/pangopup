#!/usr/bin/env bash
set -euo pipefail

# Three variables name a cache location outright, and each is read ahead of
# `XDG_CACHE_HOME` and `HOME`:
#
#   PANGOPUP_MODEL_CACHE  the model cache file itself
#   PANGOPUP_CACHE_DIR    the download cache directory
#   PANGOPUP_DATA_DIR     the installed bundle directory
#
# `tests/support/private-cache-home.sh` moves the two homes and stops there, so
# an operator who exports one of these runs the whole suite against what it
# names. Since a cache whose recorded setup no longer matches is discarded
# rather than ignored, such a run destroys that operator's file rather than
# only growing it. `tests/production-release-qualification.sh` is run by hand,
# on an operator's own machine, against that operator's real cache.
#
# The sibling scans read names in source files. This file reads the other side:
# it exports each variable at a location it owns, runs the shipped executable
# under the helper, and looks at what happened to that location afterwards.
#
# Both helpers are read that way. The shell one is sourced here directly. The
# Rust one is reached by starting the built model-route suite from a shell that
# exported the variables, because no `#[test]` can put them in its own process
# to begin with.
#
# The boundary the ticket draws is in here too. The helper drops what a harness
# inherited; the product keeps honouring all three for an operator running it.
# Proving only the first half would let an implementation that broke the
# variables outright pass.

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
helper_relative='tests/support/private-cache-home.sh'

# Build before taking a cache home: the ONNX Runtime library the build resolves
# lands under XDG_CACHE_HOME, so a build under a fresh one downloads it again.
"$repo/scripts/require-built-commands.sh"
# The Rust model-route suite is built here too, for the same reason, and run
# further down as a plain executable. Calling `cargo` after the cache home is
# taken is what would download the library again.
# Cargo reports an `executable` for the `pangopup` binary as well as for the
# test, and the order of the two is not fixed. Selecting by position picked the
# product binary in 7 of 12 consecutive runs on a built tree, measured on
# 2026-09-10; that run prints the usage block, leaves the stand-in untouched for
# the wrong reason, and fails this file on a tree where nothing is wrong. The
# test executable is selected by its target name and kind instead, and a
# selection that does not find exactly one says so rather than handing back
# whatever else cargo reported.
model_route_suite=$(
    cargo test --locked --no-run --message-format=json \
        --manifest-path "$repo/Cargo.toml" --package pangopup-cli --test model_routing \
        2>/dev/null \
        | python3 -c '
import json, sys

chosen = []
for line in sys.stdin:
    message = json.loads(line)
    if message.get("reason") != "compiler-artifact":
        continue
    executable = message.get("executable")
    target = message.get("target") or {}
    if not executable:
        continue
    if target.get("name") != "model_routing" or "test" not in (target.get("kind") or []):
        continue
    chosen.append(executable)

if len(chosen) != 1:
    sys.stderr.write(
        "inherited cache variables: cargo reported %d test executables named "
        "model_routing, not 1: %s\n" % (len(chosen), chosen)
    )
    raise SystemExit(1)
print(chosen[0])
'
)
[[ -x "$model_route_suite" ]] || {
    printf 'inherited cache variables: no built test executable named model_routing was selected, so its half of this check cannot run\n' >&2
    exit 1
}
# Spelled in full rather than through the variable: the sibling scan reads a
# literal source line, and naming the helper in a variable establishes nothing.
. "$repo/tests/support/private-cache-home.sh"

executable="$repo/target/debug/pangopup"

fail() { printf 'inherited cache variables: %s\n' "$*" >&2; exit 1; }

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
chmod 700 "$scratch"

# The miniature route the cache tests score. A modelled lookup is what opens
# the model cache, so it is what a variable naming that cache can destroy.
model_args=(
    --model-bundle "$repo/tests/fixtures/pangolin-model-kernel-mini/bundle"
    --reference-bundle "$repo/tests/fixtures/reference-route-test/bundle"
    --mask "$repo/tests/fixtures/route-mask/domains.pgm"
)
# Two variants the miniature route scores. Filling the stand-in with one and
# asking for the other under the helper forces a cache write, so a leak shows
# up as a changed file. Asking for the same variant twice would find a hit,
# write nothing, and leave this harness passing on nothing.
lookup_args=(lookup --model-only --variant GRCh38:chr1:5051:A:C "${model_args[@]}")
other_args=(lookup --model-only --variant GRCh38:chr1:5051:A:AC "${model_args[@]}")

# A private directory of this harness's own to stand in for the operator's.
# Nothing under it belongs to the product, and the cache refuses a parent it
# does not find private anyway.
owned="$scratch/owned"
mkdir -p "$owned"
chmod 700 "$owned"

# The fingerprint of a file, or `absent`. Size and content together, so neither
# a truncation nor an in-place rewrite of the same length reads as untouched.
fingerprint() {
    if [[ -f "$1" ]]; then
        printf '%s %s\n' "$(wc -c <"$1")" "$(cksum <"$1" | cut -d' ' -f1)"
    else
        printf 'absent\n'
    fi
}

# --- a file the product filled, standing in for the operator's cache --------
#
# Written through the variable itself rather than copied from anywhere, so this
# harness never reads a cache belonging to whoever runs it.

decoy="$owned/model-results.sqlite3"
PANGOPUP_MODEL_CACHE="$decoy" "$executable" "${lookup_args[@]}" >/dev/null \
    || fail 'the fixture lookup that fills the stand-in cache failed, so there is nothing to protect'
[[ -f "$decoy" ]] \
    || fail "the product wrote no cache at the path PANGOPUP_MODEL_CACHE named, so this harness has no destruction to refuse: $decoy"
before=$(fingerprint "$decoy")

# --- the helper drops what a harness inherited ------------------------------

private_home="$repo/target/shell-harness-cache-home"
under_helper=$(
    env PANGOPUP_MODEL_CACHE="$decoy" \
        PANGOPUP_CACHE_DIR="$owned/downloads" \
        PANGOPUP_DATA_DIR="$owned/bundles" \
        bash -c '
            set -euo pipefail
            . "$1"
            shift
            exec "$@"
        ' bash "$repo/$helper_relative" "$executable" "${other_args[@]}"
) || fail 'the fixture lookup failed under the helper, so nothing was proved about what it reaches'
[[ -n "$under_helper" ]] || fail 'the fixture lookup under the helper printed nothing'

after=$(fingerprint "$decoy")
[[ "$before" == "$after" ]] \
    || fail "a run under the helper reached the model cache PANGOPUP_MODEL_CACHE named and changed it from [$before] to [$after]: that is the operator's file on a real machine"

# A run that cached nothing would leave the stand-in untouched for the wrong
# reason, and this whole file would pass on nothing. The run has to have filled
# a cache of the helper's own instead.
[[ -f "$private_home/pangopup/model-results.sqlite3" ]] \
    || fail "the run under the helper filled no cache under $private_home, so finding the stand-in untouched proves nothing about where the run went"

# --- the Rust helper, proved by running as well ------------------------------
#
# `crates/pangopup-cli/tests/spawn_isolation.rs` reads the command
# `crates/pangopup-cli/tests/support/mod.rs` builds. It cannot do more: a
# `#[test]` may not set `PANGOPUP_MODEL_CACHE` in its own process, because
# `std::env::set_var` is unsound while sibling tests run in other threads, so
# no test in that file ever has the variable to inherit. Reading a `Command`
# proves the helper asked for the removal, not that a run honours it.
#
# The shell can hand the suite the variable the way an operator's shell does.
# `model_routing.rs` is the file whose runs score the miniature model route, so
# it is the file an inherited `PANGOPUP_MODEL_CACHE` destroys a cache through.
# Start it from here with all three exported at locations this harness owns,
# and require the stand-in to come back byte for byte. The suite runs as the
# executable built above rather than through `cargo`, because a build under the
# cache home this file already took would fetch the ONNX Runtime library again.

rust_decoy="$owned/rust-model-results.sqlite3"
PANGOPUP_MODEL_CACHE="$rust_decoy" "$executable" "${lookup_args[@]}" >/dev/null \
    || fail 'the fixture lookup that fills the stand-in for the Rust suite failed, so there is nothing to protect'
[[ -f "$rust_decoy" ]] \
    || fail "the product wrote no cache at $rust_decoy, so the Rust half of this check has no destruction to refuse"
rust_before=$(fingerprint "$rust_decoy")

rust_status=0
env PANGOPUP_MODEL_CACHE="$rust_decoy" \
    PANGOPUP_CACHE_DIR="$owned/rust-downloads" \
    PANGOPUP_DATA_DIR="$owned/rust-bundles" \
    "$model_route_suite" >"$scratch/rust-suite.log" 2>&1 || rust_status=$?

rust_after=$(fingerprint "$rust_decoy")
[[ "$rust_before" == "$rust_after" ]] \
    || fail "the Rust suite, started from a shell that exported PANGOPUP_MODEL_CACHE, reached the file it named and changed it from [$rust_before] to [$rust_after]: that is the operator's file on a real machine"
(( rust_status == 0 )) \
    || fail "the Rust model-route suite failed with the three cache variables exported, which is how an operator who exported one runs it: $(tail -n 20 "$scratch/rust-suite.log")"

# The same wrong reason as above. A suite that scored nothing would leave the
# stand-in untouched and prove nothing about where its runs went.
grep -qE '^test result: ok\. [1-9]' "$scratch/rust-suite.log" \
    || fail "the Rust model-route suite reported no passing test, so finding the stand-in untouched proves nothing about where its runs went: $(tail -n 20 "$scratch/rust-suite.log")"

# The other two name directories rather than a file, so what proves they were
# dropped is that the product stops reporting them. A relative value is refused
# by name whenever it is read at all.
for probe in \
    "PANGOPUP_CACHE_DIR:sync:--offline" \
    "PANGOPUP_DATA_DIR:status:"; do
    name=${probe%%:*}
    rest=${probe#*:}
    subcommand=${rest%%:*}
    option=${rest#*:}
    arguments=("$subcommand")
    [[ -z "$option" ]] || arguments+=("$option")
    [[ "$subcommand" != sync ]] || arguments+=(--data-dir "$owned/bundles")

    inherited=$(
        env "$name=relative" \
            bash -c '
                set -euo pipefail
                . "$1"
                shift
                "$@" 2>&1 || true
            ' bash "$repo/$helper_relative" "$executable" "${arguments[@]}"
    )
    case "$inherited" in
        *"$name must be"*)
            fail "a run under the helper still read $name from what the harness inherited: $inherited" ;;
    esac
done

# --- the product keeps honouring all three for an operator ------------------
#
# The helper drops an inherited value. It does not take the variables away.

fresh="$scratch/fresh"
mkdir -p "$fresh"
chmod 700 "$fresh"
chosen="$fresh/model-results.sqlite3"
PANGOPUP_MODEL_CACHE="$chosen" "$executable" "${lookup_args[@]}" >/dev/null \
    || fail 'the product refused a run whose PANGOPUP_MODEL_CACHE named a private file of its own'
[[ -f "$chosen" ]] \
    || fail "the product no longer writes the model cache PANGOPUP_MODEL_CACHE names, so an operator's own cache location stopped working: $chosen"

for name in PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR; do
    case "$name" in
        PANGOPUP_CACHE_DIR) arguments=(sync --offline --data-dir "$owned/bundles") ;;
        *) arguments=(status) ;;
    esac
    reported=$(env "$name=relative" "$executable" "${arguments[@]}" 2>&1 || true)
    case "$reported" in
        *"$name must be"*) ;;
        *) fail "the product no longer reads $name, so an operator running it lost that setting: $reported" ;;
    esac
done

printf 'both helpers drop PANGOPUP_MODEL_CACHE, PANGOPUP_CACHE_DIR and PANGOPUP_DATA_DIR, and the product still reads all three\n'
