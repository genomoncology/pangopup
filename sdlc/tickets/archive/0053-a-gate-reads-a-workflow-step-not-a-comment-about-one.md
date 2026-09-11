---
flow: build
priority: 3
---
# A gate reads a workflow step, not a comment about one

Two shipped gates prove a workflow runs a command by searching the workflow file for that command's text. A YAML comment carrying the same text satisfies the search, so the step it stands for can be deleted or replaced with a no-op while the gate stays green. Measured in this checkout on 2026-09-09, and reproduced twice since.

`tests/ci-platform-support.sh:49` requires the text `run: cargo check --locked --target aarch64-unknown-linux-gnu --package pangopup-cli` inside the slice of `.github/workflows/ci.yml` holding the Linux ARM64 step. The match is a shell `case` substring test over a joined slice, so it does not care whether the text stands in code. Replacing that step's body with `run: "true"` and leaving the original line above it as a comment leaves the workflow parsing, stops the ARM64 cross-compile, and `bash tests/ci-platform-support.sh` exits 0. Lines 43, 44, 47, 48 and 52 of that file are the same shape. Line 53 counts occurrences and requires exactly one, so a comment beside the real line is caught and a comment replacing it is not.

`tests/executable-delivery.sh:401-431` reads `.github/workflows/package-linux.yml` through about fifteen `grep -Fq` calls with no comment anchor. Line 421 requires `cargo build --locked --release --package pangopup-cli`. Commenting out the only line carrying that text leaves the same `grep -Fq` exiting 0. The release build could leave the packaging workflow with the gate reporting nothing.

Ticket 0050 closed this shape inside its own new harness. Its design review found `tests/built-executable-currency.sh` accepting `# see scripts/require-built-commands.sh` as proof a harness builds first, and anchored that harness's three discovery patterns so a match has to stand in code. Ticket 0050's boundary forbade touching the older gates, so those still carry it, and its record says so.

The gates guard the ARM64 cross-compile and the release build. Both decide what ships.

Done, observably:

- A guarded command commented out in a workflow fails the gate that claims the workflow runs it. A test proves it by commenting out each guarded command in turn and observing the refusal.
- The refusal names the command and the workflow file, so a reader knows what stopped running.
- Both harnesses answer this the same way, so the two cannot drift apart again.
- Every command these gates guard today is still guarded, and no gate starts passing on something it refused before.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage. `make test` wall time does not grow by more than two seconds.

Boundary: this ticket changes how a gate reads a workflow file. It must not change either workflow's behavior, add or remove a CI step, or change what any step runs.

It must not change what the executable prints, the oracles under `tests/fixtures/`, their pinned digests, `pangopup-regression-fixture`, or the runbooks under `sdlc/planning/artifacts/`.

It must not change `tests/ci-test-failure-evidence.sh`, which reads a Makefile line and a shell wrapper rather than a workflow, and already holds the `file=Makefile,line=30` coupling. It must not re-open the three discovery patterns ticket 0050 anchored in `tests/built-executable-currency.sh`.
