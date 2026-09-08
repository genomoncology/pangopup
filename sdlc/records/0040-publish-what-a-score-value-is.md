---
base: a1afe66
head: dbc6086
---

# Publish what a score value is

A consumer can now cite a document for what a PangoPup score value is. `README.md` states it beside the field list and `architecture/compatibility.md` carries a `## Score values` section. `spec/score-value.md` proves each statement against the code.

The value space is 101 discrete hundredths from `0.00` through `1.00`. Nothing lies between two of them. A value always renders with exactly two decimal places and a zero loss never renders `-0.00`. Rounding is ties-to-even, so 0.105 renders `0.10` and 0.115 renders `0.12` where half-up would render `0.11` and `0.12`. Both halves are exact in f32 and in f64, so the worked example holds in whichever dtype the record carries.

A threshold finer than one hundredth cannot be evaluated against a PangoPup score. The representable neighbours of 0.106 are `0.10` and `0.11`. No response distinguishes them, so a consumer holding such a threshold chooses one of the two and records which.

A modeled score does not change with a worker or thread setting. Four settings covering `sequential:1/1`, `sequential:4/1` and `sequential:8/1` produced 33 gene score records across 5 genes, 23 of them carrying a non-zero value. All six pairwise comparisons differ in zero items. Wall clock moved from 228 seconds to 114 seconds across those settings, so the thread count reached the runtime. `scoring_identity` moved three times across the same runs while the answers did not. `planning/artifacts/0040-score-value-determinism.md` records the run.

The published sentence names the strength of that evidence. The always-on test runs a 281-byte stand-in model holding only `MaxPool` and `Concat`. Neither operator sums, so its arithmetic cannot reorder across threads and the gate cannot establish the claim for the production model. Both documents state that the claim rests on a recorded measurement on one host against one build, and tell a reader to re-measure elsewhere. An `#[ignore]`d retained-assets test carries the production proof for a maintainer.

The effective CPU policy does not belong in the model cache key. `CacheIdentity::new` takes it as a component, so every deployment thread change discards every stored model row for a value the policy cannot change. Draft 0043 carries the repair, which needs a migration for existing rows.

The two scoring routes do not always report the same value. `GRCh38:chr10:114306065:A:T` returns gain `0.06` at position 12 from the published dataset and gain `0.02` at position 13 from the model on the shipped assets. One of the five records in the frozen comparison corpus disagrees. No rate is claimed.

Independent design and code reviews accepted the result after repairs. The design's determinism run scored 23 records of which only 7 carried a non-zero value. The design review chose variants by sweeping insertion positions and reran with 33 records of which 23 scored, reaching gains to 0.18 and losses to -0.11. It also found that every `cargo test` invocation in the specification passed whether or not its test existed: a filter matching nothing exits 0, and `cargo test --package pangopup-core --test score_value this_test_does_not_exist` proved it. The determinism proof sits behind a non-default feature that `make test` never builds, so that specification block was its only gate and the block could not fail. Each block now requires `1 passed; 0 failed`. The review also scoped a `scoring_identity` sentence that read as a guarantee the service cannot make, because the identity hashes `software_version` and an upgrade moves it and can move an answer.

The code review found one published claim false. Four sources stated that the published dataset reports position `-50` wherever its score is zero, resting on five records in the frozen corpus. Decoding the whole shipped index gives 8,198,511,150 score-and-position pairs, 7,651,541,764 of them zero, and 2,225,454 of those zero scores carrying another position. `pangopup lookup --variant GRCh38:chrX:100627272:C:A` answers `"gain_score":"0.00","gain_position":-49` from the published dataset. `decode_pair_code` packs an independent 7-bit position beside the 7-bit magnitude, so the format never forced the rule. The claim was removed from both documents, the evidence artifact and two test comments.

No production behavior changed. `crates/pangopup-assets/src/active_identity.rs`, `crates/pangopup-core/src/lib.rs` and `crates/pangopup-index/src/snv.rs` are byte-identical to the base commit, so `scoring_identity` and the published source fingerprints did not move.

`make lint`, `make test` and `make spec` passed with 539 tests passing, 9 ignored, and 298 specifications passing with 7 retained-asset specifications skipped by design. Specification coverage rose from 292.
