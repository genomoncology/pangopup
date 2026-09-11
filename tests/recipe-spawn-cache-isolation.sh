#!/usr/bin/env bash
set -euo pipefail

# Two gates hold a run of the built executable to a cache home of its own.
# `tests/cli-spawn-cache-isolation.sh` reads `*.rs` and
# `tests/shell-spawn-cache-isolation.sh` reads `*.sh`. Three routes to that
# executable are neither: the `spec` recipe in the `Makefile`, the spec blocks
# it runs, and the maintainer measurement under `maintainers/` that drives the
# release executable from Python.
#
# What keeps `make spec` off the operator's cache today is one assignment in
# that recipe. It is correct as far as it goes, and no check reads it: `HOME`
# is not moved beside it, so a spec block that cleared `XDG_CACHE_HOME` would
# fall back to the home directory of whoever ran `make spec`, and the four
# variables that decide what the model cache is and what it holds are
# inherited whole.
#
# This file reads those three routes. It is the same rule the siblings hold,
# applied to the files they cannot see, and it counts what it read so a pattern
# that stops matching fails here rather than passing.

repository=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# The crate whose one `[[bin]]` is the shipped executable. `pangopup-build` is
# a different binary with no model cache. Spelled as a variable rather than
# inline so that this file's own patterns are not read as runs by the sibling
# scan over `*.sh`.
package='pangopup-cli'

# A run of the built executable. `pangopup-cli` declares one `[[bin]]`, so
# `--bin` is optional and `cargo run --package` reaches it without naming it.
run="target/(debug|release)/pangopup([^-]|\$)|--bin[[:space:]=]+pangopup([^-]|\$)|cargo run[^#]*(--package|-p)[[:space:]=]+$package([^-]|\$)"

# A recipe reaches the executable the same way by putting the build directory
# on `PATH` and calling it by bare name, which is what the `spec` recipe does.
recipe_run="$run|PATH[^#]*target/(debug|release)"

# The four variables a run of the built executable must not inherit from
# whoever started the recipe. Three of them name a cache location outright and
# are read ahead of `XDG_CACHE_HOME` and `HOME`. The fourth names no location:
# `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` says how many rows the cache the recipe
# gave the run may keep, so an operator who exported it decides what a gate
# under a private cache home measures, and a value the product cannot parse
# stops the run outright. Both are the same defect -- the operator's
# environment reaching into a run that is supposed to stand on its own -- so
# both are held here.
#
# `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` contains `PANGOPUP_MODEL_CACHE`, so the
# match below asks for a character other than `_` after each name. That is what
# separates these two, and it is not a word boundary: a name extending one of
# them by a letter or a digit still answers for the shorter one, measured in
# sdlc/tickets/drafts/0107.
named_locations=(PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR PANGOPUP_MODEL_CACHE_MAX_ENTRIES)

fail() { printf 'recipe spawn cache isolation: %s\n' "$*" >&2; exit 1; }

# The code lines of $1 matching $2. A whole-line comment is commentary, in a
# Makefile and in a Markdown fence alike.
code_lines() {
    { grep -nE -- "$2" "$1" || true; } \
        | { grep -vE '^[0-9]+:[[:space:]]*#' || true; }
}

# The same, for Python, with every `#` comment cut away first. A Makefile line
# carries `$$` and a shell line carries `${bin#$PWD/}`, so neither can have its
# tail removed; Python spells no expansion that way, and a trailing comment
# naming a variable has handed the child nothing.
python_code_lines() {
    sed 's/#.*$//' "$1" | { grep -nE -- "$2" || true; }
}

# The path $1 names a location the recipe or the build owns, rather than a
# location belonging to whoever ran it. Absolute is not the test: so is
# `/home/someone/.cache`, and that is the file this whole check exists to keep
# a run away from.
owned_location() {
    case "$1" in
        /tmp/?*) return 0 ;;
        */target/?*) return 0 ;;
        *) return 1 ;;
    esac
}

# --- the three rules --------------------------------------------------------

# A recipe line that reaches the executable gives the run a cache home under
# the repository's own build directory, both variables, and drops every one of
# the four inherited variables above.
makefile_holds() {
    local makefile=$1 relative=$2 line number text refused=0 matched=0
    [[ -f "$makefile" ]] || { printf 'no %s to read\n' "$relative" >&2; return 1; }
    while IFS= read -r line; do
        [[ -n "$line" ]] || continue
        number=${line%%:*}
        text=${line#*:}
        matched=$((matched + 1))
        # `$(CURDIR)/target/` rather than any path spelling `target/`: the
        # refusal below says "into the build directory", and a rule that
        # accepts `/home/someone/target/cache` says something else.
        if ! grep -qE '(^|[[:space:]])XDG_CACHE_HOME="?\$\(CURDIR\)/target/' <<<"$text"; then
            printf 'this recipe reaches the built executable without pointing XDG_CACHE_HOME into the build directory, so the run reads the cache directory of whoever runs it: %s:%s\n' \
                "$relative" "$number" >&2
            refused=1
        fi
        if ! grep -qE '(^|[[:space:]])HOME="?\$\(CURDIR\)/target/' <<<"$text"; then
            printf 'this recipe reaches the built executable without moving HOME beside XDG_CACHE_HOME, so a block that clears XDG_CACHE_HOME falls back to the home directory of whoever runs it: %s:%s\n' \
                "$relative" "$number" >&2
            refused=1
        fi
        # Cargo keeps its registry under `$HOME/.cargo` and rustup the
        # toolchains under `$HOME/.rustup`, and spec blocks run `cargo test`.
        # A recipe that moves `HOME` without pinning these sends a rustup shim
        # to the network for the whole toolchain, into a directory the recipe
        # removes before every run.
        for toolchain in CARGO_HOME RUSTUP_HOME; do
            grep -qE -- "(^|[[:space:]])$toolchain=" <<<"$text" && continue
            printf 'this recipe moves HOME without pinning %s, so a cargo run under it resolves that directory inside the build directory and installs the toolchain from the network: %s:%s\n' \
                "$toolchain" "$relative" "$number" >&2
            refused=1
        done
        for name in "${named_locations[@]}"; do
            grep -qE -- "-u[[:space:]=]+$name([^_]|\$)|(^|[[:space:]])$name=\"?\\\$\\(CURDIR\\)/target/" <<<"$text" && continue
            printf 'this recipe reaches the built executable with %s inherited, so the operator who exported it decides where that run keeps its model cache or how much of it the run may keep: %s:%s\n' \
                "$name" "$relative" "$number" >&2
            refused=1
        done
    done < <(code_lines "$makefile" "$recipe_run")

    if (( matched == 0 )); then
        printf 'read %s and found no recipe reaching the built executable, so this rule held over nothing\n' \
            "$relative" >&2
        return 1
    fi
    (( refused == 0 )) || return 1
    printf '%s\n' "$matched"
}

# A spec block runs under the cache home its recipe took. Pointing either
# variable somewhere relative, or clearing it, gives that run back to whoever
# ran the recipe.
specs_hold() {
    local root=$1 files=() file relative line number text refused=0 scanned examined=0
    while IFS= read -r file; do files+=("$file"); done < <(
        find "$root/spec" -type f -name '*.md' 2>/dev/null | sort
    )
    scanned=${#files[@]}
    if (( scanned == 0 )); then
        printf 'found no spec file under %s/spec, so this rule read nothing\n' "$root" >&2
        return 1
    fi
    for file in "${files[@]}"; do
        relative=${file#"$root/"}
        while IFS= read -r line; do
            [[ -n "$line" ]] || continue
            number=${line%%:*}
            text=${line#*:}
            examined=$((examined + 1))
            block_holds_a_home "$text" && continue
            printf 'this spec block points HOME or XDG_CACHE_HOME somewhere the recipe does not own, so the run falls back to the cache of whoever ran it: %s:%s\n' \
                "$relative" "$number" >&2
            refused=1
        done < <(
            # `unset` needs a word boundary before the name. Without one,
            # `unset XDG_DATA_HOME` reads as an `unset ... HOME` and is refused
            # for touching a variable it never names.
            code_lines "$file" \
                '(^|[[:space:]])(XDG_CACHE_HOME|HOME)=|unset[^#]*[[:space:]](XDG_CACHE_HOME|HOME)([^A-Z_]|$)'
        )
    done
    (( refused == 0 )) || return 1
    printf '%s %s\n' "$scanned" "$examined"
}

# The block on line $1 keeps a cache home the recipe owns. Every assignment of
# either variable on that line has to name a location the block owns, and no
# `unset` of either may appear: an absolute path is not enough on its own,
# because `/home/someone/.cache` is absolute.
block_holds_a_home() {
    local text=$1 value owned=0
    ! grep -qE 'unset[^#]*[[:space:]](XDG_CACHE_HOME|HOME)([^A-Z_]|$)' <<<"$text" || return 1
    while IFS= read -r value; do
        owned_location "$value" || return 1
        owned=1
    done < <(
        grep -oE '(^|[[:space:]])(XDG_CACHE_HOME|HOME)=[^[:space:]]*' <<<"$text" \
            | sed -E 's/^[[:space:]]*[A-Z_]+=//; s/^"//; s/"$//'
    )
    (( owned == 1 ))
}

# A Python file that reaches the executable hands the child a cache home, the
# same rule the shell and Rust harnesses hold.
python_holds() {
    local root=$1 files=() file relative refused=0 scanned reached=0
    while IFS= read -r file; do files+=("$file"); done < <(
        find "$root" -type f -name '*.py' -not -path '*/target/*' -not -path '*/__pycache__/*' | sort
    )
    scanned=${#files[@]}
    if (( scanned == 0 )); then
        printf 'found no Python source under %s, so this rule read nothing\n' "$root" >&2
        return 1
    fi
    for file in "${files[@]}"; do
        relative=${file#"$root/"}
        [[ -n "$(python_code_lines "$file" "$run")" ]] || continue
        reached=$((reached + 1))
        for name in XDG_CACHE_HOME HOME "${named_locations[@]}"; do
            # On a code line. A comment naming the variable has handed the
            # child nothing, and a rule satisfied by commentary is a rule that
            # passes on nothing.
            [[ -z "$(python_code_lines "$file" "(^|[^A-Z_])$name([^A-Z_]|\$)")" ]] || continue
            printf 'this file runs the built executable and never names %s, so the run reaches the cache of whoever runs it and can discard that file: %s\n' \
                "$name" "$relative" >&2
            refused=1
        done
    done
    if (( reached == 0 )); then
        printf 'examined %s Python source(s) under %s and found none reaching the built executable, so this rule held over nothing\n' \
            "$scanned" "$root" >&2
        return 1
    fi
    (( refused == 0 )) || return 1
    printf '%s\n' "$reached"
}

# --- the rules refuse what they exist to refuse -----------------------------
#
# A check that reports nothing wrong is worth exactly as much as its ability to
# report something wrong, so each refusal is exercised against a fixture before
# the real tree is read.

fixtures=$(mktemp -d)
trap 'rm -rf "$fixtures"' EXIT

# Every fixture spells the build directory and the package through arguments,
# so that neither this file nor the fixtures it writes are read as runs by the
# sibling scan over `*.sh`.
plant_makefile() {
    local tree=$1 shape=$2
    mkdir -p "$tree"
    {
        printf 'spec:\n'
        case "$shape" in
            bare)
                printf '\tPATH="$(CURDIR)/target/%s:$$PATH" mustmatch test spec/\n' debug ;;
            cache-only)
                printf '\tXDG_CACHE_HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/%s:$$PATH" mustmatch test spec/\n' debug ;;
            homes-only)
                printf '\tXDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/%s:$$PATH" mustmatch test spec/\n' debug ;;
            unpinned)
                printf '\tenv -u %s -u %s -u %s -u %s XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/%s:$$PATH" mustmatch test spec/\n' \
                    PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR PANGOPUP_MODEL_CACHE_MAX_ENTRIES debug ;;
            held)
                printf '\tenv -u %s -u %s -u %s -u %s CARGO_HOME="$${CARGO_HOME:-$$HOME/.cargo}" RUSTUP_HOME="$${RUSTUP_HOME:-$$HOME/.rustup}" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/%s:$$PATH" mustmatch test spec/\n' \
                    PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR PANGOPUP_MODEL_CACHE_MAX_ENTRIES debug ;;
            longer-only)
                printf '\tenv -u %s CARGO_HOME="$${CARGO_HOME:-$$HOME/.cargo}" RUSTUP_HOME="$${RUSTUP_HOME:-$$HOME/.rustup}" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/%s:$$PATH" mustmatch test spec/\n' \
                    PANGOPUP_MODEL_CACHE_MAX_ENTRIES debug ;;
            limit-inherited)
                printf '\tenv -u %s -u %s -u %s CARGO_HOME="$${CARGO_HOME:-$$HOME/.cargo}" RUSTUP_HOME="$${RUSTUP_HOME:-$$HOME/.rustup}" XDG_CACHE_HOME="$(CURDIR)/target/spec-cache" HOME="$(CURDIR)/target/spec-cache" PATH="$(CURDIR)/target/%s:$$PATH" mustmatch test spec/\n' \
                    PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR debug ;;
            quiet)
                printf '\tcargo build --locked --package %s\n' "$package" ;;
            package-run)
                printf '\tcargo run --locked --package %s -- lookup --help\n' "$package" ;;
            elsewhere)
                printf '\tenv -u %s -u %s -u %s -u %s CARGO_HOME="$${CARGO_HOME:-$$HOME/.cargo}" RUSTUP_HOME="$${RUSTUP_HOME:-$$HOME/.rustup}" XDG_CACHE_HOME=/home/someone/target/c HOME=/home/someone/target/h PATH="$(CURDIR)/target/%s:$$PATH" mustmatch test spec/\n' \
                    PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR PANGOPUP_MODEL_CACHE_MAX_ENTRIES debug ;;
        esac
    } >"$tree/Makefile"
}

plant_spec() {
    local tree=$1 name=$2 shape=$3
    mkdir -p "$tree/spec"
    case "$shape" in
        quiet) printf 'pangopup lookup --help\n' ;;
        cleared) printf 'XDG_CACHE_HOME= pangopup lookup --help\n' ;;
        unset) printf 'unset XDG_CACHE_HOME\n' ;;
        relative) printf 'HOME=relative pangopup lookup --help\n' ;;
        absolute) printf 'XDG_CACHE_HOME=/tmp/pangopup-unused pangopup sync --offline\n' ;;
        reads) printf 'cache="$XDG_CACHE_HOME/pangopup/model-results.sqlite3"\n' ;;
        other) printf 'XDG_DATA_HOME=/tmp/pangopup-unused pangopup status\n' ;;
        unset-other) printf 'unset XDG_DATA_HOME\n' ;;
        quoted) printf 'XDG_CACHE_HOME="/tmp/pangopup-unused" pangopup sync --offline\n' ;;
        operator-home) printf 'HOME=/home/someone pangopup lookup --help\n' ;;
    esac >"$tree/spec/$name"
}

plant_python() {
    local tree=$1 name=$2 shape=$3
    mkdir -p "$tree/$(dirname "$name")"
    {
        case "$shape" in
            quiet)
                printf 'def go():\n    return 0\n' ;;
            runs)
                printf 'def go():\n    subprocess.run([repo / "target/%s/pangopup", "--version"])\n' release ;;
            commented)
                printf 'def go():\n'
                printf '    # %s %s %s %s %s %s\n' \
                    XDG_CACHE_HOME HOME PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR \
                    PANGOPUP_MODEL_CACHE_MAX_ENTRIES
                printf '    subprocess.run([repo / "target/%s/pangopup", "--version"])\n' release ;;
            held)
                printf 'def go():\n'
                printf '    environment = dict(os.environ)\n'
                printf '    for name in ("%s", "%s", "%s", "%s"):\n' \
                    PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR PANGOPUP_MODEL_CACHE_MAX_ENTRIES
                printf '        environment.pop(name, None)\n'
                printf '    environment["XDG_CACHE_HOME"] = str(home)\n'
                printf '    environment["HOME"] = str(home)\n'
                printf '    subprocess.run([repo / "target/%s/pangopup", "--version"], env=environment)\n' release ;;
        esac
    } >"$tree/$name"
}

expect_refusal() {
    local wanted=$1; shift
    local output status
    set +e
    output=$("$@" 2>&1)
    status=$?
    set -e
    (( status != 0 )) || fail "the scanner accepted what it must refuse: $wanted"
    case "$output" in
        *"$wanted"*) ;;
        *) fail "the refusal does not name $wanted, so an operator cannot act on it: $output" ;;
    esac
}

expect_acceptance() {
    local why=$1; shift
    "$@" >/dev/null || fail "$why"
}

# A recipe with no run in it has nothing to hold, and saying so is the
# difference between a rule that passed and a rule that read nothing.
plant_makefile "$fixtures/quiet" quiet
expect_refusal 'held over nothing' makefile_holds "$fixtures/quiet/Makefile" Makefile

plant_makefile "$fixtures/bare" bare
expect_refusal 'Makefile:2' makefile_holds "$fixtures/bare/Makefile" Makefile

# The shape the repository has today: `XDG_CACHE_HOME` moved, `HOME` left
# behind. A block that clears the first falls back to the second.
plant_makefile "$fixtures/cache-only" cache-only
expect_refusal 'without moving HOME' makefile_holds "$fixtures/cache-only/Makefile" Makefile

# Both homes moved is still not a cache home of its own while the variables
# that name a cache location outright are inherited.
plant_makefile "$fixtures/homes-only" homes-only
expect_refusal 'PANGOPUP_MODEL_CACHE inherited' makefile_holds "$fixtures/homes-only/Makefile" Makefile

# `cargo run --package` with no `--bin` reaches the same executable, because
# the crate declares one binary.
plant_makefile "$fixtures/package-run" package-run
expect_refusal 'Makefile:2' makefile_holds "$fixtures/package-run/Makefile" Makefile

# `target/` in a path of someone else's is not the build directory, and the
# refusal above says the build directory. `/home/someone/target` is absolute,
# spells `target/`, and is not a location this repository owns.
plant_makefile "$fixtures/elsewhere" elsewhere
expect_refusal 'into the build directory' makefile_holds "$fixtures/elsewhere/Makefile" Makefile

# Moving `HOME` moves `$HOME/.cargo` and `$HOME/.rustup` with it, into a
# directory the recipe removes before every run. A recipe that does not pin
# them sends a rustup shim to the network for the whole toolchain.
plant_makefile "$fixtures/unpinned" unpinned
expect_refusal 'without pinning CARGO_HOME' makefile_holds "$fixtures/unpinned/Makefile" Makefile

# The mirror of the shape above, and the reason the entry limit needs a
# fixture. `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` contains `PANGOPUP_MODEL_CACHE`,
# so a recipe dropping only the longer name would satisfy a plain search for
# the shorter one. This one drops the longer and nothing else, and has to be
# refused for the shorter.
plant_makefile "$fixtures/longer-only" longer-only
expect_refusal 'with PANGOPUP_MODEL_CACHE inherited' makefile_holds "$fixtures/longer-only/Makefile" Makefile

# And the other direction: dropping the three older names leaves the run
# whatever limit the operator exported.
plant_makefile "$fixtures/limit-inherited" limit-inherited
expect_refusal 'with PANGOPUP_MODEL_CACHE_MAX_ENTRIES inherited' makefile_holds "$fixtures/limit-inherited/Makefile" Makefile

plant_makefile "$fixtures/held" held
expect_acceptance 'the scanner refused a recipe that moves both homes, pins both toolchain homes and drops every named cache location, so it refuses the shape it exists to require' \
    makefile_holds "$fixtures/held/Makefile" Makefile

plant_spec "$fixtures/spec-empty" ignored.md quiet
rm -rf "$fixtures/spec-empty/spec"
expect_refusal 'read nothing' specs_hold "$fixtures/spec-empty"

for shape in cleared unset relative; do
    plant_spec "$fixtures/spec-$shape" cli.md "$shape"
    expect_refusal "spec/cli.md:1" specs_hold "$fixtures/spec-$shape"
done

# An absolute path is not on its own a location the block owns:
# `/home/someone/.cache` is absolute too, and it is exactly the file this whole
# check exists to keep a run away from.
plant_spec "$fixtures/spec-operator-home" cli.md operator-home
expect_refusal 'spec/cli.md:1' specs_hold "$fixtures/spec-operator-home"

# A path under the recipe's build directory or under the temporary directory is
# somewhere the block owns, quoted or not; a read of the variable is not an
# assignment; and `XDG_DATA_HOME` is a different variable, whether it is
# assigned or unset. None of these is the failure this rule exists to catch,
# and an exemption wider than its target would refuse all of them.
for shape in quiet absolute quoted reads other unset-other; do
    plant_spec "$fixtures/spec-ok-$shape" cli.md "$shape"
    expect_acceptance "the scanner refused a spec block of shape $shape, which reaches no cache belonging to whoever ran the recipe" \
        specs_hold "$fixtures/spec-ok-$shape"
done

plant_python "$fixtures/py-quiet" 'maintainers/measure.py' quiet
expect_refusal 'held over nothing' python_holds "$fixtures/py-quiet"

plant_python "$fixtures/py-runs" 'maintainers/measure.py' runs
expect_refusal 'maintainers/measure.py' python_holds "$fixtures/py-runs"

# A comment naming every variable hands the child none of them, so a rule a
# comment can satisfy is a rule that passes on nothing.
plant_python "$fixtures/py-commented" 'maintainers/measure.py' commented
expect_refusal 'maintainers/measure.py' python_holds "$fixtures/py-commented"

plant_python "$fixtures/py-held" 'maintainers/measure.py' held
expect_acceptance 'the scanner refused a Python file that hands the child both homes and drops every named cache location, so it refuses the shape it exists to require' \
    python_holds "$fixtures/py-held"

# --- the real tree ----------------------------------------------------------

problems=0

recipes=$(makefile_holds "$repository/Makefile" Makefile) || problems=1
spec_counts=$(specs_hold "$repository") || problems=1
pythons=$(python_holds "$repository") || problems=1

(( problems == 0 )) || exit 1

# The spec rule's denominator is the files it read, because a spec tree that
# points neither home anywhere is a tree with nothing to refuse. The second
# number is the blocks that actually reached the rule, so a candidate pattern
# that quietly stops matching shows up here as a zero rather than as a pass.
printf 'examined %s recipe(s) reaching the built executable, %s spec file(s) carrying %s block(s) that name a cache home, and %s Python file(s) that reach it, each holding a cache home of its own\n' \
    "$recipes" ${spec_counts} "$pythons"
