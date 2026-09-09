---
flow: build
priority: 3
---
# Five spec gates pass on a cargo filter that matches nothing

`cargo test` exits 0 when a name filter selects no test. Five bash gates in `spec/http-service.md` run `cargo test` with a filter and read only the exit code, so each one passes on nothing after the test it names is renamed or deleted.

The five are lines 77, 78 and 79 (`active_identity` on `pangopup-assets`, `scoring_identity` and `request_contract` on the `pangopup` binary), line 116 (`http_gene_filter_accepts_reported_identity_and_matches_its_stable_gene`), and the block at line 127 (`real_executable_` on `http_service_lifecycle`). Line 116 names one whole test, so a rename empties that gate completely rather than shrinking it.

Measured in this checkout on 2026-09-09. Each of these exited 0 with the filter altered so it matches nothing:

```
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures --bin pangopup scoring_identity_renamed >/dev/null 2>&1
cargo test --locked --quiet --package pangopup-assets active_identity_renamed >/dev/null 2>&1
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures --test http_service_lifecycle real_executable_renamed >/dev/null 2>&1
```

The same three commands piped to `rg -F '1 passed; 0 failed'` exit 1. Other gates in the same file already use that counting form, and ticket 0052 added two more in it, so `spec/http-service.md` now carries both shapes side by side. No new instance of the trap was introduced; these five predate it.

Ticket 0050 closed this shape for CI's one feature-gated step. `scripts/run-service-fixture-tests.sh` reads cargo's summary line back and refuses a run that asserted nothing, and `tests/ci-service-fixture-evidence.sh` holds that arithmetic shut. Ticket 0050's boundary forbade reaching into other gates, so these five stayed.

A successor decides whether the fix is the counting form at each site or one reusable wrapper, and whether the same scan should cover every spec file rather than this one. It also decides what a gate should say when it refuses, because the counting form's failure today is a bare non-zero exit from `rg`.
