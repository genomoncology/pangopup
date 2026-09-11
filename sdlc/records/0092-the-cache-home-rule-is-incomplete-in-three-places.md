---
base: fb3a7c9e1ae5ed83c2b421e3bdffd3732197eebe
head: caceecf5209bbf75d07d3862452683b571200d34
---
# The cache home rule is incomplete in three places

Ticket 0090 left three routes short by one. `PANGOPUP_MODEL_CACHE_MAX_ENTRIES`
is now dropped at the `spec` recipe, the `test` recipe,
`crates/pangopup-cli/tests/support/mod.rs`, `child_environment` in
`maintainers/ticket-053/measure.py` and `tests/support/private-cache-home.sh`.
`tests/model-cache-limit-inheritance.sh` reads the first four as routes, runs
the helper to read the fifth, and tells the long name apart from the
`PANGOPUP_MODEL_CACHE` inside it.
`tests/qualification-runner-cache-isolation.sh` holds
`scripts/run-production-qualification.sh` by running it rather than by reading
it. The `spec` recipe no longer points the ONNX Runtime library cache at a
directory it deletes: `ORT_CACHE_DIR` names `target/ort-cache` on the
`cargo build` line and on the suite line, and
`tests/spec-download-cache-durability.sh` refuses a recipe that aims a download
cache at a directory that recipe removes.

Exercised beyond the suite under a private `HOME`, `XDG_CACHE_HOME`,
`XDG_DATA_HOME` and `TMPDIR`, with real `CARGO_HOME` and `RUSTUP_HOME`, and with
the operator's own `~/.cache/ort.pyke.io` read but never written.

The operator scenario was run whole. All four names were exported in one shell:
`PANGOPUP_MODEL_CACHE` at a real 4-row model cache file the product itself had
written, mode 600; `PANGOPUP_CACHE_DIR` and `PANGOPUP_DATA_DIR` at directories;
and `PANGOPUP_MODEL_CACHE_MAX_ENTRIES=not-a-limit`. `make test` (124.21 s, a
rebuild), `make spec` (13.57 s), `bash tests/production-release-qualification.sh`
and a direct `bash scripts/run-production-qualification.sh` with stand-in
arguments all passed. The planted file was byte-identical after every one of
them: md5 `37a9c891036fef3b9a3e8e97f11335fc`, cksum `1328087005 20480`, size
20480, inode and mtime unchanged. The same run against the ticket's own defect
is the control: the same file, copied, with
`PANGOPUP_MODEL_CACHE_MAX_ENTRIES=1` reaching a lookup, went to md5
`71ea96ec80ae0a67e2ab3012f7453868` with 3 of its 4 rows evicted. That is what
every route inherited before this candidate.

The release qualification runner was run directly against a stub that records
its own environment. It received `PANGOPUP_MODEL_CACHE`, `PANGOPUP_CACHE_DIR`,
`PANGOPUP_DATA_DIR` and `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` all unset, `HOME`,
`XDG_DATA_HOME` and `XDG_CACHE_HOME` all from its arguments, and refused a
relative `XDG_DATA_HOME` with `runtime paths must be absolute`.

The download was measured under a fresh empty `HOME` and `XDG_CACHE_HOME` with
`cargo clean -p ort-sys` first, on this candidate and on the base beside it.
The candidate wrote 0 bytes into the caller's cache and 90647244 bytes into
`target/ort-cache`, which the recipe does not remove, in 29.37 s. Cleaning
`ort-sys` again under a second fresh cache home cost 0 bytes in both places and
28.76 s, with the library's md5 unchanged: a rebuild reaches no download. The
base recipe's own four lines, run verbatim in the same checkout under a third
fresh cache home, wrote 90647244 bytes into the caller's cache from the
`cargo build` line and 90667724 bytes into `target/spec-cache` from the suite
line, in 29.57 s. That is the same library fetched twice in one run, half of it
into the directory the next `make spec` deletes first.

The product still reads the variable for an operator. Run directly against the
miniature model kernel: `1` leaves 1 row in the cache, `2` leaves 2,
`unlimited` leaves all 4, and `not-a-limit`, `0` and `-3` are each refused with
`invalid model cache configuration: model cache maximum must be a positive
integer or unlimited` and exit 2.

A maintainer is not obstructed. A new `Makefile` recipe running the executable
under `$(CURDIR)/target` with all four names dropped was accepted, and the
recipe scan reported 2 recipes examined rather than 1. The same recipe without
`-u PANGOPUP_MODEL_CACHE` was refused by name at `Makefile:114`.

Four costs are accepted rather than carried as open work.

The ticket's fourth Done bullet asked that an argument-run script be counted
among the harnesses and the harness count rise to match. It is not met and
should not be. A script in that count must source
`tests/support/private-cache-home.sh`, and these scripts take their cache homes
from their arguments, which the ticket's own Boundary preserves. What landed
instead: the shape is recognised mechanically with zero measured false
positives -- 24 on a physical-line scan, 0 using the file's existing
`logical_lines()` helper -- counted on its own reported line, and every script
of that shape must be named with the gate that reads it.
`tests/shell-spawn-cache-isolation.sh` reports 46 shell sources and 2 scripts
running an executable handed to them.

A relative `ORT_CACHE_DIR` is deliberately accepted. `ort-sys` takes the value
verbatim and cargo runs a build script from the dependency's own package root
under `CARGO_HOME`, so a relative value lands beside the crate source and never
in a directory the recipe removes.

`ort-sys` emits no `cargo:rerun-if-env-changed` for `ORT_CACHE_DIR`, so
alternating `make test` and `make spec` causes no rebuild churn: a `make spec`
straight after a `make test` took 13.03 s when the code review measured it and
13.57 s here.

Two copies of the library now exist per machine: 275 MB in the operator's own
`~/.cache/ort.pyke.io` and 87 MB in `target/ort-cache`, sharing digest
`acc1cba7`. Draft 0099 holds the question of a durable home outside `target/`,
which `cargo clean` collects.

Verify committed two changes of its own. Eight `__pycache__/*.pyc` files
entered the tree with the code review commit; they are untracked again and
`__pycache__/` is ignored. Draft 0100 names the one route the entry-limit rule
does not reach: a `Makefile` recipe added later that drops the three older names
and inherits the entry limit is accepted by every gate, because the
`named_locations` array in `tests/recipe-spawn-cache-isolation.sh` carries three
names and its own comment calls them the variables that name a cache location
outright, which the entry limit does not. Drafts 0098 and 0099 came in with the
candidate and were not refiled.

`make lint` (8.61 s), `make test` (105.98 s against a 104 s baseline),
`make spec`, `scripts/run-service-fixture-tests.sh`, all twenty
`PORTABLE_QUALIFICATION` gates and all three `SHELL_QUALIFICATION` gates run one
at a time, and `bash sdlc/scripts/lint` pass on this candidate. `make spec`
reports `306 passed, 6 skipped`, unchanged from the base.
`tests/model-cache-limit-inheritance.sh` reports 4 routes dropping 4 variables,
12 matcher cases and 7 runs. `tests/qualification-runner-cache-isolation.sh`
reports 8 checks across 1 recorded spawn.
`tests/spec-download-cache-durability.sh` reports 1 recipe examined. The
operator's own `~/.cache/pangopup/model-results.sqlite3` is byte-identical
after all of it: md5 `0ce402e99f91b4d16ed3aaddc88160fc`, cksum
`2360715374 303104`, mtime and inode unchanged.
