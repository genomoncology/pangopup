---
---
# The model route record the release checker compares is still written by hand

Ticket 0046 made the seven precomputed SNV groups in `tests/production-release-qualification.sh` come from the built executable, so `scripts/check-production-qualification.py` finally meets a record the shipped renderer printed. The model route did not move. The harness stub still replays `tests/fixtures/executable-release/m09.jsonl` and `tests/fixtures/executable-release/model-only-snv.jsonl` byte for byte and splices a naming leaf onto them with `name-records.py`.

The consequence is the shape ticket 0046 was filed against, surviving on one route. Measured in this checkout on 2026-09-09:

- Added `model_drift_probe: bool` to `JsonModelRecord` in `crates/pangopup-cli/src/lib.rs:241` and to `JsonModelRecord::new`, rebuilt `pangopup-cli`.
- `bash tests/production-release-qualification.sh` printed `production release qualification tests passed`, exit 0. The renderer and `scripts/check-production-qualification.py` now disagree about what a model-route record carries, and no qualification gate said so.
- `cargo test --locked --package pangopup-cli` did fail, at `tests::modeled_jsonl_key_order_warnings_and_filtered_miss_are_byte_exact`. That test pins the model route's exact bytes with its own literal. A developer repairs the literal and the suite goes green, and the Python rule beside it never comes up.

A real release would then fail at qualification time on `model oracle mismatch: M09-insertion-short-plus`, because the oracles are pinned by digest and never move.

0046's approved design named this deferral and gave its reason: the two model oracles are the published model's answers for `ENSG00000010610`, and 0046's own boundary forbids reaching the published model. The repository's mini model fixtures cannot reproduce those scores. That reason holds for the scores. It does not hold for the record's shape.

The repository can render a model-route record offline today, from committed fixtures alone:

```
$ target/debug/pangopup lookup --bundle tests/fixtures/snv-regression/bundle \
    --variant GRCh38:chr1:5051:A:AC \
    --reference-bundle tests/fixtures/reference-route-test/bundle \
    --mask tests/fixtures/route-mask/domains.pgm \
    --model-bundle tests/fixtures/pangolin-model-kernel-mini/bundle --format jsonl
{"assembly":"GRCh38",...,"records":[{"gene":"ENSG00000000001.1","stable_gene":"ENSG00000000001","gain_score":"0.33","gain_position":-50,"loss_score":"0.00","loss_position":-50,"warnings":["no_annotated_sites"]}],...}
```

Different accession, different scores, and no `gene_names` key because `ENSG00000000001` is synthetic and the shipped index cannot name it. The key set is the same key set the oracle carries, minus the optional naming leaf. Comparing that key set against the model oracle's would catch a field added to, removed from or renamed in `JsonModelRecord`, without reaching the published model, without touching an oracle, and without moving a pinned digest.

The same argument covers the three HTTP score items. 0046's design review measured that `service.rs` builds every score item through the same `render_jsonl` the CLI uses, so a scored-record field change cannot reach the HTTP surface without also reaching the delegated SNV route. That makes the HTTP replay safe for `JsonRecord` and leaves `JsonModelRecord` exposed on both surfaces at once.

What a fix should establish: a field added to, removed from or renamed in what the tool prints for a model-route record fails `make test`, and the failure names `scripts/check-production-qualification.py` as the file that must change. Same sentence 0046 wrote for the precomputed route.
