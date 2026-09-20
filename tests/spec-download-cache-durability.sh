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
# The recipe is not the only thing that empties a directory. `cargo clean`
# removes all of `target/`, it is the command an operator runs before a
# rebuild, and a rebuild is what refills this cache -- so a library kept under
# the build directory is fetched again over the network for exactly the reason
# this rule exists. The build directory is therefore read as a directory the
# recipe removes, whether or not the recipe names it.
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

# --- reading a recipe -------------------------------------------------------

# The target names in makefile $1, in the order they are written.
targets() {
    { grep -oE '^[a-zA-Z0-9_.@%/-]+:' "$1" || true; } \
        | sed 's/:$//' \
        | grep -vx '.PHONY' \
        || true
}

# The command lines of target $2 in makefile $1. Keep them unexpanded: each
# supported assignment and operand is admitted before bounded Make-variable
# expansion, and no recipe text is evaluated by a shell.
recipe_lines() {
    local makefile=$1 target=$2
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
    ' "$makefile"
}

unquote_word() {
    local word=$1
    case "$word" in
        \"*\") word=${word#\"}; word=${word%\"} ;;
        \'*\') word=${word#\'}; word=${word%\'} ;;
        *\"*|*\'*)
            printf 'unsupported quoted path syntax: %s\n' "$1" >&2
            return 1
            ;;
    esac
    case "$word" in
        *\"*|*\'*)
            printf 'unsupported joined quoting: %s\n' "$1" >&2
            return 1
            ;;
    esac
    printf '%s\n' "$word"
}

# Refuse shell constructs before token splitting or expansion. `$(CURDIR)` is
# the one admitted Make expansion. Everything else that can execute or
# reinterpret recipe text is outside this bounded checker.
line_syntax_is_static() {
    local line=$1 without_curdir
    without_curdir=${line//\$\(CURDIR\)/}
    case "$without_curdir" in
        *'$('*) printf 'unsafe shell syntax: command substitution\n' >&2; return 1 ;;
        *'`'*) printf 'unsafe shell syntax: backticks\n' >&2; return 1 ;;
        *'<('*|*'>('*) printf 'unsafe shell syntax: process substitution\n' >&2; return 1 ;;
    esac
    if [[ "$line" =~ (^|[[:space:]])eval([[:space:]]|$) ]]; then
        printf 'unsafe shell syntax: eval\n' >&2
        return 1
    fi
}

# Normalize a path without reading the filesystem. A leading double slash has
# implementation-defined meaning on POSIX, and a parent cannot climb above the
# absolute root.
normalize_absolute() {
    local path=$1 part count normalized='' old_ifs=$IFS
    local parts=() stack=()
    case "$path" in
        //*) printf 'unsafe absolute path syntax: %s\n' "$path" >&2; return 1 ;;
        /*) ;;
        *) printf 'path is not absolute: %s\n' "$path" >&2; return 1 ;;
    esac
    IFS=/ read -r -a parts <<<"$path"
    IFS=$old_ifs
    for part in "${parts[@]}"; do
        case "$part" in
            ''|.) ;;
            ..)
                count=${#stack[@]}
                if (( count == 0 )); then
                    printf 'cannot normalize absolute path above root: %s\n' "$path" >&2
                    return 1
                fi
                unset 'stack[count-1]'
                ;;
            *) stack[${#stack[@]}]=$part ;;
        esac
    done
    for part in "${stack[@]}"; do normalized="$normalized/$part"; done
    [[ -n "$normalized" ]] || normalized=/
    printf '%s\n' "$normalized"
}

validate_path_token() {
    local value=$1 label=$2
    case "$value" in
        *'*'*|*'?'*|*'['*) printf 'wildcard %s: %s\n' "$label" "$value" >&2; return 1 ;;
        *\\*) printf 'unsupported shell escape in %s: %s\n' "$label" "$value" >&2; return 1 ;;
        *'`'*) printf 'unsafe shell syntax in %s: backticks\n' "$label" >&2; return 1 ;;
        *';'*|*'|'*|*'&'*|*'<'*|*'>'*) printf 'unsafe shell syntax in %s: %s\n' "$label" "$value" >&2; return 1 ;;
    esac
}

# Expand only values whose base this check knows. The shell never sees them.
expand_path() {
    local raw=$1 root=$2 caller_home=$3 label=$4 value
    case "$raw" in
        \'*\')
            case "$raw" in
                *'$$HOME'*|*'$$PWD'*)
                    printf 'unsupported single-quoted shell variable in %s: %s\n' "$label" "$raw" >&2
                    return 1
                    ;;
            esac
            ;;
    esac
    value=$(unquote_word "$raw") || return 1
    line_syntax_is_static "$value" || return 1
    validate_path_token "$value" "$label" || return 1
    case "$value" in
        *'$$HOME'[A-Za-z0-9_]*|*'$$PWD'[A-Za-z0-9_]*)
            printf 'unknown variable in %s: %s\n' "$label" "$raw" >&2
            return 1
            ;;
    esac
    value=${value//\$\(CURDIR\)/$root}
    value=${value//\$\$PWD/$root}
    value=${value//\$\$HOME/$caller_home}
    case "$value" in
        *'$'*) printf 'unknown variable in %s: %s\n' "$label" "$raw" >&2; return 1 ;;
    esac
    printf '%s\n' "$value"
}

validate_assignment_value() {
    local raw=$1 name=$2 value
    value=$(unquote_word "$raw") || return 1
    line_syntax_is_static "$value" || return 1
    case "$value" in
        *';'*|*'|'*|*'&'*|*'<'*|*'>'*)
            printf 'unsafe shell syntax in %s assignment\n' "$name" >&2
            return 1
            ;;
    esac
}

# The directories the recipe on standard input removes, one per line, resolved
# against root $1. Only `rm` with a recursive flag removes a directory, and
# every operand after the flags is one.
removed_directories() {
    local root=$1 line word operand normalized seen_recursive
    local words=()
    while IFS= read -r line; do
        case "$line" in
            rm\ *|rm) ;;
            *) continue ;;
        esac
        line_syntax_is_static "$line" || return 1
        read -r -a words <<<"$line"
        seen_recursive=no
        for word in "${words[@]}"; do
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
            operand=$(unquote_word "$word") || return 1
            case "$operand" in
                *'*'*|*'?'*|*'['*)
                    printf 'wildcard removal operand: %s\n' "$operand" >&2
                    return 1
                    ;;
            esac
            validate_path_token "$operand" 'removal operand' || return 1
            operand=$(expand_path "$operand" "$root" "$root/.caller-home" 'removal operand') || return 1
            case "$operand" in /*) ;; *) operand="$root/$operand" ;; esac
            normalized=$(normalize_absolute "$operand") || return 1
            printf '%s\n' "$normalized"
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

# Resolve one cache-deciding line from its admitted static environment prefix.
# Return 2 for the supported relative ORT_CACHE_DIR exception.
resolve_cache_path() {
    local line=$1 root=$2 caller_cache="$root/.caller-cache" caller_home="$root/.caller-home"
    local word name value operand expect_unset=no
    local ort_set=no ort='' xdg_set=yes xdg="$caller_cache" home_set=yes home="$caller_home"
    local expanded
    local words=()
    read -r -a words <<<"$line"
    for word in "${words[@]}"; do
        if [[ "$expect_unset" == yes ]]; then
            validate_assignment_value "$word" 'env -u operand' || return 1
            operand=$(unquote_word "$word") || return 1
            expect_unset=no
            case "$operand" in
                ORT_CACHE_DIR) ort_set=no; ort='' ;;
                XDG_CACHE_HOME) xdg_set=no; xdg='' ;;
                HOME) home_set=no; home='' ;;
            esac
            continue
        fi
        case "$word" in
            env) continue ;;
            eval) printf 'unsafe shell syntax: eval\n' >&2; return 1 ;;
            -u) expect_unset=yes; continue ;;
            --unset=*)
                operand=${word#--unset=}
                validate_assignment_value "$operand" 'env --unset operand' || return 1
                case "$operand" in
                    ORT_CACHE_DIR) ort_set=no; ort='' ;;
                    XDG_CACHE_HOME) xdg_set=no; xdg='' ;;
                    HOME) home_set=no; home='' ;;
                esac
                continue
                ;;
            [A-Za-z_]*=*)
                name=${word%%=*}
                value=${word#*=}
                validate_assignment_value "$value" "$name" || return 1
                case "$name" in
                    ORT_CACHE_DIR) ort_set=yes; ort=$value ;;
                    XDG_CACHE_HOME) xdg_set=yes; xdg=$value ;;
                    HOME) home_set=yes; home=$value ;;
                esac
                continue
                ;;
            *) break ;;
        esac
    done
    [[ "$expect_unset" == no ]] || {
        printf 'unsafe shell syntax: missing env -u operand\n' >&2
        return 1
    }

    if [[ "$ort_set" == yes ]]; then
        expanded=$(expand_path "$ort" "$root" "$caller_home" ORT_CACHE_DIR) || return 1
        if [[ -n "$expanded" ]]; then
            case "$expanded" in
                /*) normalize_absolute "$expanded" ;;
                *) return 2 ;;
            esac
            return
        fi
    fi

    if [[ "$home_set" == yes ]]; then
        home=$(expand_path "$home" "$root" "$caller_home" HOME) || return 1
        case "$home" in
            /*) home=$(normalize_absolute "$home") || return 1 ;;
            *) printf 'HOME must resolve to an absolute path: %s\n' "$home" >&2; return 1 ;;
        esac
    else
        home=''
    fi
    if [[ "$(uname -s)" == Darwin ]]; then
        normalize_absolute "$home/Library/Caches/ort.pyke.io"
        return
    fi
    if [[ "$xdg_set" == yes ]]; then
        xdg=$(expand_path "$xdg" "$root" "$caller_home" XDG_CACHE_HOME) || return 1
        case "$xdg" in
            /*) normalize_absolute "$xdg"; return ;;
        esac
    fi
    normalize_absolute "$home/.cache/ort.pyke.io"
}

# Whether path $1 stands inside directory $2, or is that directory.
inside() {
    [[ "$1" == "$2" || "$1" == "$2"/* ]]
}

# Refuse makefile $1, read for a tree rooted at $2. Prints the number of
# recipes it held on acceptance, the reason on refusal.
makefile_holds() {
    local makefile=$1 root=$2 relative=$3
    local target recipe removed line resolved directory resolve_status
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
        recipe=$(recipe_lines "$makefile" "$target")
        [[ -n "$recipe" ]] || continue
        if ! removed=$(printf '%s\n' "$recipe" | removed_directories "$root"); then
            refused=1
            continue
        fi
        [[ -n "$removed" ]] || continue
        held=$((held + 1))

        agreed=
        agreed_line=
        while IFS= read -r line; do
            [[ -n "$line" ]] || continue
            if resolved=$(resolve_cache_path "$line" "$root"); then
                :
            else
                resolve_status=$?
                if (( resolve_status == 2 )); then
                    continue
                fi
                printf 'could not resolve the downloaded-library cache of the %s recipe in %s\n' "$target" "$relative" >&2
                refused=1
                continue
            fi

            while IFS= read -r directory; do
                [[ -n "$directory" ]] || continue
                inside "$resolved" "$directory" || continue
                printf 'the %s recipe in %s removes %s and then points the downloaded ONNX Runtime library cache at %s, inside it: every run that rebuilds ort-sys fetches that library again over the network\n' \
                    "$target" "$relative" "$directory" "$resolved" >&2
                refused=1
            done <<<"$removed"

            # The build directory, which no recipe here has to name. `cargo
            # clean` collects all of it, and the run after that clean rebuilds
            # `ort-sys` and pays the download again. `$root/target` rather
            # than any path spelling `target/`: a `target/` belonging to
            # another tree survives a clean run here, and a sibling named
            # `target-library` is not the build directory at all.
            if inside "$resolved" "$root/target"; then
                printf 'the %s recipe in %s points the downloaded ONNX Runtime library cache at %s, inside the build directory: cargo clean collects all of %s, and the next run that rebuilds ort-sys fetches that library again over the network\n' \
                    "$target" "$relative" "$resolved" "$root/target" >&2
                refused=1
            fi

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
            cache-dot)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/./target/spec-cache/ort" cargo build --locked\n' ;;
            cache-parent)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/../.ort-cache" cargo build --locked\n' ;;
            closed-make-expansion)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)cache/ort" cargo build --locked\n' ;;
            escaped-cache-path)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/crates/\\../target/ort-cache" cargo build --locked\n' ;;
            escaped-removal-path)
                printf '\trm -rf target/\\../.ort-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/.ort-cache/ort" cargo build --locked\n' ;;
            joined-cache-quoting)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/tar""get/ort-cache" cargo build --locked\n' ;;
            joined-removal-quoting)
                printf '\trm -rf "target/.""./.ort-cache"\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/.ort-cache/ort" cargo build --locked\n' ;;
            single-quoted-home-path)
                printf '\trm -rf target/ort-cache\n'
                printf "\tORT_CACHE_DIR='\$(CURDIR)/\$\$HOME/../target/ort-cache' cargo build --locked\n" ;;
            single-quoted-relative-path)
                printf '\trm -rf target/spec-cache\n'
                printf "\tORT_CACHE_DIR='\$\$HOME/.ort-cache' cargo build --locked\n" ;;
            removal-dot)
                printf '\trm -rf ./target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/spec-cache/ort" cargo build --locked\n' ;;
            removal-parent)
                printf '\trm -rf target/../removed-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/removed-cache/ort" cargo build --locked\n' ;;
            equal-paths)
                printf '\trm -rf target/spec-cache/ort\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/spec-cache/ort/" cargo build --locked\n' ;;
            home-fallback)
                printf '\trm -rf target/spec-cache\n'
                printf '\tXDG_CACHE_HOME= HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            durable)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/.ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            relative-ort)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR=target/spec-cache/ort XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            pwd-ort)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$$PWD/target/spec-cache/ort" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            removes-elsewhere)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/.ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/ort-cache-not" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            env-home-ort)
                printf '\trm -rf .caller-home\n'
                printf '\tenv ORT_CACHE_DIR="$$HOME/.cache/ort.pyke.io" cargo build --locked\n' ;;
            build-directory)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            sibling-directory)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target-library/ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            not-recursive)
                printf '\trm -f target/spec-cache/stamp\n'
                printf '\tXDG_CACHE_HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            split)
                printf '\tcargo build --locked\n'
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/.ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            joined)
                printf '\tenv ORT_CACHE_DIR="$(CURDIR)/.ort-cache" cargo build --locked\n'
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/.ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            equivalent-lines)
                printf '\tenv ORT_CACHE_DIR="$(CURDIR)/.ort-cache/" cargo build --locked\n'
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/target/../.ort-cache" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" mustmatch test spec/\n' ;;
            command-substitution)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$$(>$(CURDIR)/sentinel)" cargo build --locked\n' ;;
            backticks)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="`>$(CURDIR)/sentinel`" cargo build --locked\n' ;;
            process-substitution)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="<(>$(CURDIR)/sentinel)" cargo build --locked\n' ;;
            shell-evaluation)
                printf '\trm -rf target/spec-cache\n'
                printf '\teval ORT_CACHE_DIR="$(CURDIR)/.ort-cache" cargo build --locked\n' ;;
            wildcard-removal)
                printf '\trm -rf target/*\n'
                printf '\tORT_CACHE_DIR="$(CURDIR)/.ort-cache" cargo build --locked\n' ;;
            unknown-variable)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$${UNKNOWN_CACHE}/ort" cargo build --locked\n' ;;
            longer-home-variable)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$$HOME_CACHE/ort" cargo build --locked\n' ;;
            longer-pwd-variable)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="$$PWD_OTHER/ort" cargo build --locked\n' ;;
            unsafe-other-assignment)
                printf '\trm -rf target/spec-cache\n'
                printf '\tOTHER="$$(>$(CURDIR)/sentinel)" ORT_CACHE_DIR="$(CURDIR)/.ort-cache" cargo build --locked\n' ;;
            unsafe-unset-operand)
                printf '\trm -rf target/spec-cache\n'
                printf '\tenv -u "$$(>$(CURDIR)/sentinel)" ORT_CACHE_DIR="$(CURDIR)/.ort-cache" cargo build --locked\n' ;;
            nonnormal-absolute)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="/../../ort-cache" cargo build --locked\n' ;;
            unsafe-absolute)
                printf '\trm -rf target/spec-cache\n'
                printf '\tORT_CACHE_DIR="//host/ort-cache" cargo build --locked\n' ;;
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

expect_safe_refusal() {
    local wanted=$1 tree=$2 output status
    set +e
    output=$(makefile_holds "$tree/Makefile" "$tree" Makefile 2>&1)
    status=$?
    set -e
    (( status != 0 )) || fail "the rule accepted unsafe syntax it must refuse: $wanted"
    [[ ! -e "$tree/sentinel" ]] \
        || fail "the rule executed unsafe fixture syntax before refusing it: $wanted"
    case "$output" in
        *"$wanted"*) ;;
        *) fail "the unsafe-syntax refusal does not name $wanted: $output" ;;
    esac
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

# Lexically equivalent dot and parent segments are classified by the location
# they name, on both the cache and removal operands.
for shape in cache-dot removal-dot removal-parent equal-paths; do
    plant "$work/$shape" "$shape"
    expect_refusal 'fetches that library again' "$work/$shape"
done

plant "$work/cache-parent" cache-parent
expect_acceptance 'the rule refused a cache whose parent segment resolves outside the removed build directory' \
    "$work/cache-parent"

plant "$work/closed-make-expansion" closed-make-expansion
expect_acceptance 'the rule rejected a closed CURDIR Make expansion followed by a literal path segment' \
    "$work/closed-make-expansion"

for shape in escaped-cache-path escaped-removal-path; do
    plant "$work/$shape" "$shape"
    expect_safe_refusal 'unsupported shell escape' "$work/$shape"
done

for shape in joined-cache-quoting joined-removal-quoting; do
    plant "$work/$shape" "$shape"
    expect_safe_refusal 'unsupported joined quoting' "$work/$shape"
done

for shape in single-quoted-home-path single-quoted-relative-path; do
    plant "$work/$shape" "$shape"
    expect_safe_refusal 'single-quoted shell variable' "$work/$shape"
done

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

# A cache the recipe itself never removes, standing in the build directory.
# Nothing in this repository removes it either -- and `cargo clean` removes all
# of it, which is what makes the library a download the next build pays for.
plant "$work/build-directory" build-directory
expect_refusal 'cargo clean collects all of' "$work/build-directory"

# A sibling whose name begins with the build directory's is not inside it, and
# a rule that read the letters rather than the boundary would refuse this one.
plant "$work/sibling-directory" sibling-directory
expect_acceptance 'the rule refused a downloaded-library cache in a sibling directory whose name merely begins with the build directory name' \
    "$work/sibling-directory"

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

# Equivalent spellings, including a trailing separator, still name one cache
# across separate build lines.
plant "$work/equivalent-lines" equivalent-lines
expect_acceptance 'the rule read equivalent normalized cache paths on separate build lines as different locations' \
    "$work/equivalent-lines"

# Unsupported syntax is refused before any expansion or word iteration. The
# first three shapes would create `sentinel` if a shell evaluated them.
for shape in command-substitution backticks process-substitution shell-evaluation; do
    plant "$work/$shape" "$shape"
    expect_safe_refusal 'unsafe shell syntax' "$work/$shape"
done

plant "$work/wildcard-removal" wildcard-removal
expect_safe_refusal 'wildcard removal operand' "$work/wildcard-removal"

plant "$work/unknown-variable" unknown-variable
expect_safe_refusal 'unknown variable' "$work/unknown-variable"

for shape in longer-home-variable longer-pwd-variable; do
    plant "$work/$shape" "$shape"
    expect_safe_refusal 'unknown variable' "$work/$shape"
done

plant "$work/unsafe-other-assignment" unsafe-other-assignment
expect_safe_refusal 'unsafe shell syntax' "$work/unsafe-other-assignment"

plant "$work/unsafe-unset-operand" unsafe-unset-operand
expect_safe_refusal 'unsafe shell syntax' "$work/unsafe-unset-operand"

plant "$work/nonnormal-absolute" nonnormal-absolute
expect_safe_refusal 'cannot normalize absolute path' "$work/nonnormal-absolute"

plant "$work/unsafe-absolute" unsafe-absolute
expect_safe_refusal 'unsafe absolute path syntax' "$work/unsafe-absolute"

# --- the real tree ----------------------------------------------------------

held=$(makefile_holds "$repository/Makefile" "$work/real" Makefile) || exit 1

# Read against a tree of this file's own rather than against the checkout, so
# that nothing here resolves a path inside `target/`. The anchor below is what
# keeps the count honest.
recipe=$(recipe_lines "$repository/Makefile" "$anchor")
[[ -n "$recipe" ]] \
    || fail "the Makefile has no $anchor recipe, so this scan is reading the wrong file"
[[ -n "$(printf '%s\n' "$recipe" | removed_directories "$work/real")" ]] \
    || fail "the $anchor recipe removes no directory, so this scan is holding the wrong recipe"

printf 'examined %s Makefile recipe(s) that remove a directory, each keeping its downloaded ONNX Runtime library cache outside every directory it removes\n' \
    "$held"
