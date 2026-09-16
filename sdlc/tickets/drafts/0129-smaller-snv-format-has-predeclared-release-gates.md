---
flow: build
priority: 5
---
# A smaller SNV format has predeclared release gates

The installed fixed-v1 score member is 15,033,158,255 bytes. A prior sparse-direct candidate was materially smaller and remained near fixed-reader latency, but it was never built over the complete source or hardened. Ian selected a qualified smaller SNV asset as a 0.5 requirement and accepts lookup latency up to ten times the current hardened fixed reader. Record that changed optimization order before building the candidate.

Done, observably:

- A new accepted ADR supersedes ADR 0006 for the next SNV format without changing or reinterpreting fixed-v1.
- The candidate can be promoted only if its complete installed `scores.pgi` member is at most 3 GiB (3,221,225,472 bytes). The ceiling does not include `NOTICE` or `manifest.json`.
- The authoritative comparison uses the retained 134-gene corpus and `002-query-manifest.tsv`, release builds, 20 warmups, 20 retained samples, nearest-rank p50, and the Ryzen 7 5825U Ubuntu 24.04 reference host described in `planning/artifacts/002-index-format-benchmark.md`. The candidate and hardened fixed-v1 reader run side by side in the same invocation. For 1, 10, and 100 warm one-open exact lookups, record both readers and each candidate/fixed ratio. Each candidate workload must stay within ten times its same-run fixed control and at or below the predeclared historical caps of 2,100, 19,640, and 195,880 ns.
- Promotion requires an exhaustive canonical-stream comparison over all 1,366,418,555 gene-loci and all three ordered alternates per locus. Gene, contig, coordinate, reference, alternate, gain/loss scores, relative positions, overlap order, and all 30 `REF=N` exception loci must match fixed-v1 and its recorded decoded logical digest exactly.
- Semantic parity is separate from the latency manifest. It runs both formats through `tests/fixtures/snv-regression/requests.tsv` and its independent 1,000-result oracle, including its seven filtered/unfiltered CLI batches. Cross-format cases must also exercise overlap and filtered overlap, `REF=N` ambiguity, a pure SNV miss, mixed found/miss/rejected order, invalid input, stable error and reason values, and the precomputed HTTP status shapes and identities covered by `spec/model-routing.md`, `spec/http-service.md`, and the existing service tests.
- Across those semantic cases, status, normalized variant fields, ordered records, gene identities and names, scores, positions, masking/window fields, ambiguity fields, misses, errors, and reasons must match. A comparison may replace only the asset-derived `bundle_id`, `snv_bundle_id`, `runtime_profile_id`, `data_set_version`, and `scoring_identity` values before requiring the remaining CLI and service results to be byte-identical. Separate assertions must prove each replaced value changed to the correctly derived new identity; unchanged source DOI and archive identity remain exact.
- Corruption evidence must separately prove bounded open rejects malformed metadata, directories, truncation, offsets, counts, and arithmetic overflow; lookup rejects a corrupt record when touched; bounded open does not scan an untouched ordinary payload; and complete offline verification detects corruption in an untouched payload record.
- The ADR requires a distinct format identity, SNV asset profile, bundle identity, and runtime profile. Fixed-v1 remains readable for rollback. The active profile moves only after the new asset passes every gate.
- The issue, architecture index, and `planning/frontier.md` point to the decision. The frontier must say fixed-v1 remains the shipped rollback format and the smaller qualified format is the next 0.5 dependency. They do not claim that the candidate has passed before measured evidence exists.

Boundary: this ticket records the decision and acceptance contract. It does not implement a writer or reader, generate or publish an asset, change active release profiles, change scores or precision, or make compressed download size a gate. Ian can overturn the 3 GiB or ten-times limits before candidate measurements begin; changing them after seeing results requires a new decision.
