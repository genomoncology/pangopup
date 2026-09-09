---
flow: build
priority: 3
---
# A check that stops checking says so

Two always-on checks can go green without having exercised what they claim to. Both were measured in this checkout on 2026-09-09.

`.github/workflows/ci.yml:78` and `:187` run `cargo test --locked --package pangopup-cli --features service-test-fixtures --test http_service_lifecycle installed_success`. The trailing `installed_success` is a test-name filter, and it is not a test name. It is a `mod` at `crates/pangopup-cli/tests/http_service_lifecycle.rs:7`, and the filter matches because every test inside is named `installed_success::<something>`. `cargo test` exits 0 when a filter matches nothing. The real filter reports `12 passed; 0 failed; 3 filtered out`. `installed_successXYZ` reports `0 passed; 0 failed; 15 filtered out` and exits 0. Renaming or removing that module drops the step from twelve assertions to zero and the job stays green.

That step is not incidental. `make test` runs `cargo test --locked --workspace` with no `--features`, so `service-test-fixtures` never builds there. This is the only place a real `pangopup serve` lifecycle runs against a runtime installed from repository fixtures, and ticket 0046 deferred gating the live HTTP surface partly because it exists.

`tests/production-release-qualification.sh:209` and `tests/executable-delivery.sh:79` both use `target/debug/pangopup` and guard it the same way, with `[[ -x "$real_cli" && ! -L "$real_cli" ]]`. Neither checks that the binary matches the source it stands for. Under `make test` and in CI this is safe, because `cargo test --locked --workspace` runs first and rewrites the binary. A standalone run is not safe. Edit the renderer, skip the build, run `bash tests/production-release-qualification.sh` on its own, and the stub and the comparison beside it both use the same stale binary. They agree, the checker compares stale bytes against oracles the stale binary was written for, and the harness goes green. A stale binary is a renderer from the past, and that harness exists to catch renderer drift.

Two nearby things were measured and are not holes. Do not spend work on either. `cargo test --package pangopup-cli --test no_such_target` exits 101, so naming a target that does not exist fails loudly and only the trailing name filter is silent. `tests/ci-test-failure-evidence.sh:12` asserts that `Makefile` line 30 starts with `test:`, so the hardcoded `file=Makefile,line=30` annotation in `scripts/run-linux-tests-with-public-failure.sh` is already held. Ticket 0045 did that one correctly.

Done, observably:

- Renaming or removing the module CI's feature-gated service step exercises fails that step, instead of passing with nothing run. A test proves it by making the filter match nothing and observing the failure.
- Both harness call sites of `target/debug/pangopup` refuse to run against an executable that does not match the source it stands for, or make it match first. A test proves the false green is gone: change what the renderer prints, skip the build, run the harness standalone, and it fails.
- Whatever answers the stale-binary question answers it the same way in both harnesses, so the two cannot drift apart again.
- No gate reaches the network. `make test` does not gain a second full workspace build, and its wall time does not grow by more than two seconds.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage.

Boundary: this ticket changes how a check proves it ran. It must not change what any test asserts, what the executable prints, the oracles at `tests/fixtures/`, their pinned digests, `pangopup-regression-fixture`, or the runbooks under `sdlc/planning/artifacts/`.

It must not enable `service-test-fixtures` in `make test`. Ticket 0046's design rejected that on cost, because it means a second full workspace build for one surface. The CI step stays where it is; this ticket makes it honest, not local.

It must not change the `file=Makefile,line=30` coupling or `tests/ci-test-failure-evidence.sh`, both of which already hold.
