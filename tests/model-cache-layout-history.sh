#!/usr/bin/env bash
set -euo pipefail

# The model cache judges a file by the layout stamped on it. Two constants in
# `crates/pangopup-cache/src/lib.rs` carry that judgement: `USER_VERSION` is the
# layout this build writes, and `EARLIER_USER_VERSIONS` names the layouts
# earlier releases wrote, which are discarded whole instead of refused.
#
# Both can rot silently, and both already have.
#
# Ticket 0058 changed the on-disk shape of the cache and left `USER_VERSION` at
# 1. `make lint`, `make test` and `make spec` all passed. A reader caught it,
# not a gate. Had it shipped, every file a v0.5.x release wrote would carry a
# layout stamp claiming a shape it did not have.
#
# The other half is the reverse mistake: moving `USER_VERSION` without
# appending the layout it replaced. Commit `e7bfa6a` repaired the measured cost
# of that -- every cache an earlier release wrote fails an explicitly named
# `--model-cache` path with `MODEL_CACHE_INVALID` and exit 1, and is destroyed
# with no report on the default path.
#
# Nothing else in the checkout reads either constant, so this holds them to the
# only durable record of what the cache has ever written: git. Every tag, and
# every commit that touched the cache source, is read for the layout it stamped
# and the on-disk shape it wrote, and the tree is refused when that pair
# disagrees with the running source. Tags alone would not do it. Layout 2 is
# not tagged, so a gate reading tags alone would let a bump to 3 drop 2 -- the
# exact loss commit `e7bfa6a` repaired.
#
# What this does not prove: that a discarded file is discarded whole or
# reported. That is a runtime property, and `crates/pangopup-cache/src/lib.rs`
# and `crates/pangopup-cli/tests/model_cache_setup.rs` prove it. This file
# proves only that the two constants still describe what was released.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_relative='crates/pangopup-cache/src/lib.rs'

fail() { printf 'model cache layout history: %s\n' "$*" >&2; exit 1; }

# The layout a source stamps on the files it writes. Empty when the constant
# cannot be read, which is a refusal rather than a pass: a renamed constant
# must not turn this gate into a scan that examines nothing.
layout_stamp() {
    sed -nE 's/^const USER_VERSION: i32 = ([0-9]+);$/\1/p' "$1" | head -1
}

# The layouts a source says earlier releases wrote, one per line.
named_layouts() {
    sed -nE 's/^const EARLIER_USER_VERSIONS: \[i32; [0-9]+\] = \[(.*)\];$/\1/p' "$1" \
        | head -1 | tr -d '[:space:]' | tr ',' '\n' | grep -E '^[0-9]+$' || true
}

# The on-disk shape a source creates: the initialization batch, from the
# journal-mode pragma through the last table it declares. The `user_version`
# pragma is dropped because it is the stamp being judged, not part of the shape
# it stamps, and whitespace is squeezed so that reindenting a DDL line is not
# read as a layout change.
on_disk_shape() {
    sed -n '/PRAGMA journal_mode=WAL;/,/) STRICT;"/p' "$1" \
        | grep -v 'PRAGMA user_version=' \
        | tr -s '[:space:]' ' '
}

# Refuse the running source at $1 against the earlier sources that follow, each
# given as `<where>=<path>`. Prints its counts on acceptance and its reason on
# refusal.
examine() {
    local current=$1 where path stamp shape
    shift
    local current_stamp current_shape named examined=0

    current_stamp=$(layout_stamp "$current")
    current_shape=$(on_disk_shape "$current")
    if [[ -z "$current_stamp" ]]; then
        printf 'read no USER_VERSION from %s, so this check examined nothing and could not judge any release\n' \
            "$current" >&2
        return 1
    fi
    if [[ -z ${current_shape// /} ]]; then
        printf 'read no on-disk shape from %s, so this check examined nothing and could not judge any release\n' \
            "$current" >&2
        return 1
    fi
    named=$(named_layouts "$current")

    for pair in "$@"; do
        where=${pair%%=*}
        path=${pair#*=}
        stamp=$(layout_stamp "$path")
        shape=$(on_disk_shape "$path")
        if [[ -z "$stamp" || -z ${shape// /} ]]; then
            printf '%s carries the cache but this check could not read its layout, so it read no earlier layout and proved nothing about it\n' \
                "$where" >&2
            return 1
        fi
        examined=$((examined + 1))

        if [[ "$stamp" == "$current_stamp" && "$shape" != "$current_shape" ]]; then
            printf '%s wrote layout %s and this build writes a different on-disk shape under that same layout %s, so every file this build writes claims a shape it does not have and every file %s wrote is read as though it did: bump USER_VERSION and append %s to EARLIER_USER_VERSIONS\n' \
                "$where" "$stamp" "$current_stamp" "$where" "$stamp" >&2
            return 1
        fi

        if [[ "$stamp" != "$current_stamp" ]] && ! printf '%s\n' "$named" | grep -qx "$stamp"; then
            printf '%s wrote layout %s and this build writes layout %s without naming %s in EARLIER_USER_VERSIONS, so every cache %s wrote is refused as foreign rather than discarded: append %s\n' \
                "$where" "$stamp" "$current_stamp" "$stamp" "$where" "$stamp" >&2
            return 1
        fi
    done

    if (( examined == 0 )); then
        printf 'found no earlier checkout carrying %s, so this check read no earlier layout and proved nothing\n' \
            "$source_relative" >&2
        return 1
    fi

    printf 'examined %s earlier layout(s) against layout %s\n' "$examined" "$current_stamp"
}

# --- the scanner refuses what it exists to refuse ---------------------------
#
# A check that reports nothing wrong is worth exactly as much as its ability to
# report something wrong, so each refusal is exercised against fixture sources
# before the real history is read.

fixtures=$(mktemp -d)
trap 'rm -rf "$fixtures"' EXIT

# A miniature cache source: $2 is the layout it stamps, $3 the layouts it names,
# $4 a column that stands in for the whole on-disk shape.
plant() {
    cat >"$fixtures/$1" <<PLANTED
const APPLICATION_ID: i32 = 0x5047_5043;
const USER_VERSION: i32 = $2;
const EARLIER_USER_VERSIONS: [i32; 9] = [$3];
                    "PRAGMA journal_mode=WAL;
                     PRAGMA application_id={APPLICATION_ID};
                     PRAGMA user_version={USER_VERSION};
                     CREATE TABLE IF NOT EXISTS entries (
                       key_digest TEXT PRIMARY KEY,
                       $4 TEXT NOT NULL
                     ) STRICT;"
PLANTED
}

expect_refusal() {
    local wanted=$1 output status
    shift
    set +e
    output=$(examine "$@" 2>&1)
    status=$?
    set -e
    (( status != 0 )) || fail "the scanner accepted a history it must refuse: $wanted"
    case "$output" in
        *"$wanted"*) ;;
        *) fail "the refusal does not name $wanted, so a maintainer cannot act on it: $output" ;;
    esac
}

plant released 1 '' value_json
plant unchanged 1 '' value_json
plant shape-moved-silently 1 '' scored_json
plant layout-bumped 2 '1' scored_json
plant layout-bumped-unnamed 2 '' scored_json
plant nameless 2 '1' scored_json
sed -i.bak '/USER_VERSION: i32/d' "$fixtures/nameless"
plant shapeless 2 '1' scored_json
sed -i.bak '/PRAGMA journal_mode=WAL;/,/) STRICT;"/d' "$fixtures/shapeless"

# Ticket 0058's defect: the on-disk shape moved and the stamp stayed put.
expect_refusal 'bump USER_VERSION and append 1' \
    "$fixtures/shape-moved-silently" "v9.9.9=$fixtures/released"

# The reverse: the stamp moved and the layout it replaced went unnamed.
expect_refusal 'without naming 1 in EARLIER_USER_VERSIONS' \
    "$fixtures/layout-bumped-unnamed" "v9.9.9=$fixtures/released"

# A renamed or deleted constant must refuse rather than quietly judge nothing.
expect_refusal 'examined nothing' \
    "$fixtures/nameless" "v9.9.9=$fixtures/released"
expect_refusal 'examined nothing' \
    "$fixtures/shapeless" "v9.9.9=$fixtures/released"
expect_refusal 'proved nothing about it' \
    "$fixtures/layout-bumped" "v9.9.9=$fixtures/nameless"

# No release carrying the cache at all is not a pass.
expect_refusal 'proved nothing' "$fixtures/released"

examine "$fixtures/unchanged" "v9.9.9=$fixtures/released" >/dev/null \
    || fail 'the scanner refused a build whose shape and stamp both match the release, which is the shape it exists to accept'
examine "$fixtures/layout-bumped" "v9.9.9=$fixtures/released" >/dev/null \
    || fail 'the scanner refused a bump that names the layout it replaced, which is exactly how a layout change is meant to be made'

# A second bump means a list of more than one layout. Reading only the first
# entry of it would refuse every correct history from the second bump onward,
# so the list is exercised at the length that has more than one element in it.
plant layout-bumped-twice 3 '1, 2' minted_json
examine "$fixtures/layout-bumped-twice" "v9.9.8=$fixtures/released" "v9.9.9=$fixtures/layout-bumped" >/dev/null \
    || fail 'the scanner read only part of EARLIER_USER_VERSIONS, so it refuses every history with more than one earlier layout in it'

# --- the real history -------------------------------------------------------

# A clone with no history reads one commit, finds that its only earlier state
# is the state it is judging, and agrees with itself. That is the shape of a
# check that passes on nothing, so it is refused rather than run.
if [[ $(git -C "$repository" rev-parse --is-shallow-repository) == true ]]; then
    fail 'this is a shallow clone, so the layouts earlier commits wrote are not here to read and this check would agree with itself. Fetch the full history and its tags.'
fi

history=$(mktemp -d)
trap 'rm -rf "$fixtures" "$history"' EXIT

# Every tag, and every commit that touched the cache source. A layout can be
# replaced without ever being tagged -- layout 2 was -- and a file stamped with
# it still sits on the disk of anyone who ran that build, so a scan reading
# tags alone measures the history it finds convenient rather than the history
# that exists. Identical blobs are read once.
earlier=()
seen=' '
while IFS= read -r revision; do
    [[ -n "$revision" ]] || continue
    blob=$(git -C "$repository" rev-parse --quiet --verify "${revision}:${source_relative}" 2>/dev/null) || continue
    [[ "$seen" == *" $blob "* ]] && continue
    seen+="$blob "
    git -C "$repository" cat-file blob "$blob" >"$history/$blob"
    earlier+=("$revision=$history/$blob")
done < <(git -C "$repository" tag; git -C "$repository" log --format=%H -- "$source_relative")

if (( ${#earlier[@]} == 0 )); then
    fail "found no earlier checkout carrying $source_relative, so this check read no earlier layout and proved nothing. A clone with no history reaches this: fetch it."
fi

examine "$repository/$source_relative" "${earlier[@]}" || exit 1
