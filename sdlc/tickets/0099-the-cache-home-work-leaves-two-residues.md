---
flow: build
priority: 2
---
# The cache home work leaves two residues

Ticket 0092 closed the three gaps ticket 0090 left. It left two things behind,
both measured in this checkout on 2026-09-11. They are one ticket because they
are one job: finishing the cache home work so a later contributor inherits a
rule that holds and a download that survives.

## A route added tomorrow may inherit the entry limit

0092 made four named routes drop `PANGOPUP_MODEL_CACHE_MAX_ENTRIES`: the `spec`
recipe, the `test` recipe, `crates/pangopup-cli/tests/support/mod.rs` and
`child_environment` in `maintainers/ticket-053/measure.py`.
`tests/model-cache-limit-inheritance.sh` holds those four by name.

A fifth route is not held. `tests/recipe-spawn-cache-isolation.sh` is the scan
that reads every Makefile recipe reaching the built executable, and its
`named_locations` array carries three names. With a throwaway recipe appended
to the `Makefile` that runs `pangopup --version` under `XDG_CACHE_HOME` and
`HOME` in `$(CURDIR)/target`:

- Dropping all four names: accepted, and the scan reported 2 recipes examined
  rather than 1.
- Dropping the three older names but not the entry limit: **accepted**. Every
  gate stayed green, and `make test` stayed green, on a recipe that hands a run
  of the built executable whatever limit the operator exported.

That is the defect 0092 was written about, in a route 0092 could not enumerate
because it did not exist yet. The array's own comment calls its entries the
variables that name a cache location outright, and the entry limit names none,
so the category has to be renamed or a second rule put beside it.

## The downloaded library sits where a routine command deletes it

0092 moved the ONNX Runtime static library out of `target/spec-cache`, which the
`spec` recipe removes on every run, and into `target/ort-cache`, which it does
not. The 90647244-byte file is still inside `target/`. No `clean` target exists
in the `Makefile` and no script in the repository runs `cargo clean`, so nothing
here removes it. `cargo clean` removes all of `target/`, and the next `make spec`
that rebuilds `ort-sys` fetches those bytes again over the network. `cargo clean`
is a routine command, and a rebuild is what an operator runs it for.

A machine that runs both gates also holds two copies. `make lint` and `make test`
build `ort-sys` under the cache the operator resolves, normally
`~/.cache/ort.pyke.io`; `make spec` builds it under `target/ort-cache`. Measured:
275 MB in the first, 87 MB in the second, one digest in both. They do not churn
each other, because `ort-sys` emits no `cargo:rerun-if-env-changed` for
`ORT_CACHE_DIR` -- measured as a 13.03-second `make spec` straight after a
`make test`.

Settled: the recipe keeps a cache home of its own and keeps deleting the
directory it deletes today. What moves is only the downloaded library.

Done, observably:

- A Makefile recipe that reaches the built executable while inheriting
  `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` is refused, naming the recipe and the
  variable, and a recipe dropping all four is accepted.
- The check that refuses it counts the recipes it read and refuses a count of
  zero, and the name that shares a prefix with `PANGOPUP_MODEL_CACHE` is not
  counted as held by a match on the shorter name it contains.
- `cargo clean` followed by `make spec` reaches no download, measured as bytes
  written to the network-fetched library path rather than asserted.
- One copy of the ONNX Runtime library serves every gate on a machine, or the
  second copy is justified in a comment where the recipe names it.
- A check refuses a recipe that points a cache of downloaded artifacts at a
  directory that command removes, whether the remover is the recipe itself or
  `cargo clean`.
- `make lint`, `make test` and `make spec` pass, and `make test` wall time does
  not grow by more than two seconds.

Boundary: this ticket changes no product source under `crates/`, alters no
score, position, status, reason or provenance field, and adds no route, flag or
output. `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` keeps working exactly as it does for
an operator running the product; only spawns under test stop inheriting it. The
`spec` recipe keeps a cache home of its own and keeps removing the directory it
removes today. It does not revisit what tickets 0090 or 0092 shipped, changes no
default cache location, and publishes no new number about scoring.

It names no private consumer of this software.
