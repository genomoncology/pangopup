#!/usr/bin/env bash
set -euo pipefail

# `cargo clean` removes the build directory. It stops at the first directory it
# cannot write into, with status 101, having already removed part of `target/`:
#
#     error: failed to remove directory `.../bundles/qualified/bundle`
#     Caused by: Permission denied (os error 13)
#
# Directories like that are made on purpose. A release that cannot write into an
# installed bundle is part of what `tests/production-release-qualification.sh`
# qualifies, and the product itself installs bundle directories at mode 0555,
# which is what leaves them under `target/spec/` after a spec run. The harnesses
# keep making them. What must not survive is the residue: once a harness has
# finished, nothing it left behind may be a directory its owner cannot write.
#
# This file holds that in two places. Section 1 runs a harness in a throwaway
# root -- one that restores what it made and one that does not -- so the check
# is shown to tell the two apart and to name what it refuses. Section 2 reads
# the real build directory, which is the measurement the ticket asks for: a tree
# every gate has run over holds no directory `cargo clean` would stop at. It
# reads modes rather than running `cargo clean`, because running it would throw
# away the build.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
build=$repository/target

fail() { printf 'build directory residue: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'chmod -R u+w "$work" 2>/dev/null || true; rm -rf "$work"' EXIT

# --- the check --------------------------------------------------------------

# Every directory under $1 its owner cannot write into, relative to $1, sorted.
# Prints nothing when there are none.
unwritable_directories() {
    local root=$1
    find "$root" -type d -not -writable -printf '%P\n' | sort
}

# How many directories stand under $1 at all. A check that read none proved
# nothing, whatever it reports.
directory_count() {
    find "$1" -type d -printf '.\n' | wc -l
}

# --- 1. the check tells a restored harness from a leftover one ---------------
#
# Both fixture harnesses make the same unwritable directory and both assert it
# is unwritable while they run, so neither of them is a harness that quietly
# stopped needing one. They differ only in whether the residue outlives them.

harness_body=(
    'set -euo pipefail'
    'root=$1'
    'install -d -m 700 "$root/data/bundles/qualified/bundle"'
    'printf received >"$root/data/bundles/qualified/bundle/receipt.json"'
    'chmod 555 "$root/data/bundles/qualified" "$root/data/bundles/qualified/bundle"'
    '[[ -w "$root/data/bundles/qualified/bundle" ]] && exit 3'
    'printf "%s\\n" qualified'
)

leaving=$work/leaving.sh
printf '%s\n' '#!/usr/bin/env bash' "${harness_body[@]}" >"$leaving"

restoring=$work/restoring.sh
printf '%s\n' '#!/usr/bin/env bash' \
    'trap '"'"'chmod -R u+w "$1" 2>/dev/null || true'"'"' EXIT' \
    "${harness_body[@]}" >"$restoring"

run_harness() {
    local script=$1 root=$2 output
    rm -rf "$root"
    install -d -m 700 "$root"
    output=$(bash "$script" "$root") \
        || fail "the fixture harness $script did not finish, so neither half of this section proves anything"
    [[ "$output" == qualified ]] \
        || fail "the fixture harness $script printed '$output' rather than the work it claims to have done"
}

left_root=$work/left
run_harness "$leaving" "$left_root"
left=$(unwritable_directories "$left_root")
left_count=$(printf '%s\n' "$left" | grep -c .) || true
(( left_count == 2 )) \
    || fail "the check found $left_count unwritable directory/directories after a harness that left two standing, so it is not reading directory modes: $left"
case "$left" in
    *data/bundles/qualified/bundle*) ;;
    *) fail "the check did not name the directory the harness left unwritable: $left" ;;
esac

# This is the failure `cargo clean` reports, reproduced without running it: a
# removal of the root stops on the directory the harness left standing.
rm -rf "$left_root" 2>/dev/null \
    && fail 'the root a harness left unwritable was removed anyway, so this machine cannot show what cargo clean stops at and the rule would be resting on nothing'
chmod -R u+w "$left_root"
rm -rf "$left_root"

kept_root=$work/kept
run_harness "$restoring" "$kept_root"
kept=$(unwritable_directories "$kept_root")
[[ -z "$kept" ]] \
    || fail "a harness that restores what it made still left an unwritable directory behind, so the fixture, not the rule, is wrong: $kept"
(( $(directory_count "$kept_root") >= 4 )) \
    || fail 'the restoring harness left fewer directories than it makes, so the check above passed by having nothing to read'
rm -rf "$kept_root" \
    || fail 'a root whose harness restored what it made could not be removed'

# --- 2. the real build directory --------------------------------------------

if [[ ! -d "$build" ]]; then
    printf 'build directory residue: no build directory, so nothing has been left in one\n'
    exit 0
fi

examined=$(directory_count "$build")
(( examined > 0 )) \
    || fail 'the build directory holds no directories at all, so this scan read nothing'

standing=$(unwritable_directories "$build")
if [[ -n "$standing" ]]; then
    while IFS= read -r relative; do printf 'target/%s\n' "$relative" >&2; done <<<"$standing"
    fail "$(printf '%s\n' "$standing" | grep -c .) directory/directories above are ones their owner cannot write, so cargo clean stops on the first of them with status 101 and leaves the build directory half removed; a harness that needs an unwritable directory must restore the mode before it finishes, and chmod -R u+w target clears what earlier runs left"
fi

printf 'build directory residue: %s directory/directories under the build directory, none of them unwritable\n' \
    "$examined"
