---
flow: build
priority: 4
deps: ["0040"]
---
# Keep a deployment setting out of the scoring identity

The scoring identity moves when a deployment changes a setting that produces no different score. `ActiveScoringIdentityPreimage` (`crates/pangopup-assets/src/active_identity.rs:12-17`) hashes the schema, the software version, the runtime profile identity and `effective_cpu_policy`. That last value renders as `sequential:1/1` and comes from the deployment's worker and thread settings, not from any asset.

`README.md:131` tells a consumer to store this identity as the data-set version where its system has one version field. A consumer that follows that instruction and then scales a service from one worker to four records a new data-set version against scores that did not change. Two hosts serving identical assets report different identities for identical answers.

`spec/http-service.md:40` already establishes the principle. `request_contract` reports limits that vary by deployment and deliberately does not enter `scoring_identity`. The CPU policy is the same kind of fact and is treated the opposite way.

The inputs that can actually change a score are already published. The captured v0.4.1 status carries `assets.snv_bundle_id`, `assets.model_bundle_id`, `assets.reference_bundle_id` and `assets.mask_sha256`. Those four move when a score can change and stay still when it cannot.

Ticket 0040 settles whether a CPU policy can change a modeled score. This ticket cannot be decided before that answer exists. If scores are identical across policies, the policy does not belong in an identity a consumer stores as a data-set version. If scores are not identical, the identity is correct as it stands and the documentation must say plainly that a deployment setting changes results.

Done, observably:

- The specification states which published values change when a score can change, and which change for reasons that cannot affect a score.
- A consumer has one published value, or one published set of values, to pin as its data-set version that does not move when only a deployment's worker or thread settings change.
- The guidance telling a consumer to store `scoring_identity` as its data-set version either continues to hold, or is replaced by guidance naming what to store instead. It does not stay as it is while being untrue.
- A test proves that the recommended value is stable across at least two CPU policies with assets held fixed, and that it changes when an asset changes.
- Any change to what `scoring_identity` covers is named in the release's response-shape inventory and carries the consumer-first deployment order the existing gate requires.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage.

Boundary: this ticket changes what a consumer is told to pin, and may change what one published identity covers. It must not change any score, position, rejection code, reason, request limit, or which genes a variant returns. It must not remove `scoring_identity` from the status response or from score items. It must not publish a release or move a published artifact pin.
