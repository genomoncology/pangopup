---
---
# The spawn gates cannot see three routes to the executable

Two gates hold a run of the built executable to a cache home of its own:
`tests/cli-spawn-cache-isolation.sh` reads `*.rs`, and
`tests/shell-spawn-cache-isolation.sh` reads `*.sh`. Measured in this checkout
on 2026-09-10, three routes to that executable are outside both.

The `spec` recipe in the `Makefile` puts `target/debug` on `PATH` and runs
`mustmatch test` over `spec/`. Four blocks in `spec/cli.md` copy
`../target/debug/pangopup` and run it, and `spec/model-routing.md:146` opens
`$XDG_CACHE_HOME/pangopup/model-results.sqlite3` directly. What keeps that off
the operator's cache is one assignment in the recipe,
`XDG_CACHE_HOME="$(CURDIR)/target/spec-cache"`. It is correct today. No gate
reads it, `HOME` is not moved beside it, and a block that cleared
`XDG_CACHE_HOME` would fall back to the home directory of whoever ran `make
spec`.

`cargo run --package pangopup-cli` reaches the same executable without naming
it. `pangopup-cli` declares one `[[bin]]`, so the `--bin` argument the shell
scan looks for is optional, and a harness spelled that way is read as quiet.

`maintainers/ticket-053/measure.py:704` runs `target/release/pangopup` from
Python. It passes `--model-cache` explicitly at line 492, which is the
accident ticket 0071 exists to remove rather than a rule. It is maintainer-run
and no gate invokes it.

Done, observably:

- A recipe or spec block that runs the built executable without a cache home of
  its own is refused by a check, not by inspection.
- `cargo run --package pangopup-cli` with no `--bin` is read as a run.

Boundary: no product behaviour, cache location, default, or scoring assertion
changes.
