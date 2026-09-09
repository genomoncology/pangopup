---
flow: build
priority: 4
---
# A spec gate cannot pass on a cargo filter that matches nothing

`cargo test` exits 0 when a name filter selects no test. Gates in `spec/` run `cargo test` with a filter and read only the exit code, so each one passes on nothing once the test it names is renamed or deleted.

Five are known, all in `spec/http-service.md`: lines 77, 78 and 79 (`active_identity` on `pangopup-assets`, `scoring_identity` and `request_contract` on the `pangopup` binary), line 116 (`http_gene_filter_accepts_reported_identity_and_matches_its_stable_gene`), and the block at line 127 (`real_executable_` on `http_service_lifecycle`). Line 116 names one whole test, so a rename empties that gate completely rather than shrinking it. Twenty `cargo test` invocations live across `spec/`, and the five above are the ones already measured. Others may carry the same trap; finding them is part of the work.

Measured in this checkout on 2026-09-09. Each of these exited 0 with the filter altered so it matches nothing:

```
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures --bin pangopup scoring_identity_renamed >/dev/null 2>&1
cargo test --locked --quiet --package pangopup-assets active_identity_renamed >/dev/null 2>&1
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures --test http_service_lifecycle real_executable_renamed >/dev/null 2>&1
```

The same three commands piped to `rg -F '1 passed; 0 failed'` exit 1. Other gates in the same file already read the count back, and ticket 0052 added two more, so `spec/http-service.md` carries both shapes side by side.

Ticket 0050 closed this shape for CI's one feature-gated step. `scripts/run-service-fixture-tests.sh` reads cargo's summary line back and refuses a run that asserted nothing, and `tests/ci-service-fixture-evidence.sh` holds that arithmetic shut. Ticket 0050's boundary forbade reaching into other gates, so these stayed.

The scan covers every file under `spec/`, not just `spec/http-service.md`. A check that reads one file rots the same way the gates did.

Done, observably:

- Every `cargo test` gate under `spec/` fails when its filter is altered to match no test, and the failure names the filter and the spec file that carries it, so an operator can find the gate without searching.
- Every such gate still passes unaltered, and each one still covers at least the tests it covered before. No gate's coverage shrinks.
- A gate added later that reads only cargo's exit code is refused by a check that runs in the ordinary gate ladder. The check counts what it inspected and fails when it inspects nothing.
- `make spec` passes.

Whether the fix is a counting form written at each site or one shared helper the sites call is the design stage's decision. Both satisfy this ticket.

Boundary: this ticket does not change any test's name, any product source under `crates/`, or what any gate asserts about product behavior. It does not touch `scripts/run-service-fixture-tests.sh` or `tests/ci-service-fixture-evidence.sh`, which ticket 0050 closed. It does not add or remove a CI step, and it does not change gates outside `spec/` that already read a count back.
