#!/usr/bin/env bash
set -euo pipefail

# The model cache is on by default and its directory comes from
# `XDG_CACHE_HOME`, or from `HOME` when that is unset. A shell harness that
# runs the built `pangopup` executable without redirecting both reaches the
# cache file of whoever is running the suite. Since ticket 0058 a cache whose
# recorded setup no longer matches is discarded rather than ignored, so such a
# run destroys that file instead of merely adding fixture rows to it.
#
# `tests/production-release-qualification.sh` is one of these harnesses and it
# is the one run against a real service to qualify a release, on an operator's
# own machine, against that operator's real cache.
#
# Ticket 0069 made the rule structural for the Rust suite. Its gate,
# `tests/cli-spawn-cache-isolation.sh`, reads `*.rs` only. This file is the
# other half: every shell file that names the built executable holds a cache
# home of its own, established before the first run.
#
# The rule is deliberately not "an invocation that resolves a default cache
# needs a redirect". Whether a given command line resolves one depends on which
# arguments it happens to carry, and reading argument shape out of shell is the
# fragile form -- it is the accident this ticket exists to remove, not a rule.
# A file that names the executable holds a cache home, full stop.
#
# The cache home is established by sourcing one helper, so this check has a
# single token to look for rather than a family of export spellings, and so the
# two dozen ways to get the redirect subtly wrong live in one reviewed file.
#
# Two things this file does not prove:
#
#   * That the helper redirects anything. That is a runtime property, and the
#     second half of this file exercises it directly.
#   * That `scripts/smoke-linux-release.sh` holds a cache home. It runs an
#     executable handed to it as an argument and names none, so no name scan
#     can see it. On the host its only caller is `tests/executable-delivery.sh`,
#     which is in scope here and whose cache home it inherits through the
#     environment. `.github/workflows/package-linux.yml` runs it inside a
#     container against that container's own filesystem, where there is no
#     operator cache to reach.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
helper_relative='tests/support/private-cache-home.sh'

# A run of the built executable. `pangopup-build` is a different binary with no
# model cache, so the trailing class keeps `target/debug/pangopup-build` out.
# `cargo run --bin pangopup` builds and runs the same executable by another
# name and is a run like any other.
#
# Only code counts. A line that mentions a path inside a comment runs nothing,
# which is why every pattern here is anchored at `^[^#]*`.
use='^[^#]*target/(debug|release)/pangopup([^-]|$)|^[^#]*--bin[[:space:]]+pangopup([^-]|$)'

# Sourcing the helper is what establishes the cache home.
establish='^[^#]*tests/support/private-cache-home\.sh'

# A build step. `scripts/require-built-commands.sh` is the one wrapper around
# `cargo build` the harnesses call, and it counts as a build wherever it
# appears.
build='^[^#]*([^-[:alnum:]_]|^)cargo[[:space:]]|^[^#]*scripts/require-built-commands\.sh'

# One file, named in full. `tests/built-executable-currency.sh` writes stub
# executables into a fixture tree at those paths and runs the shipped one
# nowhere; holding it to a cache home would be asking it to redirect a `printf`.
#
# The exemption is an exact relative path compared as a string rather than a
# pattern. A pattern like `built-executable-currency.sh$` would also exempt a
# file of that name planted anywhere in the tree, and a run planted there is a
# run like any other.
exempt='tests/built-executable-currency.sh'

fail() { printf 'shell spawn cache isolation: %s\n' "$*" >&2; exit 1; }

# The first line number in $1 matching the extended pattern $2, or empty.
first_line() {
    { grep -nE -- "$2" "$1" || true; } | head -n 1 | cut -d: -f1
}

# Refuse the tree rooted at $1. Prints its counts on acceptance, its reason on
# refusal. `runners` is left holding the relative paths that ran the executable.
runners=()
examine() {
    local root=$1
    local sources=() source relative use_line establish_line build_line scanned

    runners=()
    while IFS= read -r source; do sources+=("$source"); done < <(
        find "$root" -type f -name '*.sh' -not -path '*/target/*' | sort
    )
    scanned=${#sources[@]}
    if (( scanned == 0 )); then
        printf 'found no shell source under %s, so this check examined nothing\n' "$root" >&2
        return 1
    fi

    for source in "${sources[@]}"; do
        relative=${source#"$root/"}
        [[ "$relative" != "$exempt" ]] || continue
        use_line=$(first_line "$source" "$use")
        [[ -n "$use_line" ]] || continue
        runners+=("$relative")

        establish_line=$(first_line "$source" "$establish")
        if [[ -z "$establish_line" ]]; then
            printf 'this harness runs the built executable without a cache home of its own, so it reaches the model cache of whoever runs it and can discard that person'"'"'s cache: %s:%s\n' \
                "$relative" "$use_line" >&2
            printf 'source %s before that line\n' "$helper_relative" >&2
            return 1
        fi
        if (( establish_line > use_line )); then
            printf 'this harness runs the built executable at %s:%s but takes its cache home at line %s, so the runs before that line reach the model cache of whoever runs it\n' \
                "$relative" "$use_line" "$establish_line" >&2
            return 1
        fi

        build_line=$(first_line "$source" "$build")
        if [[ -n "$build_line" ]] && (( build_line > establish_line )); then
            printf 'this harness takes its cache home at %s:%s and then builds at line %s: the ONNX Runtime library the build resolves lands under XDG_CACHE_HOME, so a build under a fresh cache home downloads it again instead of linking the copy already there. Build first, take the cache home after.\n' \
                "$relative" "$establish_line" "$build_line" >&2
            return 1
        fi
    done

    if (( ${#runners[@]} == 0 )); then
        printf 'examined %s shell source(s) under %s and found none that runs the built executable, so this check read no run and proved nothing\n' \
            "$scanned" "$root" >&2
        return 1
    fi

    printf 'examined %s shell source(s), %s harness(es) running the built executable, each holding a cache home of its own\n' \
        "$scanned" "${#runners[@]}"
}

# --- the scanner refuses what it exists to refuse ---------------------------
#
# A check that reports nothing wrong is worth exactly as much as its ability to
# report something wrong, so each refusal is exercised against a fixture tree
# before the real tree is read.

fixtures=$(mktemp -d)
trap 'rm -rf "$fixtures"' EXIT

# Plant the shell file $2 in tree $1 with shape $3. The executable's directory
# name is spelled through a `printf` argument rather than inline, twice over:
# this file is a harness under `tests/`, `tests/built-executable-currency.sh`
# reads a literal debug path in code as a harness running a built executable,
# and a fixture written inline here would make this file's own scan read it as
# a harness that runs the executable.
plant() {
    local tree=$1 path=$2 shape=$3
    mkdir -p "$tree/$(dirname "$path")"
    {
        printf '#!/usr/bin/env bash\n'
        case "$shape" in
            quiet)
                printf 'printf ok\n'
                ;;
            runs)
                printf '"$repo/target/%s/pangopup" --version\n' debug
                ;;
            release)
                printf '"$repo/target/%s/pangopup" --version\n' release
                ;;
            builds-only)
                printf '"$repo/target/%s/pangopup-build" --version\n' debug
                ;;
            cargo-run)
                printf 'cargo run --package pangopup-cli --bin %s -- lookup --help\n' pangopup
                ;;
            held)
                printf 'scripts/require-built-commands.sh\n'
                printf '. "$repo/%s"\n' "$helper_relative"
                printf '"$repo/target/%s/pangopup" --version\n' debug
                ;;
            held-late)
                printf '"$repo/target/%s/pangopup" --version\n' debug
                printf '. "$repo/%s"\n' "$helper_relative"
                ;;
            held-then-builds)
                printf '. "$repo/%s"\n' "$helper_relative"
                printf 'cargo build --locked\n'
                printf '"$repo/target/%s/pangopup" --version\n' debug
                ;;
            mentions)
                printf '# a comment about $repo/target/%s/pangopup runs nothing\n' debug
                ;;
        esac
    } >"$tree/$path"
}

expect_refusal() {
    local tree=$1 wanted=$2 output status
    set +e
    output=$(examine "$tree" 2>&1)
    status=$?
    set -e
    (( status != 0 )) || fail "the scanner accepted a tree it must refuse: $wanted"
    case "$output" in
        *"$wanted"*) ;;
        *) fail "the refusal does not name $wanted, so an operator cannot act on it: $output" ;;
    esac
}

expect_acceptance() {
    local tree=$1 why=$2
    examine "$tree" >/dev/null || fail "$why"
}

empty="$fixtures/empty"
mkdir -p "$empty"
expect_refusal "$empty" 'examined nothing'

# A tree of harnesses none of which runs the executable is the same refusal: the
# scan read no run, so it checked nothing. A file that only names
# `pangopup-build`, and a file that names the executable only in a comment, are
# both in this tree -- neither is a run, and if either were counted as one this
# tree would be accepted and the refusal would go unseen.
silent="$fixtures/silent"
plant "$silent" 'tests/quiet.sh' quiet
plant "$silent" 'tests/builder.sh' builds-only
plant "$silent" 'tests/commentary.sh' mentions
expect_refusal "$silent" 'proved nothing'

stray="$fixtures/stray"
plant "$stray" 'tests/quiet.sh' quiet
plant "$stray" 'tests/qualification.sh' runs
expect_refusal "$stray" 'tests/qualification.sh:2'

# The release profile is the same executable. A scan that read only the debug
# one would pass the release qualification harness it exists to hold.
release_tree="$fixtures/release"
plant "$release_tree" 'tests/qualification.sh' release
expect_refusal "$release_tree" 'tests/qualification.sh:2'

# `cargo run --bin pangopup` reaches the same executable without naming a path.
cargo_tree="$fixtures/cargo"
plant "$cargo_tree" 'tests/help-contract.sh' cargo-run
expect_refusal "$cargo_tree" 'tests/help-contract.sh:2'

# A cache home taken after the first run leaves that run reaching the operator's
# cache, which is the whole failure.
late="$fixtures/late"
plant "$late" 'tests/qualification.sh' held-late
expect_refusal "$late" 'tests/qualification.sh:2'

# The boundary, exercised rather than only described.
early="$fixtures/early"
plant "$early" 'tests/qualification.sh' held-then-builds
expect_refusal "$early" 'ONNX Runtime'

# The exemption names one path, not one basename. A file of that name anywhere
# else runs the executable like any other harness.
lookalike="$fixtures/lookalike"
plant "$lookalike" 'crates/pangopup-cli/tests/built-executable-currency.sh' runs
expect_refusal "$lookalike" 'crates/pangopup-cli/tests/built-executable-currency.sh:2'

clean="$fixtures/clean"
plant "$clean" 'tests/quiet.sh' quiet
plant "$clean" 'tests/qualification.sh' held
expect_acceptance "$clean" \
    'the scanner refused a harness that builds, takes a cache home and only then runs the executable, so it refuses the shape it exists to require'

# The one exempted path, exercised rather than only described.
exempted="$fixtures/exempted"
plant "$exempted" 'tests/qualification.sh' held
plant "$exempted" "$exempt" runs
expect_acceptance "$exempted" \
    "the scanner refused $exempt for writing stub executables into a fixture tree, which this check has no business demanding"

# --- the helper redirects ---------------------------------------------------
#
# The scan above reads the calling side: it proves every harness takes a cache
# home. Should the helper stop redirecting, every one of those harnesses would
# reach the operator's cache and the scan would still be green, because they
# would still be taking a cache home. This reads the other side.

helper="$repository/$helper_relative"
[[ -f "$helper" ]] \
    || fail "no $helper_relative: nothing gives a shell harness a cache home of its own"

ambient="$fixtures/ambient"
mkdir -p "$ambient/home" "$ambient/cache"
chmod 700 "$ambient/home" "$ambient/cache"

# The default model cache one environment resolves, spelled the way the product
# spells it: `XDG_CACHE_HOME` when set, otherwise `HOME/.cache`.
resolved=$(
    env -u PANGOPUP_MODEL_CACHE HOME="$ambient/home" XDG_CACHE_HOME="$ambient/cache" \
        bash -c '
            set -euo pipefail
            . "$1"
            printf "%s\n%s\n" "${XDG_CACHE_HOME-}" "${HOME-}"
        ' bash "$helper"
) || fail 'sourcing the helper failed'

private_cache=$(printf '%s\n' "$resolved" | sed -n '1p')
private_home=$(printf '%s\n' "$resolved" | sed -n '2p')

[[ -n "$private_cache" ]] \
    || fail 'the helper must set XDG_CACHE_HOME, or a run reads the cache directory of whoever runs the harness'
[[ -n "$private_home" ]] \
    || fail 'the helper must set HOME, or a run whose XDG_CACHE_HOME is later cleared falls back to the home directory of whoever runs the harness'

[[ -d "$private_cache" ]] \
    || fail "the helper's XDG_CACHE_HOME must be a directory that exists, and $private_cache is not"
# The two variables together decide one path, so compare the paths a run would
# actually open rather than the variables.
[[ "$private_cache/pangopup/model-results.sqlite3" != "$ambient/cache/pangopup/model-results.sqlite3" ]] \
    || fail 'a run under the helper resolves the model cache of whoever runs the harness'
[[ "$private_home/.cache/pangopup/model-results.sqlite3" != "$ambient/home/.cache/pangopup/model-results.sqlite3" ]] \
    || fail 'a run under the helper whose XDG_CACHE_HOME is cleared falls back to the cache of whoever runs the harness'

# A cache home nested inside the ambient one is the ambient one's problem when
# it is discarded, and inherits its permissions besides.
case "$private_cache/" in
    "$ambient/cache/"*|"$ambient/home/"*)
        fail "the helper's cache home sits inside the ambient cache or home: $private_cache" ;;
esac
case "$private_home/" in
    "$ambient/cache/"*|"$ambient/home/"*)
        fail "the helper's HOME sits inside the ambient cache or home: $private_home" ;;
esac

# A cache home others can read is a cache home shared with them.
[[ -O "$private_cache" ]] \
    || fail "the helper's XDG_CACHE_HOME is not owned by the user running the harness: $private_cache"
[[ "$(ls -ld "$private_cache" | cut -c1-10)" == 'drwx------' ]] \
    || fail "the helper's XDG_CACHE_HOME is readable by others: $private_cache"

# Sourcing the helper must not itself write into the ambient locations.
for ambient_directory in "$ambient/home" "$ambient/cache"; do
    [[ -z "$(find "$ambient_directory" -mindepth 1 -print -quit)" ]] \
        || fail "sourcing the helper wrote into $ambient_directory, which belongs to whoever runs the harness"
done

# --- the real tree ----------------------------------------------------------
examine "$repository" || exit 1

# Discovered rather than listed, so a new harness is held without being added
# here. The two the repository qualifies releases with are still named, so a
# scan that quietly stops matching them fails here instead of passing.
for anchor in tests/production-release-qualification.sh tests/executable-delivery.sh; do
    found=no
    for runner in "${runners[@]}"; do
        [[ "$runner" != "$anchor" ]] || found=yes
    done
    [[ "$found" == yes ]] \
        || fail "$anchor no longer runs the built executable, so this scan is checking the wrong files"
done
(( ${#runners[@]} >= 2 )) \
    || fail "the scan found ${#runners[@]} harness(es) running the built executable, so it matched less than the repository holds"
