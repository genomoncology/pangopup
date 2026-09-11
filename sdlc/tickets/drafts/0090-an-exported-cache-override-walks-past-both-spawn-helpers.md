---
---
# An exported cache override walks past both spawn helpers

Both helpers that give a spawn a cache home of its own set `HOME` and
`XDG_CACHE_HOME` and stop there: `tests/support/private-cache-home.sh` for the
shell harnesses and `crates/pangopup-cli/tests/support/mod.rs` for the Rust
suite. `PANGOPUP_MODEL_CACHE` names the cache file outright and is read ahead
of both, so an operator who exports it runs the whole suite against the file it
names.

Measured in this checkout on 2026-09-10. With `HOME` and `XDG_CACHE_HOME`
pointed at a private directory and `PANGOPUP_MODEL_CACHE` pointed at a copy of
a real cache, one fixture lookup printed `discarded model cache ...: an earlier
layout wrote it` and replaced 303104 bytes with 20480. That is the same
destruction ticket 0058 introduced and ticket 0071 exists to keep away from the
operator's file, reached by a route neither helper closes.

`tests/production-release-qualification.sh:374` already clears
`PANGOPUP_DATA_DIR`, `PANGOPUP_CACHE_DIR`, `PANGOPUP_MODEL_CACHE` and
`PANGOPUP_MODEL_CACHE_MAX_ENTRIES` for one command it runs. Every other spawn
in the repository inherits whatever the operator exported. Clearing them per
invocation is the accident ticket 0069 and ticket 0071 both exist to remove;
the helpers are the one place each should be cleared.

Fixing only one helper would leave the guarantee uneven, so this covers both.

Done, observably:

- A spawn through either helper resolves no cache file named by an inherited
  `PANGOPUP_MODEL_CACHE`, `PANGOPUP_CACHE_DIR` or `PANGOPUP_DATA_DIR`.
- A check proves it, by running with one of them exported at a file it then
  finds untouched.

Boundary: no product behaviour, cache location, default, or scoring assertion
changes. The variables keep working for an operator running the product.
