# Source-derived SNV regression fixture

This fixture contains exactly 1,000 deterministic requests selected from the six attributed Pangolin precomputed-score excerpts in `../pangolin-precompute/` under the contract in Ticket 006. The source is Pangolin precomputed scores by Nils Wagner and Aleksandr Neverov, Zenodo DOI <https://doi.org/10.5281/zenodo.15649338>, archive `Pangolin_hg38_snvs_masked.zip` (MD5 `679ef0b50e511b6102b4b88fbf811108`), CC BY 4.0.

`source/` is the closure of every source gene/locus needed by those requests, `reference.fa.gz` is a deterministic fixture-only reference, and `bundle/` is fixed-v1. `requests.tsv` defines original and seven-batch order. `expected.jsonl` and `expected/*.jsonl` come from this tool's direct strict TSV join and centi-score formatter; they do not call `BundleOpen`, `ScoreProvider`, or the CLI renderer.

Regenerate into an absent directory with:

```bash skip
cargo run --locked --package pangopup-build --bin pangopup-regression-fixture -- tests/fixtures/pangolin-precompute <ABSENT_OUTPUT>
```

Bundle identity: `sha256:5d90c69bb9220fbbbc2dd30fcf6cd0c1f88037d891fb79fe7e01df5d9f08c624`.
