---
flow: build
priority: 1
deps: []
---
# Cache rule coverage meets its test budget

## Outcome

The cache-rule mutation checks keep their original combined two-second warm-run budget without weakening the failures they prove.

## What is true today

On 2026-09-19 after ticket 0141, one warm local run measured `recipe-cache-rule-coverage.sh` at 2.38 seconds and `harness-rule-coverage.sh` at 9.65 seconds. Their combined 12.03 seconds exceeds the recorded two-second budget. `recipe-spawn-cache-isolation.sh` itself measured 2.12 seconds, so the earlier 23-second whole-checkout scan is no longer the main cost.

## Done, observably

- Profile the two coverage harnesses on the accepted parent and identify the repeated work responsible for the current cost.
- Preserve every named mutation and positive control. A faster check must still fail each defect independently for the recorded reason.
- Run each warm harness at least five times on one otherwise idle supported host. Record all runs and the median. The sum of the two medians must not exceed two seconds.
- Keep memory and work bounded by the tracked fixture inputs. Do not hide cost by starting retained background work or sharing mutable state across gate runs.
- Archive draft 0109. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change repository checks and their fixtures only. Do not reduce coverage, raise the budget, alter product behavior, or depend on machine-specific commands.
