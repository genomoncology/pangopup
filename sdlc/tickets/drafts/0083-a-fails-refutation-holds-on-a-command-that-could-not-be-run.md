---
---
# A `--fails` refutation holds on a command that could not be run

`scripts/spec-refutes.sh --fails <command...>` refuses status 127, because a
command that was never there to run proves nothing by failing. A command that
is there and cannot be run exits 126 instead, and 126 falls through to the
branch that reads any other non-zero status as the refutation holding.

Measured in this checkout on 2026-09-10 while running the code review of ticket
0080. A shell script with mode 644 was named to `--fails`. The helper exited 0
and reported nothing, so the pin was green on a command that never ran a line.
The same status comes back when the named path is a directory.

`spec/runtime-transport.md` names `pangopup-build` on three `--fails` pins. A
build that leaves the executable without its execute bit turns all three green.

Done, observably:

- A `--fails` pin whose command could not be run fails, naming the command.
- `tests/spec-refutation-evidence.sh` holds that against a fixture, beside the
  127 case it already holds.

Boundary: this is about statuses that mean the command did not run. It states
no new claim about what any command does when it does run, and 127 keeps the
message it has.
