# A smaller SNV format has predeclared release gates

ADR 0027 makes a qualified smaller complete installed `scores.pgi` member a 0.5 dependency. It supersedes the speed-first optimization order in ADR 0004 and the next-format selection in ADR 0006 without changing fixed-v1. Fixed-v1 remains the shipped readable rollback, and no smaller candidate has passed qualification.

The decision requires the complete installed `scores.pgi` member to be at most 3,221,225,472 bytes. `NOTICE` and `manifest.json` do not count toward that ceiling. The same-run Ryzen reference-host comparison uses the retained 134-gene corpus and query manifest, release builds, 20 warmups, 20 retained nearest-rank p50 samples, and the independent ten-times and 2,100 / 19,640 / 195,880 ns latency ceilings.

Promotion also requires exhaustive equality across 1,366,418,555 gene-loci, every ordered alternate, the decoded logical digest, overlap order, and all 30 `REF=N` exceptions.

Semantic parity is separate from the latency manifest. Both formats run through `tests/fixtures/snv-regression/requests.tsv`, its independent 1,000-result oracle, and its seven filtered and unfiltered command-line batches. Cross-format cases cover overlap and filtered overlap, `REF=N` ambiguity, a pure SNV miss, mixed found/miss/rejected order, invalid input, stable errors and reasons, and the precomputed HTTP status shapes and identities held by `spec/model-routing.md`, `spec/http-service.md`, and the existing service tests. Command-line and service results remain byte-identical after replacing only `snv_bundle_id`, `bundle_id`, `runtime_profile_id`, `data_set_version`, and `scoring_identity`. Separate assertions must prove that each value changes to its correctly derived new identity. Source DOI and archive identity remain exact.

Bounded-open, touched-record, and complete offline corruption tests remain separate gates. A promoted format receives a distinct format identity, SNV asset profile, bundle identity, and runtime profile. The active profile cannot move until every gate passes.

The architecture index, retained benchmark, planning issue, and frontier now point to the decision. The planning issue distinguishes the exact 15,030,604,105-byte ordinary-locus payload from the certified shipped 15,033,158,255-byte `scores.pgi`; the smaller 1,706,199,888-byte sparse figure remains a complete-corpus calculation rather than a built member. The frontier date check now requires 2026-09-15.

`git diff --check` and `make lint` exited 0. The lint gate completed version consistency, formatting, Clippy, advisory, bans, license, and source checks. Its existing duplicate-dependency notices remained warnings.
