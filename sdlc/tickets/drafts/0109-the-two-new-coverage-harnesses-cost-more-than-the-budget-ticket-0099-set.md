---
---
# The two new coverage harnesses cost more than the budget ticket 0099 set

Ticket 0099 asked that `make test` wall time not grow by more than two seconds.
Measured in this checkout on 2026-09-11, warm, after the ticket landed:

- `tests/recipe-cache-rule-coverage.sh` 2.55 s
- `tests/harness-rule-coverage.sh` 0.56 s

Together 3.11 s, against a budget of 2 s. The four gates the ticket changed did
not move: `tests/recipe-spawn-cache-isolation.sh` ran 23.08 s after the change
and 23.76 s before it, `tests/spec-download-cache-durability.sh` 0.31 s against
0.28 s, `tests/built-executable-currency.sh` 0.26 s against 0.30 s, and
`tests/negative-assertion-strength.sh` 0.43 s against 0.61 s, even though it now
reads a second harness. The whole `make test` run took 155.65 s.

Almost all of the 2.55 s is `tests/recipe-cache-rule-coverage.sh` running
`tests/recipe-spawn-cache-isolation.sh` four times against a tree carrying this
repository's `spec/` and `maintainers/` directories. That gate spends its time
one `sed | grep` per file per variable, and with the entry limit added it now
reads six variables rather than five.

The same shape costs far more in the real tree. `python_holds` runs
`find` over the whole checkout for `*.py` and gets 5,692 files here, because
nothing excludes the agent tooling directories `.gitignore` already names, and
it pays a `sed | grep` for every one of them. That is where the gate's 23 s
goes, and it is what a faster coverage harness would have to fix first.

Nothing here is wrong, only slow. The question for this ticket is whether the
budget moves or the scan does.
