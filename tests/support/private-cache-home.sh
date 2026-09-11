# A model cache home of the sourcing harness's own. Sourced, never run.
#
# The model cache is on by default and its directory comes from
# `XDG_CACHE_HOME`, or from `HOME` when that is unset. A shell harness that runs
# the built `pangopup` executable without redirecting both reaches the cache
# file of whoever is running the suite: it fills that file with rows scored from
# miniature fixtures, and since ticket 0058 a cache whose recorded setup no
# longer matches is discarded rather than ignored, so such a run destroys that
# file instead of only growing it.
#
# This is the one place a shell harness takes a cache home, so the two dozen
# ways to get the redirect subtly wrong live in one reviewed file rather than
# in every harness. `tests/shell-spawn-cache-isolation.sh` holds both sides:
# that every harness naming the executable sources this, and that this
# redirects.
#
# Source it AFTER the build, never before. The ONNX Runtime library the build
# resolves lands under `XDG_CACHE_HOME`, so a build under a fresh cache home
# fetches it again instead of linking the copy already there.
#
# The cache home lives under `target/` rather than in a temporary directory.
# A temporary directory has to be removed on exit, and the only way to arrange
# that from a sourced file is to install an `EXIT` trap in the caller, which
# would silently replace the trap the caller already has. `cargo clean` and
# `make clean` collect this one instead, and it is outside every cache the
# product resolves.

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    printf 'tests/support/private-cache-home.sh is sourced, not run: it changes the cache environment of the caller\n' >&2
    exit 1
fi

# Cargo keeps its registry and its downloaded crates under `$HOME/.cargo`, and
# rustup keeps the toolchains under `$HOME/.rustup`, so moving `HOME` without
# pinning these would make a `cargo run` after the redirect fetch the whole
# index again and, where `cargo` is a rustup shim, install the toolchain named
# by `rust-toolchain.toml` from the network: measured at 601 MB, paid on every
# run, because this file removes the private home each time it is sourced.
# Neither is a model cache and the operator's copies are the right ones to keep
# using. Each is pinned to what it resolved to before the move, so a machine
# that has neither directory is left exactly as it was.
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"

__private_cache_home=$(
    cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
)/target/shell-harness-cache-home

rm -rf -- "$__private_cache_home"
mkdir -p -- "$__private_cache_home"
# Whatever the caller's umask says. A cache home others can read is a cache
# home shared with them.
chmod 700 -- "$__private_cache_home"

HOME=$__private_cache_home
XDG_CACHE_HOME=$__private_cache_home
export HOME XDG_CACHE_HOME
unset __private_cache_home

# Moving the two homes is not enough. `PANGOPUP_MODEL_CACHE` names the model
# cache file outright, `PANGOPUP_CACHE_DIR` the download cache,
# `PANGOPUP_DATA_DIR` the installed bundle directory and
# `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` how many rows the model cache keeps, and
# each is read ahead of both homes, so a harness inherits whatever the operator
# exported and runs against it. A value of the last one the product cannot
# parse refuses every modelled lookup, so an operator who exported it could not
# run the suite at all. They are unset rather than set empty: an empty value is
# still a value the product reads. What a caller sets after sourcing this still
# reaches its runs, which is how the product's own variables stay testable.
unset PANGOPUP_MODEL_CACHE PANGOPUP_CACHE_DIR PANGOPUP_DATA_DIR \
    PANGOPUP_MODEL_CACHE_MAX_ENTRIES
