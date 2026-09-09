---
---
# The test suite fills the operator's own model cache

`make test` writes rows into the model cache of whoever runs it. The cache is on by default and its path comes from `XDG_CACHE_HOME`, which `make test` does not set, so the suite reaches `~/.cache/pangopup/model-results.sqlite3` on a developer machine and on a CI runner alike.

Measured in this checkout on 2026-09-09.

- `crates/pangopup-cli/tests/model_routing.rs:44` builds a modelled run with no `--model-cache`. `crates/pangopup-cli/src/main.rs:1719` then resolves the default cache, and `:1776` resolves its path from the ambient `XDG_CACHE_HOME` or `HOME`.
- Running `cargo test --locked --workspace` with `XDG_CACHE_HOME` pointed at an empty scratch directory left `pangopup/model-results.sqlite3` there holding two rows. Without that redirection those two rows land in the operator's own cache.
- `make spec` already redirects: `Makefile` sets `XDG_CACHE_HOME="$(CURDIR)/target/spec-cache"` for `mustmatch`. `make test` has no equivalent.

The rows the suite writes come from miniature fixture assets. They can never be read back by a production run, because a production run reaches other assets, so the cost today is a growing file and an eviction the operator did not ask for. Once a cache file is discarded when the setup that filled it changes, the cost is larger: a suite run would discard whatever the operator's real cache held.

Done, observably:

- A full `make test` writes nothing under the operator's `XDG_CACHE_HOME` or `HOME`.
- A test that means to exercise the default cache says where the default is, rather than inheriting it.
- `make lint`, `make test` and `make spec` pass.
