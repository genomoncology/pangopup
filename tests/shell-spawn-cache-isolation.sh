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
#   * That `scripts/smoke-linux-release.sh` runs the executable. It runs one
#     handed to it as an argument and names none, so no name scan can see it.
#     What covers it is inheritance: every shell file that names it is one of
#     the harnesses accepted above, so it has a cache home of its own and the
#     executable it hands over runs under that. This file checks that rather
#     than asserting it, because a comment saying "its only caller today is in
#     scope" stays true only until someone adds a second caller.
#     `.github/workflows/package-linux.yml` runs it inside a container against
#     that container's own filesystem, where there is no operator cache to
#     reach.
#
#     What that leaves unproved is order. `inheritors_hold` asks whether the
#     caller takes a cache home, not whether it takes one before the line that
#     hands the executable over. `tests/executable-delivery.sh` hands the smoke
#     script a stub of its own writing twice before its cache home and the real
#     executable once after, and separating those two means reading what each
#     call passes -- the argument-shape form this file exists to refuse.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
self_relative=${BASH_SOURCE[0]##*/}
self_relative=tests/$self_relative
helper_relative='tests/support/private-cache-home.sh'

# The one shell script that runs an executable it is handed rather than one it
# names.
smoke_relative='scripts/smoke-linux-release.sh'

# A run of the built executable. `pangopup-build` is a different binary with no
# model cache, so the trailing class keeps `target/debug/pangopup-build` out.
# `cargo run --bin pangopup` builds and runs the same executable by another
# name and is a run like any other; cargo spells that option with a space or an
# equals sign, so the separator class carries both.
#
# Only code counts, and a whole-line comment is the only commentary these
# patterns recognise. Anchoring them at `^[^#]*` instead would hide a run from
# every code line carrying an earlier `#`, and `"${bin#$PWD/}"` is ordinary
# shell. That is the one direction a gate like this must never fail in.
# `code_lines` drops the comment lines instead.
use='target/(debug|release)/pangopup([^-]|$)|--bin[[:space:]=]+pangopup([^-]|$)'

# Sourcing the helper is what establishes the cache home, so the line has to
# source it. A file that names the path in a variable or an error message has
# established nothing.
establish='^[[:space:]]*(\.|source)[[:space:]]+.*tests/support/private-cache-home\.sh'

# A build step: `cargo` as the command the line runs, or the one wrapper around
# `cargo build` the harnesses call. Anchored at the start of the line, past an
# assignment and a command substitution, because the word `cargo` inside a
# `grep` argument is a string being searched for rather than a build --
# `tests/executable-delivery.sh` carries eight such lines.
build='^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=)?(\$\()?(cargo[[:space:]]|[^[:space:]]*scripts/require-built-commands\.sh)'

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

# The line numbers in $1 matching the extended pattern $2, lowest first. A
# whole-line comment is commentary rather than code and never matches.
code_lines() {
    { grep -nE -- "$2" "$1" || true; } \
        | { grep -vE '^[0-9]+:[[:space:]]*#' || true; } \
        | cut -d: -f1
}

# The first such line, or empty.
first_line() {
    code_lines "$1" "$2" | head -n 1
}

# Refuse the tree rooted at $1. Prints its counts on acceptance, its reason on
# refusal. `runners` is left holding the relative paths that ran the
# executable, and `held_runners` the ones that ran it under a cache home of
# their own -- the first is what the scan matched, the second what it accepted.
runners=()
held_runners=()
examine() {
    local root=$1
    local sources=() source relative use_line establish_line build_line scanned
    local refused=0 warmed run_builds onnx held

    runners=()
    held_runners=()
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
            printf 'source %s before that line, with a leading `.` or `source`\n' "$helper_relative" >&2
            refused=1
            continue
        fi
        if (( establish_line > use_line )); then
            printf 'this harness runs the built executable at %s:%s but takes its cache home at line %s, so the runs before that line reach the model cache of whoever runs it\n' \
                "$relative" "$use_line" "$establish_line" >&2
            refused=1
            continue
        fi

        # Every build line, not just the first. A harness that builds, takes a
        # cache home and builds again -- debug first and release after -- put
        # its second build under the fresh home, and reading only the first
        # build line calls that clean.
        #
        # The one build allowed after the cache home is the run itself.
        # `cargo run --bin pangopup` is a build and a run on one line and the
        # run has to come after the cache home, so that line is refused on the
        # other condition instead: it is safe only where an earlier build
        # already left `target/` warm, and a harness whose run is its only
        # build has no such build.
        warmed=no
        run_builds=no
        onnx=no
        while IFS= read -r build_line; do
            [[ -n "$build_line" ]] || continue
            if (( build_line <= establish_line )); then
                warmed=yes
                continue
            fi
            if (( build_line == use_line )); then
                run_builds=yes
                continue
            fi
            printf 'this harness takes its cache home at %s:%s and then builds at line %s: the ONNX Runtime library the build resolves lands under XDG_CACHE_HOME, so a build under a fresh cache home downloads it again instead of linking the copy already there. Build first, take the cache home after.\n' \
                "$relative" "$establish_line" "$build_line" >&2
            refused=1
            onnx=yes
        done < <(code_lines "$source" "$build")
        held=yes
        [[ "$onnx" != yes ]] || held=no
        if [[ "$run_builds" == yes && "$warmed" == no ]]; then
            held=no
            printf 'this harness builds and runs the executable in one step at %s:%s under the cache home it took at line %s, and nothing builds before that line: the ONNX Runtime library the build resolves lands under XDG_CACHE_HOME, so it is downloaded again instead of linked. Build before taking the cache home.\n' \
                "$relative" "$use_line" "$establish_line" >&2
            refused=1
        fi
        [[ "$held" != yes ]] || held_runners+=("$relative")
    done

    (( refused == 0 )) || return 1

    if (( ${#runners[@]} == 0 )); then
        printf 'examined %s shell source(s) under %s and found none that runs the built executable, so this check read no run and proved nothing\n' \
            "$scanned" "$root" >&2
        return 1
    fi

    printf 'examined %s shell source(s), %s harness(es) running the built executable, each holding a cache home of its own\n' \
        "$scanned" "${#runners[@]}"
}

# Refuse the tree rooted at $1 if a shell file there names the script that runs
# an executable handed to it, without being one of the harnesses `examine`
# accepted. Call it after `examine`, which is what fills `held_runners`.
smoke_callers=()
inheritors_hold() {
    local root=$1 caller relative runner held refused=0
    smoke_callers=()
    while IFS= read -r caller; do
        relative=${caller#"$root/"}
        [[ "$relative" != "$smoke_relative" ]] || continue
        [[ "$relative" != "$self_relative" ]] || continue
        smoke_callers+=("$relative")
        held=no
        for runner in "${held_runners[@]}"; do
            [[ "$runner" != "$relative" ]] || held=yes
        done
        [[ "$held" == yes ]] && continue
        printf 'this file names %s, which runs an executable handed to it as an argument, but takes no cache home of its own to hand it: %s\n' \
            "$smoke_relative" "$relative" >&2
        refused=1
    done < <(
        find "$root" -type f -name '*.sh' -not -path '*/target/*' -print0 \
            | xargs -0 -r grep -lF -- "$smoke_relative" \
            | sort
    )
    (( refused == 0 )) || return 1

    # A scan that read no caller has read nothing, and the same refusal covers
    # the smoke script being renamed out from under the name this file greps
    # for: the rule would then hold over an empty set and stay green.
    if (( ${#smoke_callers[@]} == 0 )); then
        printf 'examined %s and found no shell file naming %s, so the inheritance rule held over nothing\n' \
            "$root" "$smoke_relative" >&2
        return 1
    fi
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
                printf 'cargo run --package %s --bin %s -- lookup --help\n' pangopup-cli pangopup
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
            builds-again)
                printf 'cargo build --locked\n'
                printf '. "$repo/%s"\n' "$helper_relative"
                printf 'cargo build --release --locked\n'
                printf '"$repo/target/%s/pangopup" --version\n' release
                ;;
            names-helper)
                printf 'helper=%s\n' "$helper_relative"
                printf '"$repo/target/%s/pangopup" --version\n' debug
                ;;
            bin-equals)
                printf 'cargo run --package %s --bin=%s -- lookup --help\n' pangopup-cli pangopup
                ;;
            after-a-hash)
                printf 'trimmed=${bin#$PWD/}; "$repo/target/%s/pangopup" --version\n' debug
                ;;
            held-cargo-run)
                printf 'scripts/require-built-commands.sh\n'
                printf '. "$repo/%s"\n' "$helper_relative"
                printf 'cargo run --package %s --bin %s -- lookup --help\n' pangopup-cli pangopup
                ;;
            names-smoke)
                printf '"$repo/%s" "$1" "$2"\n' "$smoke_relative"
                ;;
            held-names-smoke)
                printf 'scripts/require-built-commands.sh\n'
                printf '. "$repo/%s"\n' "$helper_relative"
                printf '"$repo/target/%s/pangopup" --version\n' debug
                printf '"$repo/%s" "$1" "$2"\n' "$smoke_relative"
                ;;
            cold-cargo-run)
                printf '. "$repo/%s"\n' "$helper_relative"
                printf 'cargo run --package %s --bin %s -- lookup --help\n' pangopup-cli pangopup
                ;;
            package-no-bin)
                printf 'cargo run --package %s -- lookup --help\n' pangopup-cli
                ;;
            package-short)
                printf 'cargo run -p %s -- lookup --help\n' pangopup-cli
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

# Reading only the first build line calls a harness clean when it builds, takes
# a cache home and builds again -- a debug build and then a release one, which
# is the ordinary shape of a release harness. The second build is the one under
# the fresh cache home.
builds_again="$fixtures/builds-again"
plant "$builds_again" 'tests/qualification.sh' builds-again
expect_refusal "$builds_again" 'ONNX Runtime'

# Naming the helper is not sourcing it. A file that assigns the path to a
# variable and never sources it has taken no cache home.
named="$fixtures/named"
plant "$named" 'tests/qualification.sh' names-helper
expect_refusal "$named" 'tests/qualification.sh:3'

# `--bin=pangopup` is the same run as `--bin pangopup`.
bin_equals="$fixtures/bin-equals"
plant "$bin_equals" 'tests/help-contract.sh' bin-equals
expect_refusal "$bin_equals" 'tests/help-contract.sh:2'

# `pangopup-cli` declares one `[[bin]]`, so `--bin` is optional: `cargo run
# --package pangopup-cli` reaches the same executable without naming it, and a
# scan that reads only `--bin pangopup` calls such a harness quiet. Cargo
# spells the package option long and short.
package_no_bin="$fixtures/package-no-bin"
plant "$package_no_bin" 'tests/help-contract.sh' package-no-bin
expect_refusal "$package_no_bin" 'tests/help-contract.sh:2'

package_short="$fixtures/package-short"
plant "$package_short" 'tests/help-contract.sh' package-short
expect_refusal "$package_short" 'tests/help-contract.sh:2'

# A run on a code line carrying an earlier `#` is still a run. `${bin#$PWD/}`
# is ordinary shell, and a scan that reads it as a comment reads the harness as
# quiet.
hashed="$fixtures/hashed"
plant "$hashed" 'tests/qualification.sh' after-a-hash
expect_refusal "$hashed" 'tests/qualification.sh:2'

# `cargo run` builds and runs in one step, so it cannot be moved before the
# cache home. It is safe only where an earlier build left `target/` warm, and
# refused where nothing built first.
cold="$fixtures/cold"
plant "$cold" 'tests/help-contract.sh' cold-cargo-run
expect_refusal "$cold" 'nothing builds before that line'

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

# The shape `tests/release-help-contract.sh` has to take: build, take the cache
# home, then build and run in one step. A rule with no satisfying shape for a
# `cargo run` harness would be a rule that cannot be obeyed.
warm="$fixtures/warm"
plant "$warm" 'tests/help-contract.sh' held-cargo-run
expect_acceptance "$warm" \
    'the scanner refused a harness that builds, takes a cache home and only then runs cargo, which leaves a cargo-run harness no shape it can take'

# A file that hands an executable to the script that runs whatever it is given
# has to have taken a cache home first, or the run reaches the operator's cache
# under a name no scan can read.
inherited="$fixtures/inherited"
plant "$inherited" 'tests/qualification.sh' held
plant "$inherited" 'scripts/caller.sh' names-smoke
examine "$inherited" >/dev/null \
    || fail 'the inheritance fixture was refused by the name scan, so it proves nothing about inheritance'
if inheritors_hold "$inherited" 2>/dev/null; then
    fail 'the scanner accepted a file handing an executable to the smoke script without a cache home of its own'
fi

# A tree where nothing names the smoke script. The rule has nothing to hold and
# says so, rather than reporting the vacuous pass as a verdict.
uncalled="$fixtures/uncalled"
plant "$uncalled" 'tests/qualification.sh' held
examine "$uncalled" >/dev/null || fail 'the uncalled fixture was refused by the name scan'
if inheritors_hold "$uncalled" 2>/dev/null; then
    fail 'the scanner accepted a tree in which no file names the smoke script, so the inheritance rule can pass on nothing'
fi

inheriting="$fixtures/inheriting"
plant "$inheriting" 'tests/qualification.sh' held-names-smoke
examine "$inheriting" >/dev/null || fail 'the inheriting fixture was refused by the name scan'
inheritors_hold "$inheriting" \
    || fail 'the scanner refused a harness that takes a cache home and only then hands an executable to the smoke script'

# --- the helper redirects ---------------------------------------------------
#
# The scan above reads the calling side: it proves every harness takes a cache
# home. Should the helper stop redirecting, every one of those harnesses would
# reach the operator's cache and the scan would still be green, because they
# would still be taking a cache home. This reads the other side.

helper_redirects() (
helper="$repository/$helper_relative"
[[ -f "$helper" ]] \
    || fail "no $helper_relative: nothing gives a shell harness a cache home of its own"

ambient="$fixtures/ambient"
mkdir -p "$ambient/home" "$ambient/cache"
chmod 700 "$ambient/home" "$ambient/cache"

# The default model cache one environment resolves, spelled the way the product
# spells it: `XDG_CACHE_HOME` when set, otherwise `HOME/.cache`.
# `CARGO_HOME` and `RUSTUP_HOME` are read back from the same source, with both
# unset going in, because that is the case the helper has to pin. A toolchain
# resolved under the private home is fetched from the network on every run,
# since the helper removes that directory each time it is sourced.
resolved=$(
    env -u PANGOPUP_MODEL_CACHE -u CARGO_HOME -u RUSTUP_HOME \
        HOME="$ambient/home" XDG_CACHE_HOME="$ambient/cache" \
        bash -c '
            set -euo pipefail
            . "$1"
            printf "%s\n%s\n%s\n%s\n" \
                "${XDG_CACHE_HOME-}" "${HOME-}" "${CARGO_HOME-}" "${RUSTUP_HOME-}"
        ' bash "$helper"
) || fail 'sourcing the helper failed'

private_cache=$(printf '%s\n' "$resolved" | sed -n '1p')
private_home=$(printf '%s\n' "$resolved" | sed -n '2p')
toolchain_homes=$(printf '%s\n' "$resolved" | sed -n '3,4p')

# The three variables that name a cache location outright. `PANGOPUP_MODEL_CACHE`
# names the model cache file itself and is read *ahead* of `XDG_CACHE_HOME` and
# `HOME`, so moving those two leaves a run reaching the file this variable
# names; `PANGOPUP_CACHE_DIR` and `PANGOPUP_DATA_DIR` name the download cache
# and the installed bundle directory the same way. An operator who exports one
# runs the whole suite against what it names, and a cache whose recorded setup
# no longer matches is discarded rather than ignored.
#
# Read back from a shell that exported all three at a location the helper must
# not leave resolvable. Removing them is the requirement: an empty value is a
# value the product still reads, and one pointed somewhere else is still a
# location this file cannot vouch for.
inherited=$(
    env PANGOPUP_MODEL_CACHE="$ambient/cache/inherited.sqlite3" \
        PANGOPUP_CACHE_DIR="$ambient/cache/inherited-downloads" \
        PANGOPUP_DATA_DIR="$ambient/home/inherited-bundles" \
        bash -c '
            set -euo pipefail
            . "$1"
            for name in PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR; do
                if [[ -n "${!name+set}" ]]; then
                    printf "%s=%s\n" "$name" "${!name}"
                fi
            done
        ' bash "$helper"
) || fail 'sourcing the helper with the cache variables exported failed'

if [[ -n "$inherited" ]]; then
    printf 'the helper leaves these exported after a harness sources it, so every run under it reaches the cache location whoever runs the harness named and can destroy that file:\n' >&2
    printf '  %s\n' $inherited >&2
    fail 'the helper must unset PANGOPUP_MODEL_CACHE, PANGOPUP_CACHE_DIR and PANGOPUP_DATA_DIR'
fi

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

# Cargo and rustup both keep their downloads under `$HOME`, so the move has to
# pin them to where they resolved before it. Left to follow `HOME`, a `cargo
# run` after the helper re-fetches the registry index and installs the whole
# toolchain named by `rust-toolchain.toml` under the private home, which the
# next run deletes.
[[ $(printf '%s\n' "$toolchain_homes" | grep -c .) -eq 2 ]] \
    || fail 'the helper left CARGO_HOME or RUSTUP_HOME unset, so cargo and rustup follow the moved HOME and re-download the registry and the toolchain on every run'
while IFS= read -r toolchain_home; do
    case "$toolchain_home/" in
        "$private_home/"*|"$private_cache/"*)
            fail "the helper points cargo or rustup inside the cache home it just emptied, so every run re-downloads: $toolchain_home" ;;
    esac
    case "$toolchain_home" in
        "$ambient/home/"*) ;;
        *) fail "the helper must pin cargo and rustup to where they resolved before the move, and $toolchain_home is not under $ambient/home" ;;
    esac
done < <(printf '%s\n' "$toolchain_homes")

# Sourcing the helper must not itself write into the ambient locations.
for ambient_directory in "$ambient/home" "$ambient/cache"; do
    [[ -z "$(find "$ambient_directory" -mindepth 1 -print -quit)" ]] \
        || fail "sourcing the helper wrote into $ambient_directory, which belongs to whoever runs the harness"
done
)

# --- the real tree ----------------------------------------------------------
#
# The two halves answer different questions and are reported independently. Run
# in sequence, whichever failed first would hide the other, and an operator
# handed one of two verdicts has to fix it to find out what the second one is.
# The helper half runs in a subshell so that its own refusals stop it without
# stopping this file.

problems=0

examine "$repository" || problems=1

# Discovered rather than listed, so a new harness is held without being added
# here. The three the repository has today are still named, so a scan that
# quietly stops matching one of them fails here instead of passing. Naming them
# is also what makes the count honest: a floor of two under this loop could
# never fail, because the loop has already put two entries in `runners`.
for anchor in \
    tests/production-release-qualification.sh \
    tests/executable-delivery.sh \
    tests/release-help-contract.sh; do
    found=no
    for runner in "${runners[@]}"; do
        [[ "$runner" != "$anchor" ]] || found=yes
    done
    if [[ "$found" != yes ]]; then
        printf 'shell spawn cache isolation: %s no longer runs the built executable, so this scan is checking the wrong files\n' \
            "$anchor" >&2
        problems=1
    fi
done

if inheritors_hold "$repository"; then
    # Named for the same reason the runners above are: a scan that quietly
    # stops matching the one caller the repository has fails here instead of
    # reporting an empty set as a clean one.
    found=no
    for caller in "${smoke_callers[@]}"; do
        [[ "$caller" != tests/executable-delivery.sh ]] || found=yes
    done
    if [[ "$found" != yes ]]; then
        printf 'shell spawn cache isolation: tests/executable-delivery.sh no longer hands an executable to %s, so this scan is checking the wrong files\n' \
            "$smoke_relative" >&2
        problems=1
    fi
else
    problems=1
fi

helper_redirects || problems=1

(( problems == 0 )) || exit 1
