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

Two further gaps sit in the same question and are settled here rather than separately.

A score item's provenance pins to a different depth depending on which route answered. In the captured v0.4.1 response, a precomputed item carries six provenance fields: `kind`, `bundle_id`, `source_doi`, `source_archive_md5`, `masked` and `window`. A modeled item carries twelve, adding `scoring_semantics`, `model_profile`, `effective_cpu_policy`, `reference_profile`, `reference_sequence_set_sha256`, `mask_bytes` and `mask_sha256`. So `scoring_semantics` and the mask identity are absent from a precomputed score entirely, and one response can carry two items that pin to different depths. A consumer reading status alongside the response has everything today. A clinical receipt is meant to stand on its own.

The runtime profile's scoring block carries `assembly`, `semantics`, `distance` of 50, `masking_policy` and `cpu_policy` (`crates/pangopup-build/src/runtime_profile.rs:121-127`). Every one of them can change a score. `assets` in the status response carries only the four bundle and mask digests, and `model` carries the effective CPU policy and queue arithmetic. Neither carries `distance`, `masking_policy` or `semantics`. A consumer pinning the four digests alone is not pinning those three. `runtime_profile_id` hashes the whole profile and is computed in the service but never published.

Publishing it is safe, and the code settles that rather than leaving it to design. `production_runtime_profile()` hardcodes `cpu_policy: "sequential:1/1"` (`crates/pangopup-assets/src/runtime_profile.rs:189`) and `require_trusted_production` accepts an installed profile only when it equals that constant exactly (line 237-243). The profile's CPU policy is therefore a property of the qualified assets and never of the running service. The live value is built separately from the deployment's thread option (`crates/pangopup-cli/src/service.rs:755-761`) and rendered `sequential:{threads}/1`. So `--model-workers` and `--model-threads` move `effective_cpu_policy` and cannot move `runtime_profile_id`.

That leaves a trap a consumer will meet. A service running `--model-threads 4` reports an effective policy of `sequential:4/1` while its runtime profile declares `sequential:1/1`. Both are true and they describe different things. A reader who finds only the profile value concludes the service runs single-threaded.

Ticket 0040 settles whether a CPU policy can change a modeled score. This ticket cannot be decided before that answer exists. If scores are identical across policies, the policy does not belong in an identity a consumer stores as a data-set version. If scores are not identical, the identity is correct as it stands and the documentation must say plainly that a deployment setting changes results.

Done, observably:

- The specification states which published values change when a score can change, and which change for reasons that cannot affect a score.
- A consumer has one published value, or one published set of values, to pin as its data-set version that does not move when only a deployment's worker or thread settings change.
- The guidance telling a consumer to store `scoring_identity` as its data-set version either continues to hold, or is replaced by guidance naming what to store instead. It does not stay as it is while being untrue.
- A test proves that the recommended value is stable across at least two CPU policies with assets held fixed, and that it changes when an asset changes.
- The recommended value covers every input the runtime profile's scoring block holds, `distance`, `masking_policy`, `semantics` and `assembly` among them. A change to any of them moves it. A consumer cannot pin a complete scoring configuration only to discover a fourth score-affecting input was never published.
- A precomputed score item and a modeled score item carry provenance to the same depth, or the specification states which facts a precomputed item does not carry and names where a consumer reads them instead. A reader of one response can tell what produced every item in it without a second request.
- The published documentation distinguishes the runtime profile's declared CPU policy from the deployment's effective one, and states which of the two a reader is looking at wherever either appears.
- Any change to what `scoring_identity` covers is named in the release's response-shape inventory and carries the consumer-first deployment order the existing gate requires.
- `make lint`, `make test` and `make spec` pass without reducing specification coverage.

Boundary: this ticket changes what a consumer is told to pin, and may change what one published identity covers. It must not change any score, position, rejection code, reason, request limit, or which genes a variant returns. It must not remove `scoring_identity` from the status response or from score items. It must not publish a release or move a published artifact pin.
