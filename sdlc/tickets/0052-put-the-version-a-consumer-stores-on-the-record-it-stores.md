---
flow: build
priority: 2
---
# Put the version a consumer stores on the record it stores

`architecture/compatibility.md` tells a consumer to store `data_set_version` beside every retained score. The service does not put that value on a score. `StatusOutput` carries `scoring_identity`, `data_set_version`, `runtime_profile_id` and `scoring_semantics` (`crates/pangopup-cli/src/service.rs:517`). `add_service_fields` attaches one identity to each score item, and it is `scoring_identity` (`crates/pangopup-cli/src/service.rs:1613`). A consumer reading a scored item finds the wrong version in hand and has to make a second request for the right one.

The two values differ in exactly the way that matters. `ActiveScoringIdentityPreimage` carries `effective_cpu_policy`, so `scoring_identity` moves when a deployment starts with a different `--model-threads`. `ScoringDataSetVersionPreimage` carries the schema, the software version and the runtime profile identity, so `data_set_version` does not move. Ticket 0040 measured that a thread or worker setting moves no score, no position, no status and no rejection reason.

So a deployment that restarts with a different thread count writes a different stored version beside identical numbers. A consumer that later groups or compares on that stored value sees a data-set change that did not happen.

This is not a documentation gap. A downstream annotation store integrating against v0.4.1 took the identity the item carried and named its own column after the data-set version. That column now holds `scoring_identity`. The integration could not have done better, because v0.4.1 published no other identity on an item.

v0.5.0 is not published. The current public release is v0.4.1. Adding the field now puts it in the same response-shape inventory a consumer already has to accept for v0.5.0, instead of a second inventory entry against a shipped release.

Done, observably:

- Every scored HTTP item carries `data_set_version` beside `scoring_identity`.
- The value on an item equals the value the status response reports, and every item in one response reports the same value.
- Starting the service with a different `--model-threads` moves `scoring_identity` on a scored item and leaves `data_set_version` on that same item unchanged. One test proves both halves against one variant.
- A rejected item carries `data_set_version` wherever it carries `scoring_identity` today, so a consumer storing an outcome stores one version whatever the outcome was.
- The v0.5.0 response-shape inventory names the added field, and the version gate and its independent portable copy both enforce the new entry.
- The published guidance states which identity to store and why, and a consumer reaching that answer needs no second request.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage.

Boundary: this ticket adds a field a consumer reads. It must not change any score, position, status, rejection code or rejection reason. It must not change `scoring_identity`'s value, its meaning, or its place on the status response and on every score item.

It must not add the field to the command-line tool's JSONL output. A `--bundle` lookup opens no installed runtime profile, so `data_set_version` is not computable on every command-line path. What a retained command-line score pins is a real question and it gets its own ticket. Say so in the guidance rather than answering it here.

The JSONL oracles under `tests/fixtures/` and their pinned digests in `scripts/check-production-qualification.py` do not move, because the command-line output does not change. The checker's HTTP item-shape comparison does change, and the gate ticket 0046 built will require that change rather than discovering it at release time.

It must not publish a release or move a published artifact pin.
