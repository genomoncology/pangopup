---
flow: build
priority: 3
---
# The release checker accepts one value published under both identity names

Ticket 0052 exists because a consumer stored `scoring_identity` under a column it named after the data-set version. `scripts/check-production-qualification.py` is the gate that judges the shipped HTTP item shape at release time. It does not catch that confusion.

`require_http_score` (`scripts/check-production-qualification.py:297`) holds an item's `scoring_identity` to the status `scoring_identity` (`:322`) and an item's `data_set_version` to the status `data_set_version` (`:327`). `main` reads both status values and checks each against the digest pattern (`:413`, `:416`). Nothing compares the two status values to each other. A deployment that published one value under both names satisfies every rule the checker applies.

Measured in this checkout on 2026-09-09. Loading the checker as a module and calling `require_http_score` with an item whose `scoring_identity` and `data_set_version` hold the same digest, against status values that hold that same digest, returns without failing. The same call with two different digests also returns. The checker cannot tell the two cases apart.

The inside-out tests do assert distinctness. `every_score_item_carries_the_status_data_set_version_beside_its_scoring_identity` in `crates/pangopup-cli/src/service_tests.rs` refuses a status response whose two values are equal, and `a_thread_setting_moves_the_item_identity_and_holds_its_data_set_version` in `crates/pangopup-cli/tests/http_service_lifecycle.rs` repeats that assertion against a real service. So a code change that collapsed the two values turns those red first. The release checker is the last reader before a published artifact, and it is the one that looks at a real deployment rather than a fixture.

`tests/production-release-qualification.sh:695` and `:697` drive the checker through a stub whose two values already differ, and the mutation list added by 0052 covers a missing, mistyped, malformed, cross-item and status-mismatched version. No mutation makes the two values equal.

The status response publishes a third digest, `runtime_profile_id`. Neither the checker nor `tests/production-release-qualification.sh` mentions it anywhere. The three digests hash three different preimages, so a real deployment publishes three different values, and any two of them being equal means a deployment collapsed something it should not have. The checker judges all three.

Done, observably:

- The checker refuses a deployment whose status response publishes the same digest under any two of `scoring_identity`, `data_set_version` and `runtime_profile_id`, and says which two names collided.
- The checker holds `runtime_profile_id` to the same digest pattern it already applies to the other two, and refuses a status response that omits it or publishes it malformed.
- `tests/production-release-qualification.sh` drives the checker through a stub for each new refusal, so a checker that stopped refusing turns the harness red. Its existing mutations keep passing unchanged.
- A real deployment that publishes three distinct well-formed digests still qualifies. The checker gains refusals, not a new reason to reject a correct release.

Boundary: this ticket changes `scripts/check-production-qualification.py` and `tests/production-release-qualification.sh`. It does not change any product source under `crates/`, any response shape, any field name, or how any identity is computed. It does not change the item-shape comparison or the response-shape inventory that 0052 shipped, and it does not add a fourth published value for the checker to read.
