---
flow: build
priority: 5
---
# The qualification cache-isolation check runs on macOS Bash

Mac `make test` now passes `tests/published-claim-evidence.sh` and stops at `tests/qualification-runner-cache-isolation.sh:118` with `mapfile: command not found`. macOS ships Bash 3.2, and this check uses `mapfile` again for its successful-spawn case. The check cannot currently prove that qualification runs reject relative paths and keep inherited cache settings away from an operator's files.

The check must run under the system Bash on macOS and under Linux Bash without changing the qualification runner. It must still test all three relative-path refusals before any executable starts, then test a recorded spawn under the supplied data and cache homes with inherited PangoPup cache settings removed.

Done, observably:

- `/bin/bash tests/qualification-runner-cache-isolation.sh` exits 0 on macOS and Linux and reports its eight checks against one recorded spawn.
- Its negative cases still refuse relative data, cache, and output paths before starting the stub. Each case also proves that all supplied absent absolute candidate paths and the relative working-tree paths remain uncreated.
- Its positive spawn still records only the supplied private homes and no inherited PangoPup cache location.
- Mac `make test` advances beyond this check. Run `make lint`, `make test`, and `make spec` before commit; report separate remaining failures honestly.

Boundary: do not change the qualification runner, production cache behavior, or the separate Bash associative-array failure in the route-disagreement check. Record Mac and Linux evidence in `sdlc/records/`; no public score or operator documentation changes.
