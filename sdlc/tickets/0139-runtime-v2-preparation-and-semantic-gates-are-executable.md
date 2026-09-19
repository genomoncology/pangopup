---
flow: build
priority: 1
deps: ["0138"]
---
# Runtime-v2 preparation and semantic gates are executable

## Outcome

Maintainers can deterministically prepare an outer `runtime-grch38-v2` release bound to the qualified sparse SNV authority, and one executable cross-format harness proves every ADR 0027 semantic comparison before activation.

## What is true today

PangoPup authenticates the inactive v2 SNV authority and can derive an inner sparse-bound runtime profile after exhaustive bundle certification. Outer runtime transport and release preparation remain hard-coded to v1. Existing command and service tests prove many individual shapes, but no one harness runs both complete formats through the 1,000-case oracle, seven command batches, mixed routing, errors, and HTTP identities while limiting normalization to ADR 0027's five asset-derived fields.

## Done, observably

- A closed inactive v2 runtime authority accepts only an inner profile whose SNV descriptor matches the exact qualified v2 release and whose model, reference, mask, scoring policy, and source facts equal v1. It derives and pins a distinct runtime profile identity.
- Deterministic runtime transport and release preparation can target `runtime-grch38-v2` without changing v1 bytes, v1 preparation, the ordinary production selector, sync, installation discovery, or trusted admission. It validates exact member inventory, URLs, checksums, target/tooling provenance, preferred source, and SNV-v2 binding.
- Miniature preparation runs twice and produces identical runtime profile, transport, release profile, checksums, notes, and source supplement metadata. V1 and v2 contracts remain version-specific and crossed authorities fail.
- One fixture-driven qualification harness runs fixed-v1 and sparse-v2 providers through all 1,000 regression requests and seven filtered/unfiltered command batches. It also covers overlap and filtered overlap, `REF=N`, pure miss, mixed found/miss/rejected order, invalid input, stable errors/reasons, and the precomputed HTTP shapes already named by ADR 0027.
- Comparison permits replacement of only `snv_bundle_id`, `bundle_id`, `runtime_profile_id`, `data_set_version`, and `scoring_identity`. Separate assertions derive each replacement from admitted inputs. Every other command and service byte matches, including source identity, gene names, scores, positions, ambiguity, masking, and order.
- The harness uses miniature or retained fixtures only. It does not claim a public v2 service or run retained large assets. ADR 0027's complete candidate already passed its defined 1/10/100 gene-filtered latency gates; this ticket adds no post-result threshold and does not treat unfiltered whole-genome throughput as a promotion gate.
- Architecture, release documentation, compatibility guidance, and executable specifications describe the later retained run and activation sequence.
- Focused red-green tests, `make lint`, `make test`, and `make spec` pass.

## Boundary

Do not build the retained outer runtime-v2 bytes, publish assets, switch compiled authorities or URLs, install or activate v2 outside test roots, change output, delete v1, rerun the 2 GB sparse construction, invent an unmeasured throughput gate, or tag 0.5. The next ticket runs exact pushed tooling against retained assets, qualifies upgrade and rollback, then switches production authority only if every gate passes.
