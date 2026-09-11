#!/usr/bin/env bash
set -euo pipefail

# `scripts/run-production-qualification.sh` runs the shipped executable eleven
# times. It is the script the release runbook tells an operator to run by hand,
# on that operator's own machine, against that operator's real data and cache.
#
# It holds the cache-home rule today: it refuses a relative `XDG_DATA_HOME`,
# `XDG_CACHE_HOME` or output directory, it drops the four variables that name a
# pangopup cache location outright, and it sets both homes from its arguments.
# No gate read any of it. The sibling scan over `*.sh` reads the file as quiet,
# because it runs `"$pangopup"` from its first argument and names no path a
# scan can see, so deleting any one of those three lines left every gate green
# and handed a release qualification run the cache of whoever started it.
#
# This file reads it by running it. The executable it is handed is a stub of
# this file's own writing that records the environment it was started with and
# then fails, so the run stops at the first spawn: what the stub recorded is
# what the shipped executable would have been started with, and no bundle is
# synced, no service is started and nothing of the operator's is touched.
#
# Running it is the point. A scan for the text of those three lines would go
# green on a line that had been rewritten to do nothing, and the failure this
# file exists to catch is a run reaching the operator's cache, not a line
# losing its spelling.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
runner_relative='scripts/run-production-qualification.sh'
runner="$repository/$runner_relative"

# The four variables that name a pangopup cache location outright. Each is read
# ahead of `XDG_CACHE_HOME` and `HOME`, so setting the two homes leaves a run
# reaching whatever the operator exported.
# `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` is the fourth: it does not name a
# directory, it names how many rows the cache keeps, and an inherited value
# either refuses every modelled lookup outright or evicts the rows a
# qualification run is about to read.
names=(
    PANGOPUP_MODEL_CACHE
    PANGOPUP_CACHE_DIR
    PANGOPUP_DATA_DIR
    PANGOPUP_MODEL_CACHE_MAX_ENTRIES
)

fail() { printf 'qualification runner cache isolation: %s\n' "$*" >&2; exit 1; }

[[ -f "$runner" ]] \
    || fail "no $runner_relative: the script this file holds is gone, so this gate reads nothing"
[[ -x "$runner" ]] \
    || fail "$runner_relative is not executable, so the runbook's own command cannot start it"

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
chmod 700 "$scratch"

# A stand-in for the operator's own cache locations. Nothing under here belongs
# to the product; the point is that the runner must reach none of it.
owned="$scratch/owned"
mkdir -p "$owned"
chmod 700 "$owned"

# The executable the runner is handed. It records the environment it was
# started with and fails, so the runner stops at its first spawn. The
# destination is written into the stub rather than passed through the
# environment, because the environment is the thing under test.
stub="$scratch/pangopup"
dump="$scratch/environment"
{
    printf '#!/usr/bin/env bash\n'
    printf 'env >%q\n' "$dump"
    printf 'exit 1\n'
} >"$stub"
chmod 700 "$stub"

# The runner's five arguments, with a fresh absent directory for each of the
# three it creates. Counted so that a rule reading none of them says so.
checks=0
runs=0

# A working directory of this file's own to start the runner in. A relative
# path the runner resolved would land here rather than in the checkout, which
# is both what keeps this gate from writing into the tree and what lets the
# rule below see that nothing was created.
working="$scratch/working"
mkdir -p "$working"

# Start the runner with $1 as XDG_DATA_HOME, $2 as XDG_CACHE_HOME and $3 as the
# output directory, every cache variable exported at a location this file owns.
# Prints the runner's combined output; returns its status.
start_runner() {
    local data_home=$1 cache_home=$2 output_dir=$3
    (
        cd "$working" || exit 1
        env \
            PANGOPUP_MODEL_CACHE="$owned/inherited.sqlite3" \
            PANGOPUP_CACHE_DIR="$owned/inherited-downloads" \
            PANGOPUP_DATA_DIR="$owned/inherited-bundles" \
            PANGOPUP_MODEL_CACHE_MAX_ENTRIES=17 \
            "$runner" "$stub" "$repository" "$data_home" "$cache_home" "$output_dir" 2>&1
    )
}

# A fresh set of three absent directories under the scratch tree.
absent_set() {
    local name=$1
    printf '%s/%s-data\n%s/%s-cache\n%s/%s-out\n' \
        "$scratch" "$name" "$scratch" "$name" "$scratch" "$name"
}

# --- 1. the runner refuses a runtime path that is not absolute --------------
#
# A relative `XDG_DATA_HOME`, `XDG_CACHE_HOME` or output directory is resolved
# against whatever directory the operator happened to be standing in, which is
# how a qualification run lands in a tree it does not own. The runner refuses
# all three before it creates anything.

for position in data cache output; do
    mapfile -t paths < <(absent_set "relative-$position")
    case "$position" in
        data) paths[0]=relative/data ;;
        cache) paths[1]=relative/cache ;;
        output) paths[2]=relative/out ;;
    esac
    rm -f "$dump"
    status=0
    reported=$(start_runner "${paths[@]}") || status=$?
    checks=$((checks + 1))
    (( status != 0 )) \
        || fail "the runner accepted a relative $position path, so a qualification run resolves it against whatever directory the operator was standing in"
    if [[ -e "$dump" ]]; then
        fail "the runner started the executable although the $position path it was given was relative, so a qualification run resolves that path against whatever directory the operator was standing in"
    fi
    case "$reported" in
        *'must be absolute'*) ;;
        *) fail "the runner refused a relative $position path without saying that runtime paths must be absolute, so an operator cannot act on it: $reported" ;;
    esac
    for path in relative/data relative/cache relative/out; do
        if [[ -e "$working/$path" ]]; then
            fail "the runner created $path relative to the directory it was started in before refusing it, so a relative runtime path lands in whatever tree the operator was standing in"
        fi
    done
done

# --- 2. the first spawn runs under the homes the runner was given -----------
#
# The runner is started with every cache variable exported at a location this
# file owns, and the executable it is handed records what it was started with.

mapfile -t paths < <(absent_set held)
status=0
reported=$(start_runner "${paths[@]}") || status=$?
checks=$((checks + 1))

[[ -f "$dump" ]] \
    || fail "the runner never started the executable it was handed, so nothing was recorded about the environment it would run under: $reported"
runs=$((runs + 1))

# The stub fails, so the runner is expected to stop. A runner that carried on
# past a failing first command would be a different defect and is not this
# file's business; what matters is that the spawn happened.
(( status != 0 )) \
    || fail 'the runner reported success although the executable it was handed failed, so its own failure reporting is broken'

recorded() {
    local name=$1 line
    line=$({ grep -m 1 -E "^$name=" "$dump" || true; })
    printf '%s' "${line#*=}"
}

present() {
    grep -qE "^$1=" "$dump"
}

# Both homes, taken from the arguments rather than from whoever started the
# runner. They are compared as paths rather than as text so that a runner which
# starts naming them some other way still passes while it points a run at the
# directories it was given.
[[ "$(recorded XDG_DATA_HOME)" == "${paths[0]}" ]] \
    || fail "the runner started the executable with XDG_DATA_HOME=[$(recorded XDG_DATA_HOME)] rather than the directory it was given, ${paths[0]}, so a qualification run reads the installed bundles of whoever started it"
[[ "$(recorded XDG_CACHE_HOME)" == "${paths[1]}" ]] \
    || fail "the runner started the executable with XDG_CACHE_HOME=[$(recorded XDG_CACHE_HOME)] rather than the directory it was given, ${paths[1]}, so a qualification run reads the model cache of whoever started it and can discard that file"

# `HOME` as well as `XDG_CACHE_HOME`, because a run whose `XDG_CACHE_HOME` is
# cleared falls back to `$HOME/.cache`. It has to stand inside the output
# directory the runner was given, not merely be set to something.
home=$(recorded HOME)
[[ -n "$home" ]] \
    || fail 'the runner started the executable with no HOME, so a run whose XDG_CACHE_HOME is cleared has no cache home of its own'
case "$home/" in
    "${paths[2]}"/*) ;;
    *) fail "the runner started the executable with HOME=[$home], which is not inside the output directory it was given, ${paths[2]}: a run whose XDG_CACHE_HOME is cleared falls back to the home directory of whoever started it" ;;
esac

# The four variables that name a cache location outright, each exported at a
# location this file owns before the runner started. A run that still carries
# one reaches what the operator named, whatever the two homes say.
for name in "${names[@]}"; do
    checks=$((checks + 1))
    if present "$name"; then
        fail "the runner started the executable with $name=[$(recorded "$name")] inherited from whoever ran it: that variable names a cache location outright and is read ahead of XDG_CACHE_HOME, so the run reaches the operator's own file and can destroy it"
    fi
done

# --- 3. the recording proves something ---------------------------------------
#
# A stub that recorded an empty environment would satisfy every absence above
# for the wrong reason, and this whole file would pass on nothing.

[[ -s "$dump" ]] \
    || fail 'the executable the runner started recorded an empty environment, so finding the cache variables absent proves nothing'
present PATH \
    || fail 'the executable the runner started recorded no PATH, so the recording is not an environment and the absences above prove nothing'

(( runs > 0 )) \
    || fail 'the runner started nothing, so this gate held over no run'
(( checks > 0 )) \
    || fail 'this gate made no check, so it read nothing'

printf '%s ran under %s check(s) across %s recorded spawn(s): both homes taken from its arguments, every runtime path required absolute, and all %s inherited cache variables dropped\n' \
    "$runner_relative" "$checks" "$runs" "${#names[@]}"
