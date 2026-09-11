---
---
# The shell cache home helper moves rustup with `HOME`

`tests/support/private-cache-home.sh` moves `HOME` so that a run of the built
executable cannot resolve the operator's model cache. It pins `CARGO_HOME`
before the move, because a cargo whose home moved fetches the whole registry
index again. `RUSTUP_HOME` has the same shape and is not pinned.

Measured in this checkout on 2026-09-10 while implementing ticket 0071. With
`RUSTUP_HOME` unset, `tests/release-help-contract.sh` runs `cargo run` after
sourcing the helper, the rustup shim reads `$HOME/.rustup`, finds nothing, and
installs the toolchain named by `rust-toolchain.toml` from the network: 601 MB
into `target/shell-harness-cache-home/.rustup`, on every run. The helper
removes that directory each time it is sourced, so the download is paid again
on the next run. With `RUSTUP_HOME` exported to the operator's own directory
the same harness finishes with nothing written under the private home.

An operator who has `RUSTUP_HOME` set does not see this. One who does not set
it, which is what a default rustup installation leaves, pays a toolchain
download for every `make test`.

Done, observably:

- `tests/release-help-contract.sh` run with `RUSTUP_HOME` unset writes no
  rustup toolchain under the private cache home and reaches the network for
  none.

Boundary: no product behaviour, cache location, or default changes, and the
helper keeps setting `HOME` and `XDG_CACHE_HOME` as it does today. Pinning a
variable is not the same as leaving `HOME` in place.
