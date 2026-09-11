---
---
# The release qualification runner holds the cache rule by inspection

`scripts/run-production-qualification.sh` runs the shipped executable eleven
times. It is the script the release runbook tells an operator to run by hand,
on that operator's own machine. It holds the cache rule correctly today: line
19 refuses a relative `XDG_DATA_HOME`, `XDG_CACHE_HOME` or output directory,
line 32 drops `PANGOPUP_DATA_DIR`, `PANGOPUP_CACHE_DIR`, `PANGOPUP_MODEL_CACHE`
and `PANGOPUP_MODEL_CACHE_MAX_ENTRIES`, and lines 33 to 36 set both homes from
the arguments it was given.

No gate reads any of that. Measured in this checkout on 2026-09-10.
`tests/shell-spawn-cache-isolation.sh` scans every `*.sh` in the repository and
reports 38 sources and 4 harnesses running the built executable. This file is
not among the four, because it runs `"$pangopup"` from its first argument
rather than naming a path the scan can see, so the scan reads it as quiet.

Ticket 0090 gave the two spawn helpers the rule and gave the `Makefile` recipe,
the spec blocks and the maintainer measurement a check of their own. This route
is the remaining one whose correctness rests on a reader noticing lines 19, 32
and 33 together. Deleting any one of them leaves every gate green and hands a
release qualification run the cache of whoever started it.

Done, observably:

- A check refuses `scripts/run-production-qualification.sh` if it stops
  requiring absolute runtime paths, stops dropping the four inherited names, or
  stops setting both homes.
- A script that runs an executable handed to it by argument is read as a run
  rather than as quiet.
- `make test`, `make spec` and `make lint` pass.

Boundary: no product behaviour, cache location, default, or scoring assertion
changes, and the runner keeps taking the same five arguments.
