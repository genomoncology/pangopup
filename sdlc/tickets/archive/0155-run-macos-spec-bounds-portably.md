---
flow: build
priority: 1
deps: ["0154"]
---
# Run macOS spec bounds portably

## Outcome

The macOS CI spec gate reaches and evaluates the FIFO refusal and the retained runtime-v2 qualification.

## Done, observably

- Reproduce the public CI failures from run `35854996720`: the FIFO spec invokes a missing `timeout` command and the runtime-v2 qualification exceeds mustmatch's default 30-second block limit.
- Keep the FIFO verification bounded on Linux and macOS. An actual hang still fails the exact-output pin, while the legitimate refusal remains exit 1 with its pinned error.
- Give only the long qualification block a bound supported by observed execution. Retain its exact test selection, execution checks, output pin, and failure behavior.
- Run focused evidence and the repository lint, test, and spec gates. Record the result in the matching SDLC record.

## Boundary

Change spec portability and timeout behavior only. Do not weaken scoring, release, or asset checks.
