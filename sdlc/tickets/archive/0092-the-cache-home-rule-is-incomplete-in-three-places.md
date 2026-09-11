---
flow: build
priority: 1
---
# The cache home rule is incomplete in three places

Ticket 0090 gave every route to the built executable a cache home of its own
and a check that holds it. Three gaps survived that work. They are one ticket
because they are one job: the routes, names and directories 0090 enumerated
were each short by one, and each shortfall is held by a reader rather than by a
gate. All three were measured in this checkout on 2026-09-10.

## A fourth name is inherited everywhere

0090 dropped three names at every route: `PANGOPUP_MODEL_CACHE`,
`PANGOPUP_CACHE_DIR` and `PANGOPUP_DATA_DIR`. A fourth,
`PANGOPUP_MODEL_CACHE_MAX_ENTRIES`, is read by `resolve_model_cache_options` in
`crates/pangopup-cli/src/main.rs` and is left inherited by both spawn helpers,
the `spec` recipe and `child_environment` in
`maintainers/ticket-053/measure.py`. The repository already knows the name:
`tests/production-release-qualification.sh:374` clears all four for the one
command it runs.

The consequence is not theoretical. Sourcing
`tests/support/private-cache-home.sh` from a shell that exported
`PANGOPUP_MODEL_CACHE_MAX_ENTRIES=not-a-limit` and running a miniature modelled
lookup printed:

```
{"status":"error","code":"CLI_USAGE","message":"invalid model cache configuration: model cache maximum must be a positive integer or unlimited","details":null}
```

Every modelled lookup fails that way, so an operator who exported the variable
cannot run `make test` at all. A valid but small value is worse: the suite runs,
the cache evicts on a schedule the operator chose, and the scoring harnesses
read cache state they did not set up. The maintainer measurement has the same
gap while asserting on the number of rows the cache holds.

## The release qualification runner is held by inspection

`scripts/run-production-qualification.sh` runs the shipped executable eleven
times. It is the script the release runbook tells an operator to run by hand,
on that operator's own machine. It holds the rule correctly today: line 19
refuses a relative `XDG_DATA_HOME`, `XDG_CACHE_HOME` or output directory, line
32 drops all four names, and lines 33 to 36 set both homes from its arguments.

No gate reads any of it. `tests/shell-spawn-cache-isolation.sh` scans every
`*.sh` in the repository and reports 38 sources and 4 harnesses running the
built executable. This file is not among the four, because it runs `"$pangopup"`
from its first argument rather than naming a path the scan can see, so the scan
reads it as quiet. Deleting any one of those three lines leaves every gate green
and hands a release qualification run the cache of whoever started it.

## The spec recipe throws away a downloaded library before every run

The `spec` recipe points `XDG_CACHE_HOME` at `target/spec-cache` and removes
that directory first, so the run starts with an empty cache. `ort-sys` builds
with `download-binaries`, and that build script caches the ONNX Runtime static
library under `$XDG_CACHE_HOME/dfbin`. The recipe therefore discards that
library before every run, and any run that rebuilds `ort-sys` fetches it again
over the network.

A `make spec` that rebuilt the registry crates wrote 87 MB to
`target/spec-cache/dfbin/x86_64-unknown-linux-gnu/<digest>/libonnxruntime.a` and
took 157.00 seconds. The next run, with nothing to rebuild, wrote 20480 bytes
and took 13.03 seconds. No copy of that library exists in the checkout or under
`CARGO_HOME`, so the 87 MB came from the network. This is older than ticket
0090: the base commit's recipe already removed that directory and already
pointed `XDG_CACHE_HOME` at it. 0090 pinned `CARGO_HOME` and `RUSTUP_HOME`
against the same class of accident for the toolchain and left this one standing.

Done, observably:

- Both spawn helpers, the `spec` recipe and `child_environment` drop
  `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` alongside the three names they already
  drop, and an operator who exported it can run `make test` and `make spec`
  unchanged.
- The checks that hold those routes read four names rather than three, and the
  longer name is not counted as held by a check that only found the shorter one
  it contains.
- A check refuses `scripts/run-production-qualification.sh` if it stops
  requiring absolute runtime paths, stops dropping the inherited names, or stops
  setting both homes.
- A script that runs an executable handed to it by argument is counted as a run
  rather than as quiet, and the count of harnesses the scan reports rises to
  match.
- A rebuild of `ort-sys` under `make spec` reaches no download, because the
  library cache it reads is not a directory the recipe removes.
- A check refuses a recipe that points a cache of downloaded artifacts at a
  directory that same recipe deletes.
- Every new check counts what it read and fails when that count is zero, so a
  scan matching nothing is refused rather than passed.
- `make test`, `make spec` and `make lint` pass, and the operator's own model
  cache is byte-identical after all three.

Boundary: this ticket changes no product source under `crates/`, alters no
score, position, status, reason or provenance field, and adds no route, flag or
output. `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` keeps working exactly as it does
today for an operator running the product; only spawns under test stop
inheriting it. The release qualification runner keeps taking the same five
arguments and keeps doing what it does. `make spec` keeps running under a cache
home of its own, and the directory it deletes each run stays deleted each run.
It does not revisit what ticket 0090 shipped, changes no default cache location,
and publishes no new number about scoring.

It names no private consumer of this software.
