# Ticket 0040 — a modeled score under four deployment settings

Date: 2026-09-08

## Question

`ActiveScoringIdentityPreimage` folds the effective CPU policy into the scoring
identity, and `CacheIdentity::new` folds it into the model cache key. Both treat
a deployment's thread setting as something that can change an answer. Nothing
stated whether it can. This run answers that against the shipped production
assets.

## Method

The published runtime installation was copied to a scratch data root and the
service was pointed at the copy with `--data-dir`. The installation itself was
never written to. Each run started a fresh `pangopup serve` with its own empty
model cache, scored the same 18 variants in two requests, and stopped. A
separate cache per run means no run read another run's stored rows.

Two fields are excluded from the comparison because a CPU policy is allowed to
move them: `scoring_identity` and `provenance.effective_cpu_policy`. Everything
else a caller reads is compared, including every score, every position, every
status, every rejection reason and the whole rest of provenance.

## Host and build

- Commit `a1afe66a6caf2dedf6d4d1cbb92d230bbb1fbdb4`, PangoPup 0.5.0, release build.
- AMD Ryzen 7 5825U, 16 logical CPUs, Linux 6.17.0-35-generic x86_64.
- SNV bundle `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`.
- Model bundle `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`.
- Reference bundle `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`.
- Mask `sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.

## Runs

| `--model-workers` | `--model-threads` | `effective_cpu_policy` | `scoring_identity` |
| --- | --- | --- | --- |
| 1 | 1 | `sequential:1/1` | `sha256:bb3962cbd6d20305c48696ebffe89f0e82e494216ef83c5494f55849d908adf5` |
| 1 | 4 | `sequential:4/1` | `sha256:e2fad6d0472e72171189b3e324a0323886835c4136cb96e6fe1092a1a3ed9b82` |
| 4 | 1 | `sequential:1/1` | `sha256:bb3962cbd6d20305c48696ebffe89f0e82e494216ef83c5494f55849d908adf5` |
| 2 | 8 | `sequential:8/1` | `sha256:dbfe3c45815ab442caaab16a79204c18ed8eb3dd34d4f281f95225914f8510be` |

## Result

18 variants produced 23 gene score records across 5 genes. All six pairwise
comparisons of the four runs differ in **0** items. Every score, position,
status, reason and remaining provenance field is identical across three
distinct CPU policies and worker counts of 1, 2 and 4.

The worker count does not reach the policy at all. `--model-workers 1` and
`--model-workers 4` at one thread report the same `sequential:1/1` and the same
scoring identity. Only `--model-threads` moves either.

## The scored variants and their records

| variant | gene | gain | gain position | loss | loss position |
| --- | --- | --- | --- | --- | --- |
| `GRCh38:chr17:INS:7668400:7668401:A` | ENSG00000141510.18 | 0.00 | 0 | 0.00 | 21 |
| `GRCh38:chr17:INS:7669673:7669674:A` | ENSG00000141510.18 | 0.00 | 32 | -0.09 | 17 |
| `GRCh38:chr17:INS:7670946:7670947:A` | ENSG00000141510.18 | 0.00 | -5 | 0.00 | -50 |
| `GRCh38:chr17:INS:7672219:7672220:A` | ENSG00000141510.18 | 0.00 | 10 | 0.00 | -50 |
| `GRCh38:chr17:INS:7673492:7673493:A` | ENSG00000141510.18 | 0.00 | -14 | 0.00 | -50 |
| `GRCh38:chr17:INS:7674765:7674766:A` | ENSG00000141510.18 | 0.01 | 31 | 0.00 | -50 |
| `GRCh38:chr17:INS:7676038:7676039:A` | ENSG00000141510.18 | 0.00 | 26 | -0.11 | -44 |
| `GRCh38:chr17:INS:7677311:7677312:A` | ENSG00000141510.18 | 0.01 | 14 | 0.00 | -50 |
| `GRCh38:chr17:INS:7678584:7678585:A` | ENSG00000141510.18 | 0.00 | -1 | 0.00 | -50 |
| `GRCh38:chr17:INS:7679857:7679858:A` | ENSG00000141510.18 | 0.00 | 5 | 0.00 | -50 |
| `GRCh38:chr12:6801303:GG:AC` | ENSG00000010610.10 | 0.00 | 0 | 0.00 | -50 |
| `GRCh38:chr17:7687421:GCCC:ATTA` | ENSG00000141499.17 | 0.00 | 17 | 0.00 | -50 |
| `GRCh38:chr17:7687421:GCCC:ATTA` | ENSG00000141510.18 | 0.00 | -33 | -0.08 | -44 |
| `GRCh38:chr12:6801303:G:GA` | ENSG00000010610.10 | 0.00 | 0 | 0.00 | -50 |
| `GRCh38:chr17:7687421:G:GACG` | ENSG00000141499.17 | 0.00 | -35 | 0.00 | -49 |
| `GRCh38:chr17:7687421:G:GACG` | ENSG00000141510.18 | 0.00 | 0 | 0.00 | -50 |
| `GRCh38:chr3:29000000:T:TACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTAC` | ENSG00000283563.1 | 0.01 | 20 | 0.00 | -50 |
| `GRCh38:chr3:29000000:T:TACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTAC` | ENSG00000144642.22 | 0.01 | 20 | 0.00 | -50 |
| `GRCh38:chr12:6801303:GG:G` | ENSG00000010610.10 | 0.00 | -9 | 0.00 | -50 |
| `GRCh38:chr17:7687421:GCCC:G` | ENSG00000141499.17 | 0.00 | -35 | 0.00 | -50 |
| `GRCh38:chr17:7687421:GCCC:G` | ENSG00000141510.18 | 0.00 | -13 | 0.00 | -49 |
| `GRCh38:chr3:29000000:TTTTTTGCACCTAAATTTAGGATTATATTCAAATAGCAAATGCCTTGAAGTGCTCTGATACTGAGCTTCCCAGTTTTTGTTGAGCTAGTGACATATTTGT:T` | ENSG00000283563.1 | 0.00 | -34 | 0.00 | -50 |
| `GRCh38:chr3:29000000:TTTTTTGCACCTAAATTTAGGATTATATTCAAATAGCAAATGCCTTGAAGTGCTCTGATACTGAGCTTCCCAGTTTTTGTTGAGCTAGTGACATATTTGT:T` | ENSG00000144642.22 | 0.00 | -34 | 0.00 | -50 |

## The two routes on the same variant

The same installation answers a covered SNV from the published dataset and
everything else from the model. `pangopup lookup --model-only` scores a covered
SNV through the model instead, so both routes can be read for one variant.

| variant | route | gene | gain | gain position |
| --- | --- | --- | --- | --- |
| `GRCh38:chr12:6801301:G:A` | precomputed | ENSG00000010610 | 0.00 | -50 |
| `GRCh38:chr12:6801301:G:A` | model | ENSG00000010610.10 | 0.00 | 5 |
| `GRCh38:chr17:7686079:A:T` | precomputed | ENSG00000141499 | 0.21 | 18 |
| `GRCh38:chr17:7686079:A:T` | model | ENSG00000141499.17 | 0.21 | 18 |
| `GRCh38:chr13:113723021:C:G` | precomputed | ENSG00000185974 | 0.03 | 0 |
| `GRCh38:chr13:113723021:C:G` | model | ENSG00000185974.7 | 0.03 | 0 |
| `GRCh38:chr12:6786859:A:G` | precomputed | ENSG00000010610 | 0.05 | 50 |
| `GRCh38:chr12:6786859:A:G` | model | ENSG00000010610.10 | 0.05 | 50 |
| `GRCh38:chr10:114306065:A:T` | precomputed | ENSG00000169129 | 0.06 | 12 |
| `GRCh38:chr10:114306065:A:T` | model | ENSG00000169129.15 | 0.02 | 13 |

The last row is the one that matters. The two routes report different values for
the same variant. The frozen `pangolin-compat-v1` corpus already carried that
case as `M03-snv-afap1l2-precomputed`, holding the published record `0.06` at 12
beside the model's `0.02` at 13. Nothing published it.

A zero score also carries a different position on each route. The published
dataset reports `-50` wherever its score is zero. The model reports the position
of its own extremum even when that extremum rounds to `0.00`.

## Findings

1. A deployment's worker and thread settings change no modeled score, position,
   status or reason. The specification can state that as a property rather than
   a hope.
2. The effective CPU policy therefore cannot change a value. It is in the
   scoring identity and in the model cache key anyway. Ticket 0041 owns the
   identity. The cache key is a separate cost: every deployment thread change
   discards every stored model row.
3. The precomputed route and the model route can report different values for the
   same variant. A consumer must record which route answered.

## Reproducing

`retained_assets_score_identically_under_two_cpu_policies` in
`crates/pangopup-cli/tests/http_service_lifecycle.rs` repeats the comparison.
It is `#[ignore]`d and reads `PANGOPUP_RETAINED_DATA_DIR`.

```
PANGOPUP_RETAINED_DATA_DIR=<scratch copy of the installed runtime> \
  cargo test --locked --package pangopup-cli --test http_service_lifecycle \
  retained_production:: -- --ignored --nocapture
```

Run on 2026-09-08 against the scratch copy:

```
test retained_production::retained_assets_serve_all_routes_and_order_lookup_then_m09_model ... ok
test retained_production::retained_assets_score_identically_under_two_cpu_policies ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 100.44s
```

Eight variants reached the production model under `sequential:1/1` and
`sequential:4/1`, the batch carried a non-zero score, and the two runs returned
identical items.
