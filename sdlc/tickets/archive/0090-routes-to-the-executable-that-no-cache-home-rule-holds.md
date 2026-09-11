---
flow: build
priority: 2
---
# Routes to the executable that no cache-home rule holds

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

## Three routes are outside both spawn gates

Two gates hold a run of the built executable to a cache home of its own:
`tests/cli-spawn-cache-isolation.sh` reads `*.rs`, and
`tests/shell-spawn-cache-isolation.sh` reads `*.sh`. Measured on 2026-09-10,
three routes to that executable are outside both.

The `spec` recipe in the `Makefile` puts `target/debug` on `PATH` and runs
`mustmatch test` over `spec/`. Four blocks in `spec/cli.md` copy
`../target/debug/pangopup` and run it, and `spec/model-routing.md:146` opens
`$XDG_CACHE_HOME/pangopup/model-results.sqlite3` directly. What keeps that off
the operator's cache is one assignment in the recipe,
`XDG_CACHE_HOME="$(CURDIR)/target/spec-cache"`. It is correct today. No gate
reads it, `HOME` is not moved beside it, and a block that cleared
`XDG_CACHE_HOME` would fall back to the home directory of whoever ran
`make spec`.

`cargo run --package pangopup-cli` reaches the same executable without naming
it. `pangopup-cli` declares one `[[bin]]`, so the `--bin` argument the shell
scan looks for is optional, and a harness spelled that way is read as quiet.

`maintainers/ticket-053/measure.py:704` runs `target/release/pangopup` from
Python. It passes `--model-cache` explicitly at line 492, which is the accident
ticket 0071 exists to remove rather than a rule. It is maintainer-run and no
gate invokes it.

Done, observably:

- A spawn through either helper resolves no cache file named by an inherited
  `PANGOPUP_MODEL_CACHE`, `PANGOPUP_CACHE_DIR` or `PANGOPUP_DATA_DIR`.
- A check proves it, by running with one of them exported at a file it then
  finds untouched.
- A recipe or spec block that runs the built executable without a cache home of
  its own is refused by a check, not by inspection.
- `cargo run --package pangopup-cli` with no `--bin` is read as a run.
- `make test`, `make spec` and `make lint` pass.

Boundary: no product behaviour, cache location, default, or scoring assertion
changes. The variables keep working for an operator running the product.
