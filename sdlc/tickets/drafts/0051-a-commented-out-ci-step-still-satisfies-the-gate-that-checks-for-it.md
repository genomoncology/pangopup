---
---
# A commented-out CI step still satisfies the gate that checks for it

Two shipped gates prove a workflow runs a command by searching the workflow file for that command's text. A YAML comment carrying the same text satisfies the search. The step it stands for can be deleted or replaced with a no-op and the gate stays green. Measured in this checkout on 2026-09-09.

`tests/ci-platform-support.sh:49` requires the text `run: cargo check --locked --target aarch64-unknown-linux-gnu --package pangopup-cli` inside the slice of `.github/workflows/ci.yml` that holds the Linux ARM64 step. The match is a shell `case` substring test, so it does not care whether the text stands in code. I replaced that step's body with `run: "true"` and left the original line above it as a YAML comment. The workflow still parses, the ARM64 cross-compile no longer happens, and `bash tests/ci-platform-support.sh` exited 0. I reverted the edit; the working tree is clean.

Lines 43, 44, 47, 48 and 52 of the same file are the same shape. Line 53 counts `run: scripts/run-linux-tests-with-public-failure.sh` and requires exactly one occurrence, so a comment added beside the real line is caught; a comment replacing the real line is not.

`tests/executable-delivery.sh:401-431` reads `.github/workflows/package-linux.yml` through about fifteen `grep -Fq` calls with no comment anchor. Line 421 requires `cargo build --locked --release --package pangopup-cli`. On a copy of that workflow I commented out line 50, the only line carrying that text, and the same `grep -Fq` still exited 0. The release build could be removed from the packaging workflow with the gate reporting nothing.

This is the shape ticket 0050 closed inside its own new harness. Its design review found `tests/built-executable-currency.sh` accepting `# see scripts/require-built-commands.sh` as proof a harness builds first, and anchored the three discovery patterns with `^[^#]*` so a match has to stand in code. Ticket 0050's boundary forbade touching the older gates, so they still carry it.

The repair is the same anchor, applied where each gate reads a workflow file: require the match to sit on a line whose leading text carries no `#`. The two harnesses named above are where it lives. `tests/ci-platform-support.sh` needs its `require_text` helper to work per line rather than over a joined slice, which is the larger half of the work. `tests/ci-test-failure-evidence.sh` is not affected; it reads a Makefile line and a shell wrapper, not a workflow.

Whatever fixes this needs its own break-it proof: comment out each guarded command in turn and observe the gate refuse.
