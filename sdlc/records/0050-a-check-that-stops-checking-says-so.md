---
base: c07477888e164c7b8fdc970df6117260b9bbe70e
head: 8ba45937ed93b15223a5e1fd484fad337ee63b7f
---

# A check that stops checking says so

Two always-on checks could pass without exercising what they claimed.

CI's only feature-gated service step ran `cargo test --locked --package pangopup-cli --features service-test-fixtures --test http_service_lifecycle installed_success`. The trailing word is a test-name filter. It is not a test name. `cargo test` exits 0 when a filter matches nothing, so renaming or deleting that module dropped the step from twelve assertions to zero and the job stayed green. Separately, `tests/production-release-qualification.sh` and `tests/executable-delivery.sh` ran `target/debug/pangopup` after checking only that the file existed. A standalone run compared a renderer built from older source against oracles written for the current one. Both harnesses agreed with the stale bytes and passed.

`scripts/run-service-fixture-tests.sh` now runs CI's step, reads cargo's summary line back, and refuses a run that passed no test. `scripts/require-built-commands.sh` builds `pangopup` and `pangopup-build` before either harness reaches into `target/debug`, so one mechanism answers the stale-binary question at both call sites. `tests/ci-service-fixture-evidence.sh` and `tests/built-executable-currency.sh` hold both shut, and `make test` runs them. No assertion changed, no oracle moved, and `make test` still builds no `service-test-fixtures`.

One hole of the same shape stays open. Draft 0051 records it. `tests/ci-platform-support.sh` and `tests/executable-delivery.sh` prove that a workflow runs a command by searching the workflow file for that command's text, and a YAML comment carrying the same text satisfies the search. I commented out the ARM64 cross-compile step, left its original line above it as a comment, and `tests/ci-platform-support.sh` exited 0. This ticket's boundary forbade touching those two gates, so it anchored only its own three discovery patterns.
