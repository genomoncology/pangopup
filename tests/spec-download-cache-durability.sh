#!/usr/bin/env bash
set -euo pipefail

# A recipe that empties a directory before every run must not point a cache of
# downloaded artifacts at that directory. Whatever the cache holds is fetched
# again on the next run that needs it, over the network, every time.
#
# The `spec` recipe does exactly that today. It removes `target/spec-cache` and
# then points `XDG_CACHE_HOME` at it, and the `ort-sys` build script -- built
# with `download-binaries` -- keeps the ONNX Runtime static library under
# `$XDG_CACHE_HOME/dfbin/<target>/<digest>/`. Every `make spec` that rebuilds
# `ort-sys` therefore fetches 87 MB again.
#
# This file resolves that library cache the way `ort-sys` resolves it, out of
# the recipe's own environment rather than out of a description of it, and
# refuses the recipe when the answer stands inside a directory the same recipe
# removes. Nothing here downloads anything: the proof is where the cache
# resolves to, which is what decides whether the download happens at all.
#
# `ort-sys` 2.0.0-rc.12 resolves it in `src/internal/dirs.rs` as `ORT_CACHE_DIR`
# when that is set, otherwise the platform default: on Linux `XDG_CACHE_HOME`
# when it is an absolute path and `$HOME/.cache/ort.pyke.io` otherwise, and on
# macOS `$HOME/Library/Caches/ort.pyke.io`. The probe below is that rule and
# nothing more, so a recipe is read against what the build actually does rather
# than against which variable someone remembered to name.
#
# The probe runs from the tree root, because that is where `make` runs a
# recipe and what `$$PWD` in one resolves to. Reading it from wherever this
# file happens to have been started would answer `ORT_CACHE_DIR="$$PWD/..."`
# with a directory the recipe never names.
#
# The rule is deliberately not "XDG_CACHE_HOME must survive the recipe".
# `make spec` runs under a model cache home of its own and that home is
# supposed to be emptied each run; it is the download cache that must not be,
# and a rule wider than that would refuse the shape the repository wants.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# The recipe the repository has today, named rather than only discovered: a
# scan that quietly stops matching it would otherwise hold over an empty set
# and report that as a pass.
anchor=spec

fail() { printf 'spec download cache durability: %s\n' "$*" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The ONNX Runtime library cache one environment resolves. Run under the
# recipe's own environment prefix, so what it prints is what the build script
# would compute.
probe="$work/resolve-ort-cache.sh"
{
    printf '#!/usr/bin/env bash\n'
    printf 'set -euo pipefail\n'
    printf 'if [[ -n "${ORT_CACHE_DIR-}" ]]; then\n'
    printf '    printf %%s "$ORT_CACHE_DIR"\n'
    printf 'elif [[ "$(uname -s)" == Darwin ]]; then\n'
    printf '    printf %%s "${HOME-}/Library/Caches/ort.pyke.io"\n'
    printf 'elif [[ "${XDG_CACHE_HOME-}" == /* ]]; then\n'
    printf '    printf %%s "$XDG_CACHE_HOME"\n'
    printf 'else\n'
    printf '    printf %%s "${HOME-}/.cache/ort.pyke.io"\n'
    printf 'fi\n'
} >"$probe"

# --- reading a recipe -------------------------------------------------------

# The target names in makefile $1, in the order they are written.
targets() {
    { grep -oE '^[a-zA-Z0-9_.@%/-]+:' "$1" || true; } \
        | sed 's/:$//' \
        | grep -vx '.PHONY' \
        || true
}

# The command lines of target $2 in makefile $1, with make's own expansions
# applied for a tree rooted at $3: `$(CURDIR)` becomes that root and `$$`
# becomes the single `$` the shell receives. Whole-line comments are dropped.
recipe_lines() {
    local makefile=$1 target=$2 root=$3
    awk -v target="$target" '
        index($0, target ":") == 1 { inside = 1; next }
        inside && /^\t/ {
            line = substr($0, 2)
            sub(/^[[:space:]]+/, "", line)
            if (line == "" || line ~ /^#/) { next }
            print line
            next
        }
        inside && /^[[:space:]]*$/ { next }
        inside { exit }
    ' "$makefile" \
        | sed -e "s|\\\$(CURDIR)|$root|g" -e 's|\$\$|$|g'
}

# The directories the recipe on standard input removes, one per line, resolved
# against root $1. Only `rm` with a recursive flag removes a directory, and
# every operand after the flags is one.
removed_directories() {
    local root=$1 line word
    while IFS= read -r line; do
        case "$line" in
            rm\ *|rm) ;;
            *) continue ;;
        esac
        local seen_recursive=no
        for word in $line; do
            case "$word" in
                rm) continue ;;
                --) continue ;;
                -*)
                    case "$word" in
                        *r*|*R*) seen_recursive=yes ;;
                    esac
                    continue
                    ;;
            esac
            [[ "$seen_recursive" == yes ]] || continue
            word=${word%\"}
            word=${word#\"}
            case "$word" in
                /*) printf '%s\n' "$word" ;;
                *) printf '%s/%s\n' "$root" "$word" ;;
            esac
        done
    done
}

# The lines of the recipe on standard input that decide where a downloaded
# library lands. A recipe is not one command: `spec` builds with cargo on its
# first line and runs the spec suite, which builds with cargo again, on its
# last. Every line that runs cargo resolves this cache for itself, because it
# is the `ort-sys` build script that does the downloading; so does every line
# that names a cache location for a command running cargo behind another name,
# the way `mustmatch test spec/` runs `cargo test` through the spec gates.
cache_deciding_lines() {
    local line
    while IFS= read -r line; do
        case "$line" in
            *XDG_CACHE_HOME=*|*ORT_CACHE_DIR=*|cargo|cargo\ *|*\ cargo\ *|*\ cargo)
                printf '%s\n' "$line" ;;
        esac
    done
}

# The leading environment prefix of command line $1: the `env` word, its
# `-u NAME` options and the `NAME=value` assignments that stand before the
# command. The command itself is dropped, so nothing this file evaluates can
# run anything the recipe runs. A line with no prefix answers empty, which is
# how a bare `cargo build` is read as resolving this cache from whatever
# environment the operator started make in.
environment_prefix() {
    local line=$1 word prefix='' expect_name=no
    for word in $line; do
        if [[ "$expect_name" == yes ]]; then
            expect_name=no
            prefix="$prefix $word"
            continue
        fi
        case "$word" in
            env) ;;
            -u) expect_name=yes ;;
            --unset=*) ;;
            [A-Za-z_]*=*) ;;
            *) break ;;
        esac
        prefix="$prefix $word"
    done
    printf '%s\n' "${prefix# }"
}

# Whether path $1 stands inside directory $2, or is that directory.
inside() {
    [[ "$1" == "$2" || "$1" == "$2"/* ]]
}

# Refuse makefile $1, read for a tree rooted at $2. Prints the number of
# recipes it held on acceptance, the reason on refusal.
makefile_holds() {
    local makefile=$1 root=$2 relative=$3
    local target recipe removed line prefix raw resolved directory
    local agreed agreed_line
    local held=0 refused=0
    [[ -f "$makefile" ]] || { printf 'no %s to read\n' "$relative" >&2; return 1; }
    mkdir -p "$root"
    # A controlled stand-in for the environment make was started in, so that a
    # line resolving this cache from the operator's own environment answers the
    # same way on every machine instead of answering with the reviewer's cache.
    mkdir -p "$root/.caller-cache" "$root/.caller-home"

    while IFS= read -r target; do
        [[ -n "$target" ]] || continue
        recipe=$(recipe_lines "$makefile" "$target" "$root")
        [[ -n "$recipe" ]] || continue
        removed=$(printf '%s\n' "$recipe" | removed_directories "$root")
        [[ -n "$removed" ]] || continue
        held=$((held + 1))

        agreed=
        agreed_line=
        while IFS= read -r line; do
            [[ -n "$line" ]] || continue
            prefix=$(environment_prefix "$line")
            # Run from the tree root, because that is where make runs a recipe
            # and what a relative answer stands against.
            # The stand-ins are put into the environment of the shell that
            # reads the prefix, not written in front of it. A prefix spelled
            # `env NAME="$HOME/..."` -- which is how every recipe here spells
            # one -- is a command with arguments, and the shell expands those
            # arguments before any assignment standing in front of the command
            # takes effect. Measured on 2026-09-11: with the stand-ins written
            # in front, `env ORT_CACHE_DIR="$HOME/.cache/ort.pyke.io"` resolved
            # to the operator's own home directory, so this rule answered with
            # the reviewer's machine instead of with the recipe.
            raw=$(cd "$root" && XDG_CACHE_HOME="$root/.caller-cache" HOME="$root/.caller-home" \
                bash -c "$prefix bash \"\$0\"" "$probe") \
                || { printf 'could not resolve the downloaded-library cache of the %s recipe in %s\n' "$target" "$relative" >&2; refused=1; continue; }
            # A relative answer is not inside any directory named here.
            # `ort-sys` takes `ORT_CACHE_DIR` verbatim with no absolute-path
            # test, and cargo runs a build script from the dependency's own
            # package root under `CARGO_HOME` rather than from `$(CURDIR)`, so
            # a relative value lands beside the crate source and never in a
            # directory this recipe removes. Measured with a build script that
            # printed its own working directory.
            resolved=
            case "$raw" in
                /*) resolved=$raw ;;
            esac
            [[ -n "$resolved" ]] || continue

            while IFS= read -r directory; do
                [[ -n "$directory" ]] || continue
                inside "$resolved" "$directory" || continue
                printf 'the %s recipe in %s removes %s and then points the downloaded ONNX Runtime library cache at %s, inside it: every run that rebuilds ort-sys fetches that library again over the network\n' \
                    "$target" "$relative" "$directory" "$resolved" >&2
                refused=1
            done <<<"$removed"

            # One recipe, one library cache. A line that resolves somewhere
            # else builds against a cache the rest of the recipe does not fill,
            # so it downloads the library again whenever that other cache is
            # empty -- which is what a clean CI home and a transient
            # `XDG_CACHE_HOME` both are. Measured: with the cache named on the
            # spec suite line alone, a `spec` run that rebuilt `ort-sys` wrote
            # 90647244 bytes into the caller's cache from the recipe's own
            # first line.
            if [[ -z "$agreed" ]]; then
                agreed=$resolved
                agreed_line=$line
            elif [[ "$resolved" != "$agreed" ]]; then
                printf 'the %s recipe in %s resolves the downloaded ONNX Runtime library cache to %s for `%s` and to %s for `%s`: the line reaching the other cache fetches that library again over the network whenever that cache is empty\n' \
                    "$target" "$relative" "$agreed" "$agreed_line" "$resolved" "$line" >&2
                refused=1
            fi
        done < <(printf '%s\n' "$recipe" | cache_deciding_lines)
    done < <(targets "$makefile")

    if (( held == 0 )); then
        printf 'read %s and found no recipe that removes a directory, so this rule held over nothing\n' \
            "$relative" >&2
        return 1
    fi
    (( refused == 0 )) || return 1
    printf '%s\n' "$held"
}

# --- the rule refuses what it exists to refuse ------------------------------
#
# A check that reports nothing wrong is worth exactly as much as its ability to
# report something wrong, so each refusal is exercised against a fixture before
# the real tree is read.

plant() {
    local tree=$1 shape=$2
    mkdir -p "$tree"
    {
        printf 'build:\n'
        printf '\tcargo build --locked\n'
        printf '\n'
        printf 'spec:\n'
        case "$shape" in
            quiet)
                printf '\tXDG_CACHE_HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            deleted)
                printf '\trm -rf target/spec-cache\n'
                printf '\tenv -u %s XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' \
                    PANGOPUP_MODEL_CACHE ;;
            nested)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/spec-cache/ort" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            home-fallback)
                printf '\trm -rf target/spec-cache\n'
                printf '\tXDG_CACHE_HOME= HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            durable)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            relative-ort)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR=target/spec-cache/ort XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            pwd-ort)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$$PWD/target/spec-cache/ort" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            removes-elsewhere)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/ort-cache-not" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            env-home-ort)
                printf '\trm -rf .caller-home\n'
                printf '\tenv ORT_CACHE_DIR="$$HOME/.cache/ort.pyke.io" cargo build --locked\n' ;;
            not-recursive)
                printf '\trm -f target/spec-cache/stamp\n'
                printf '\tXDG_CACHE_HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            split)
                printf '\tcargo build --locked\n'
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            joined)
                printf '\tenv ORT_CACHE_DIR="$(CURDIR)/target/ort-cache" cargo build --locked\n'
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
        esac
    } >"$tree/Makefile"
}

expect_refusal() {
    local wanted=$1 tree=$2 output status
    set +e
    output=$(makefile_holds "$tree/Makefile" "$tree" Makefile 2>&1)
    status=$?
    set -e
    (( status != 0 )) || fail "the rule accepted a recipe it must refuse: $wanted"
    case "$output" in
        *"$wanted"*) ;;
        *) fail "the refusal does not name $wanted, so a maintainer cannot act on it: $output" ;;
    esac
}

expect_acceptance() {
    local why=$1 tree=$2
    makefile_holds "$tree/Makefile" "$tree" Makefile >/dev/null || fail "$why"
}

# A makefile whose recipes remove nothing has nothing to hold, and saying so is
# the difference between a rule that passed and a rule that read nothing.
plant "$work/quiet" quiet
expect_refusal 'held over nothing' "$work/quiet"

# The shape the repository has today.
plant "$work/deleted" deleted
expect_refusal 'fetches that library again' "$work/deleted"

# Naming a cache directory of its own is not enough when the directory named
# stands inside the one the recipe removes.
plant "$work/nested" nested
expect_refusal 'fetches that library again' "$work/nested"

# Clearing `XDG_CACHE_HOME` sends the resolution to `$HOME`, which the recipe
# moved into the directory it removes. A rule that read only the variable it
# expected would call this recipe clean.
plant "$work/home-fallback" home-fallback
expect_refusal 'fetches that library again' "$work/home-fallback"

# A `$$HOME`-relative cache named through an `env` prefix resolves against the
# home the recipe runs under, which this rule stands in for. `.caller-home` is
# that stand-in, and this recipe removes it, so the answer has to land inside it
# and be refused. A probe that let the operator's own home answer instead would
# resolve somewhere outside this tree and call the recipe clean -- differently
# on every machine, which is the one thing the stand-in exists to stop.
plant "$work/env-home-ort" env-home-ort
expect_refusal 'fetches that library again' "$work/env-home-ort"

# `rm -f` on one file removes no directory, so there is nothing for this rule
# to refuse and an exemption wider than that would refuse the whole recipe.
plant "$work/not-recursive" not-recursive
expect_refusal 'held over nothing' "$work/not-recursive"

plant "$work/durable" durable
expect_acceptance 'the rule refused a recipe whose downloaded-library cache stands outside every directory it removes, so it refuses the shape it exists to require' \
    "$work/durable"

# The model cache home may still be emptied every run. Only the download cache
# has to survive, and a rule that refused this recipe would be wider than the
# thing it covers.
plant "$work/removes-elsewhere" removes-elsewhere
expect_acceptance 'the rule refused a recipe that empties its model cache home each run while keeping its downloaded-library cache elsewhere' \
    "$work/removes-elsewhere"

# `$$PWD` names the directory make ran the recipe in, which is the tree root:
# the same directory `$(CURDIR)` names, by the other spelling. A probe read
# from wherever this file was started would answer with some other directory
# and call this recipe clean.
plant "$work/pwd-ort" pwd-ort
expect_refusal 'fetches that library again' "$work/pwd-ort"

# A relative `ORT_CACHE_DIR` is not this rule's business. `ort-sys` takes the
# value verbatim, and cargo runs a build script from the dependency's own
# package root under `CARGO_HOME`, so the cache lands beside the crate source
# and not in the directory this recipe removes. Refusing it here would be a
# rule wider than the thing it covers.
plant "$work/relative-ort" relative-ort
expect_acceptance 'the rule refused a recipe whose relative ORT_CACHE_DIR lands beside the crate source rather than in a directory the recipe removes' \
    "$work/relative-ort"

# A cache named on the line that runs the spec suite and not on the line that
# builds is half a cache. The build line resolves whatever the operator started
# make in, and on a clean CI home or a transient `XDG_CACHE_HOME` that is empty
# every run, so the download the other line avoids is paid on this one instead.
plant "$work/split" split
expect_refusal 'fetches that library again over the network whenever that cache is empty' "$work/split"

# Both lines naming the same durable cache is the shape this rule exists to
# require.
plant "$work/joined" joined
expect_acceptance 'the rule refused a recipe whose every cargo line resolves the same downloaded-library cache, outside every directory it removes' \
    "$work/joined"

# --- the real tree ----------------------------------------------------------

held=$(makefile_holds "$repository/Makefile" "$work/real" Makefile) || exit 1

# Read against a tree of this file's own rather than against the checkout, so
# that nothing here resolves a path inside `target/`. The anchor below is what
# keeps the count honest.
recipe=$(recipe_lines "$repository/Makefile" "$anchor" "$work/real")
[[ -n "$recipe" ]] \
    || fail "the Makefile has no $anchor recipe, so this scan is reading the wrong file"
[[ -n "$(printf '%s\n' "$recipe" | removed_directories "$work/real")" ]] \
    || fail "the $anchor recipe removes no directory, so this scan is holding the wrong recipe"

printf 'examined %s Makefile recipe(s) that remove a directory, each keeping its downloaded ONNX Runtime library cache outside every directory it removes\n' \
    "$held"
