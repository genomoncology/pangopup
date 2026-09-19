---
flow: build
priority: 1
deps: ["0138"]
---
# Runtime-v2 preparation and semantic gates are executable

## Outcome

Maintainers can deterministically prepare an outer `runtime-grch38-v2` release bound to the qualified sparse SNV authority, and one executable cross-format harness proves every ADR 0027 semantic comparison before activation.

## What is true today

PangoPup authenticates the inactive v2 SNV authority and can derive an inner sparse-bound runtime profile after exhaustive bundle certification. Runtime transport pack, verify, and unpack already accept any internally consistent canonical profile. Outer runtime release preparation and production admission remain v1-only. Existing command and service tests prove many individual shapes, but no one harness runs both formats through the 1,000-case oracle, seven command batches, mixed routing, errors, and HTTP identities while limiting normalization to ADR 0027's five asset-derived fields.

## Done, observably

- A closed inactive v2 runtime authority accepts only an inner profile whose SNV descriptor matches the exact qualified v2 release and whose model, reference, mask, scoring policy, and source facts equal v1. It derives and pins a distinct runtime profile identity.
- Deterministic outer runtime release preparation can target `runtime-grch38-v2` without changing v1 bytes, v1 preparation, the ordinary production selector, sync, installation discovery, or trusted admission. It validates exact member inventory, URLs, checksums, target/tooling provenance, preferred source, and SNV-v2 binding. The eight model-side transport members remain byte-for-byte equal to v1; only the inner runtime profile and enclosing transport manifest change.
- Keep the original model converter commit, new clean compiled preparation-tool commit, and independently supplied release target distinct. Preserve v1's complete `model_source`, including converter `e6d8497aaf1e3db521360ad969252a2ec6fd14e4`. Generated authorities land after their target. Source-supplement metadata has an explicit verified inventory and makes no URL-only verification claim.
- Miniature preparation runs twice and produces identical runtime profile, transport manifest, release profile, checksums, notes, and source supplement metadata. V1 and v2 contracts remain version-specific and crossed authorities fail. Miniature authority injection remains test-only.
- One fixture-driven qualification harness runs fixed-v1 and sparse-v2 providers through all 1,000 regression requests and seven filtered/unfiltered command batches. It also covers overlap and filtered overlap, `REF=N`, pure miss, mixed found/miss/rejected order, invalid input, stable errors/reasons, and the precomputed HTTP shapes already named by ADR 0027.
- Keep the independent oracle projection separate because it predates gene names and software version. Cross-format comparison preserves both fields and permits replacement of only `snv_bundle_id`, `bundle_id`, `runtime_profile_id`, `data_set_version`, and `scoring_identity`. Derive and validate every observed identity from the actually admitted miniature profile, fixed software version, and fixed effective CPU policy before replacement. Every other command and service byte and key order matches. Negative cases reject unauthorized changes and incorrect allowed identities.
- Exercise real fixed and sparse `BundleOpen` providers. Keep fabricated serializer-shape coverage labeled separately from reachable service routing. Explicit misses remain `not_found`; discovery and complete fallback keep request-validation ordering, including six position-1 `MODEL_REJECTED` outcomes without partial command output. HTTP retains ordered result and rejection items, exact status and body, and makes no claim about variable transport headers.
- One executable gate coordinates the focused tests and fails unless it observes exactly 1,000 requests, seven nonempty groups, the named edge cases, real-provider command parity, reachable service routing, serializer shapes, and identity derivation. A zero-selected test fails.
- The harness uses miniature or retained fixtures only. It does not claim a public v2 service or run retained large assets. ADR 0027's complete candidate already passed its defined 1/10/100 gene-filtered latency gates; this ticket adds no post-result threshold and does not treat unfiltered whole-genome throughput as a promotion gate.
- Architecture, release documentation, compatibility guidance, and executable specifications describe the later retained run and activation sequence.
- Focused red-green tests, `make lint`, `make test`, and `make spec` pass.

## Boundary

Do not build the retained outer runtime-v2 bytes, publish assets, switch compiled authorities or URLs, install or activate v2 outside test roots, change output, delete v1, rerun the 2 GB sparse construction, invent an unmeasured throughput gate, or tag 0.5. The next ticket runs exact pushed tooling against retained assets, qualifies upgrade and rollback, then switches production authority only if every gate passes.
