# ADR 0027: Smaller SNV format has predeclared release gates

Status: accepted
Date: 2026-09-15

## Decision

A qualified smaller complete installed `scores.pgi` member is a dependency for
the 0.5 release. This decision supersedes ADR 0006 for selection of the next
SNV format. It does not change or reinterpret fixed-v1. The immutable fixed-v1
asset remains the shipped format and readable rollback.

The replacement optimization order is exact correctness and public parity,
then a complete installed `scores.pgi` member at or below 3 GiB, then warm
lookup latency within both predeclared bounds. Every item is a promotion gate. The earlier
speed-first order no longer selects the next format. Resident page behavior
may be recorded as supporting evidence. Compressed download size is not a
promotion gate.

The next format may be promoted only after one complete candidate passes every
gate below:

- Its complete installed `scores.pgi` member is at most 3 GiB, or
  3,221,225,472 bytes. This ceiling does not include `NOTICE` or
  `manifest.json`.
- The authoritative latency comparison uses the retained 134-gene corpus and
  `planning/artifacts/002-query-manifest.tsv`, release builds, 20 warmups, 20
  retained samples, nearest-rank p50, and the Ryzen 7 5825U Ubuntu 24.04
  reference host recorded in `planning/artifacts/002-index-format-benchmark.md`.
  The candidate and hardened fixed-v1 reader run side by side in the same
  invocation. The evidence records both readers and each candidate/fixed ratio
  for 1, 10, and 100 warm one-open exact lookups. Each candidate workload must
  stay within ten times its same-run fixed control and at or below its
  historical cap: 2,100 ns for 1 lookup, 19,640 ns for 10, and 195,880 ns for
  100.
- An exhaustive canonical-stream comparison covers all 1,366,418,555
  gene-loci and all three ordered alternates per locus. Gene, contig,
  coordinate, reference, alternate, gain and loss scores, relative positions,
  overlap order, and all 30 `REF=N` exception loci must match fixed-v1 and its
  recorded decoded logical digest exactly.
- Semantic parity is separate from the latency manifest. It runs both formats
  through `tests/fixtures/snv-regression/requests.tsv` and its independent
  1,000-result oracle, including its seven filtered and unfiltered command-line
  batches. Cross-format cases also exercise overlap and filtered overlap,
  `REF=N` ambiguity, a pure SNV miss, mixed found/miss/rejected order, invalid
  input, stable error and reason values, and the precomputed HTTP status shapes
  and identities covered by `spec/model-routing.md`, `spec/http-service.md`,
  and the existing service tests.
- Across those semantic cases, status, normalized variant fields, ordered
  records, gene identities and names, scores, positions, masking and window
  fields, ambiguity fields, misses, errors, and reasons must match. A
  comparison may replace only the asset-derived `snv_bundle_id`, `bundle_id`,
  `runtime_profile_id`, `data_set_version`, and `scoring_identity` values
  before requiring the remaining command-line and service results to be
  byte-identical. Separate assertions must prove that each replaced value
  changed to the correctly derived new identity. The unchanged source DOI and
  archive identity remain exact.
- Corruption evidence separately proves that bounded open rejects malformed
  metadata, directories, truncation, offsets, counts, and arithmetic overflow;
  lookup rejects a corrupt record when touched; bounded open does not scan an
  untouched ordinary payload; and complete offline verification detects
  corruption in an untouched payload record.
- The format has a distinct format identity, SNV asset profile, bundle
  identity, and runtime profile. Fixed-v1 stays readable for rollback. The
  active profile moves only after the new asset passes every gate.

The installed `scores.pgi` member-size ceiling and the bounded latency ceiling
are independent requirements.

## Consequences

The prior sparse-direct result motivates a candidate. It does not qualify one:
that candidate was not built over the complete source or hardened. Candidate
work must produce the complete size, side-by-side latency, exhaustive parity,
and corruption evidence before any asset or active profile changes.

The fixed-v1 writer, reader, asset, score precision, and public behavior remain
unchanged while qualification runs. A candidate failure leaves fixed-v1 active
and does not weaken any gate. The 3 GiB and ten-times limits may be changed
before candidate measurements begin. A change after results are observed
requires a new decision.
