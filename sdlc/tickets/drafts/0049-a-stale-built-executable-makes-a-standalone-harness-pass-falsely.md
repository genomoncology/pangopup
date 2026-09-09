---
---
# A stale built executable makes a standalone harness pass falsely

Two shell harnesses use `target/debug/pangopup` and guard it the same way:

- `tests/production-release-qualification.sh` exports it to the qualification stub and re-renders the seven SNV groups with it.
- `tests/executable-delivery.sh:79` runs it directly to prove an unsafe model-cache path is refused.

Both check only `[[ -x "$real_cli" && ! -L "$real_cli" ]]`. Neither checks that the binary matches the source it is standing in for.

Under `make test` this is safe, and it was measured rather than assumed. `make test` runs `cargo test --locked $(WORKSPACE_TESTS)` before any shell harness, and that step rewrites `target/debug/pangopup`. Removing the binary and running `cargo test --locked --package pangopup-cli --no-run` put it back. CI runs `make test`, so CI is safe too.

The exposure is a standalone run. Edit the renderer, skip the build, run `bash tests/production-release-qualification.sh` on its own, and the stub and the comparison beside it both use the same stale binary. They agree, the checker compares stale bytes against oracles the stale binary was written for, and the harness goes green while the release would fail. A stale binary is a renderer from the past, and that harness exists to catch renderer drift.

Ticket 0046 moved the guard so an *absent* binary now reports `build the command-line tool before this harness: cargo build --package pangopup-cli` instead of a confusing runner failure. Staleness still reports nothing.

Options, cheapest first:

- Have each harness run `cargo build --locked --quiet --package pangopup-cli` before its first use of the binary. `make spec` already does exactly this, so the pattern exists in the repository. Costs a no-op cargo invocation, measured at roughly 0.2 s when the binary is current.
- Compare the binary's mtime against the newest file under `crates/` and refuse when it is older. No cargo dependency, but mtime comparisons are their own trap.
- Leave both harnesses alone and say in each one that it assumes a current build.

Whatever is chosen should apply to both harnesses, so the two do not drift apart again.
