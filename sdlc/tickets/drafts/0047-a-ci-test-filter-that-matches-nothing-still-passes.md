---
---
# A CI test filter that matches nothing still passes

`.github/workflows/ci.yml:78` (Linux) and `.github/workflows/ci.yml:187` (macOS) both run:

```
cargo test --locked --package pangopup-cli --features service-test-fixtures --test http_service_lifecycle installed_success
```

The trailing `installed_success` is a test-name filter. It is not a test name. It is a `mod` at `crates/pangopup-cli/tests/http_service_lifecycle.rs:7`, and the filter matches because every test inside it is named `installed_success::<something>`.

`cargo test` exits 0 when a filter matches nothing. Measured in this checkout:

```
$ cargo test --locked --package pangopup-cli --features service-test-fixtures \
    --test http_service_lifecycle installed_success
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out

$ cargo test --locked --package pangopup-cli --features service-test-fixtures \
    --test http_service_lifecycle installed_successXYZ
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out
EXIT=0
```

So renaming or removing that module drops the step from twelve assertions to zero and the CI job stays green. Nobody sees a failure, and nothing in the repository says the count must stay above zero.

That step is not incidental coverage. `make test` runs `cargo test --locked --workspace` with no `--features`, so `service-test-fixtures` never builds there. This CI step is the only place a real `pangopup serve` lifecycle runs against a runtime installed from repository fixtures. Ticket 0046's design leans on that step existing when it defers gating the live HTTP surface.

Ticket 0040 established the same trap in `spec/` blocks and closed it by requiring evidence that the assertions ran. The same instrument fits here: assert the executed count, or filter on something that cannot silently stop matching.

Related, and a candidate for the same sweep: `scripts/run-linux-tests-with-public-failure.sh` hardcodes `file=Makefile,line=30`, held by `tests/ci-test-failure-evidence.sh`. Both are hardcoded references into another file that go quiet rather than loud when their target moves.
