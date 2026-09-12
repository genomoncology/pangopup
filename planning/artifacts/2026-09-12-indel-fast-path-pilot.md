# Indel fast-path pilot

Date: 2026-09-12

Related plan: [`2026-09-12-two-path-measurement-plan.md`](2026-09-12-two-path-measurement-plan.md)

## Result

An observed-indel catalogue has a real frequency head. The head is not large enough on its own to meet the target on independent traffic. A contract-specific loss-only route changes the answer because PangoPup can prove zero loss without inference across most genic requests.

The chromosome 22 pilot supports this strategy:

1. Keep exact score lookup as the first indel route.
2. Add the existing exact no-boundary rule for loss-only threshold requests.
3. Run the full model for unresolved gain requests and complete gain-and-loss requests.
4. Do not build a second classifier yet. First measure the full-genome gnomAD catalogue and score the residual misses. A classifier earns a place only if exact routes remain too slow.

The independent transfer result changed the recommendation. A same-callset split suggested 99.94 percent recurrence. Independent traffic reduced exact 1000 Genomes catalogue membership to 75.16 percent. The official gnomAD v4.1.1 genomes catalogue raised exact-tuple membership to 98.45 percent. The exact loss-zero rule raised potential loss-only fast resolution to 99.91 percent when combined with that catalogue.

These are routing measurements. Population catalogue membership does not equal a PangoPup fast hit. Every catalogue entry still needs an exact stored Pangolin result for the same submitted tuple.

## Primary public measurement

The primary source is the public 1000 Genomes 30× high-coverage phased callset on native GRCh38. The experiment used chromosome 22 from the 3,202-person callset and selected the canonical 2,504 unrelated Phase 3 people. A deterministic SHA-256 ordering within each of 26 populations assigned 2,002 people to training and 502 to held-out traffic. The split happened before catalogue construction. A homozygous carrier contributed one submitted variant request.

Source identities:

| Member | Bytes | SHA-256 |
|---|---:|---|
| `1kGP_high_coverage_Illumina.chr22.filtered.SNV_INDEL_SV_phased_panel.vcf.gz` | 445,701,977 | `91aeaf0d1e805cf4223c5cbf12e22afc28db1709054c42bd668337219f8fddcc` |
| `integrated_call_samples_v3.20130502.ALL.panel` | 55,156 | `b4023dc6ee2d62ee89c8d4d347db4d348e65518d66d346574cdae7a4bbd76858` |
| `gencode.v38.annotation.gtf.gz` | 46,556,621 | `22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050` |

The VCF came from the official EBI directory `data_collections/1000G_2504_high_coverage/working/20220422_3202_phased_SNV_INDEL_SV`. The GTF digest matches PangoPup's retained GENCODE v38 source identity.

The experiment preserved literal VCF tuples. It counted PangoPup's supported anchored insertions and deletions with allele lengths no greater than 100 bases. It reported current anchor membership and full deletion-span overlap separately. Current anchor membership classified 88,577 unique supported chromosome 22 indels as genic. Full-span overlap added five unique variants and 21 held-out person-level requests.

PangoPup's runtime mask contains 60,649 exact versioned GENCODE gene records. The familiar count near 19,000 describes protein-coding genes and does not describe the current route boundary. This experiment used every GENCODE v38 gene body because the shipped mask does the same.

### Catalogue recurrence

Held-out people carried 3,856,023 supported anchor-genic indel requests. The median is not retained in this result. The arithmetic mean is 7,681.3 requests per person on chromosome 22.

| Training catalogue size | Held-out requests covered | Hit rate |
|---:|---:|---:|
| 100 | 50,172 | 1.301% |
| 1,000 | 476,790 | 12.365% |
| 10,000 | 2,797,476 | 72.548% |
| 86,438, all training-observed genic entries | 3,853,862 | 99.944% |

Population-specific full-catalogue rates ranged from 99.884 percent to 99.985 percent. This narrow range does not prove transfer. Every population contributed people to both sides of one shared callset.

The queued follow-on completed chromosome 1 across 5,759,060 source records. It found 457,435 unique supported anchor-genic indels and 21,236,550 held-out person-level requests. The top 10,000 training entries covered 22.02 percent of requests, the top 100,000 covered 90.49 percent, and all 445,897 training entries covered 99.946 percent. Chromosome 1 confirms high same-callset full-catalogue recurrence. It also shows that a small fixed head covers much less traffic on a large chromosome than chromosome 22 suggested. The global ranked curve remains necessary.

### Length concentration

One-base events account for 50.41 percent of held-out genic requests. All indels through four changed bases account for 83.47 percent. All indels through ten changed bases account for 93.67 percent.

The bounded class used by the SpliceAI precompute design contains one-base insertions and deletions through four bases. That class accounts for 67.57 percent of held-out genic requests and 60.38 percent of the training catalogue entries. Exhaustive bounded enumeration spends far more entries for substantially less traffic coverage than observed-only precompute in this pilot.

## Independent transfer measurement

An authorized independent diagnostic-style whole-genome corpus supplied 20 family VCFs and 60 people. The public result retains no cohort name, file path, family name, sample identifier, or variant list. The aggregate passed a direct identifier scan.

The PASS subset contained 568,939 supported anchor-genic chromosome 22 requests. Per-person request counts ranged from 8,402 to 12,046 with a median of 9,244.

| Potential route | Requests covered | Rate |
|---|---:|---:|
| Exact tuple in the 1000 Genomes training catalogue | 427,604 | 75.158% |
| Exact tuple with positive allele count in gnomAD v4.1.1 genomes | 560,112 | 98.449% |
| Positive gnomAD exomes or genomes allele count | 560,383 | 98.496% |
| Conservative exact loss-zero rule | 545,561 | 95.891% |
| 1000 Genomes catalogue or exact loss-zero rule | 563,917 | 99.117% |
| gnomAD v4.1.1 genomes catalogue or exact loss-zero rule | 568,407 | 99.906% |
| gnomAD exomes/genomes membership or exact loss-zero rule | 568,448 | 99.914% |

The 1000 Genomes full-catalogue family hit rates ranged from 73.45 percent to 76.87 percent. Population membership alone does not prove score availability or a fast PangoPup route.

The official gnomAD v4.1.1 genomes chromosome 22 sites VCF contains 803,614 supported anchor-genic indel tuples with positive `AC`. Literal matching against that catalogue reproduced all 560,112 PASS request hits from the retained genome annotation. The file is 8,774,039,914 bytes. Its SHA-256 is `4c066b13f946c779e42083f2714605d4d185f7313f7519e6767bbd88e6f62678`. Its MD5 is `8018e6e02b9bf7ec97c3367928c7e105`, which matches the corrected checksum [published by the gnomAD team](https://discuss.gnomad.broadinstitute.org/t/v4-1-1-variant-md5sums-outdated/829). The project distributes VCF downloads without a gate and describes them as free to download in its [requester-pays notice](https://gnomad.broadinstitute.org/news/2020-07-requester-pays-notice/). The combined exome-or-genome annotation count remains directional because this run reproduced only the genome source. Transfer schema v1 names its catalogue-size field `training_catalogue_entries`; the value 803,614 in the official output counts positive-AC gnomAD entries and does not describe a training split.

The loss-zero count uses a conservative rule. A request qualifies only when the complete public GENCODE v38 GTF contains no exon start or end anywhere from `position - 50` through `position + 49 + REF length`. The full GTF supplies a boundary superset of the filtered runtime mask. Absence in the superset proves absence in every returned gene's runtime boundary list. `apply_mask` then makes loss exactly zero. This rule says nothing about gain.

A loss-only deployment does not need stored loss scores for catalogue entries that this rule already resolves. It can apply the exact-zero rule first and precompute only the boundary-near remainder. A gain or complete-score deployment still needs full scores across the catalogue. The full-genome run must count both storage scopes separately.

## Mac CPU measurement

The exact v0.5.0 runtime assets ran through the shipped ONNX CPU provider on an Apple M5 Max with 18 logical CPUs. The measurement used one HTTP request containing ten uncached, supported, one-gene chromosome 22 indels from the head of the training catalogue. Each configuration used a fresh private SQLite cache. The result payloads were biologically identical across configurations after removal of the declared CPU-policy identity.

| Workers × inference threads | Ten-request elapsed | Elapsed per request |
|---|---:|---:|
| 1 × 1 | 34.293 s | 3.429 s |
| 1 × 4 | 9.319 s | 0.932 s |
| 1 × 8 | 6.527 s | 0.653 s |
| 4 × 4 | 9.236 s | 0.924 s |
| 4 × 2 | 17.911 s | 1.791 s |

One worker with eight inference threads won this bounded batch. More workers did not improve throughput. This sample covers ten common, one-gene requests on one host. It does not measure rare residual misses, two-strand requests, the full allele-length distribution, or tail latency.

The same one-worker, eight-thread service then answered 2,000 sequential keep-alive HTTP requests from its completed SQLite score cache. A one-indel request measured 0.083 milliseconds p50, 0.117 milliseconds p95, and 0.088 milliseconds arithmetic mean. A ten-indel request measured 0.185 milliseconds p50, 0.203 milliseconds p95, and 0.185 milliseconds arithmetic mean. The batched mean was 0.019 milliseconds per indel. These figures include local HTTP and JSON handling. They show that the existing score cache already satisfies the fast-route latency target once the exact result exists.

The current binary compiles ONNX Runtime's CPU provider. It does not compile the CoreML provider. The shipped product therefore cannot use the Apple GPU and cannot silently fall back from it.

## Latency interpretation

The following projections use the measured 0.088-millisecond single-item completed-cache mean and the measured 0.653-second eight-thread miss cost. They explain the required coverage. They do not replace an end-to-end whole-genome run.

| Request contract and potential route | Fast coverage | Projected arithmetic mean |
|---|---:|---:|
| Gain or complete score, 1000 Genomes catalogue | 75.158% | 162.2 ms |
| Gain or complete score, gnomAD v4.1.1 genomes membership | 98.449% | 10.2 ms |
| Loss only, 1000 Genomes catalogue plus exact zero | 99.117% | 5.85 ms |
| Loss only, gnomAD v4.1.1 genomes membership plus exact zero | 99.906% | 0.70 ms |

The same gnomAD gain/complete projection is 53.3 milliseconds with the measured one-thread miss cost. A single model call remains hundreds of milliseconds even on this Mac. The average reaches the target only when exact routes absorb almost every request.

The measured Mac rates require 98.481 percent fast resolution to average 10 milliseconds, 99.247 percent to average 5 milliseconds, and 99.860 percent to average 1 millisecond. The chromosome 22 gnomAD v4.1.1 genomes rate of 98.449 percent misses the 10-millisecond requirement by 0.032 percentage points. In this 568,939-request aggregate, 186 more exact resolutions would close that arithmetic gap. A slow-route mean of 639 milliseconds would also close it without another fast rule. The full-autosome transfer and direct residual-miss timing can move the answer either way.

## Threshold scenarios

The experiment treats 0.1, 0.2, and 0.5 as caller-supplied performance scenarios. It does not treat them as clinical defaults.

The observed-catalogue hit rate does not change across these thresholds. An exact stored score can answer every threshold. A catalogue miss can answer none of them without another proven bound. The conservative loss rule also has the same 95.891 percent coverage at every positive threshold because it proves an exact public loss score of zero.

| Caller question | 0.1 | 0.2 | 0.5 |
|---|---:|---:|---:|
| Loss below threshold from the conservative exact-zero rule | 95.891% | 95.891% | 95.891% |
| Complete answer from a fully scored gnomAD v4.1.1 genomes catalogue | 98.449% | 98.449% | 98.449% |
| Loss answer from that catalogue or exact zero, potential | 99.906% | 99.906% | 99.906% |

Threshold choice can change coverage only when another fast route returns a score interval instead of an exact score or exact zero. PangoPup has no qualified indel interval today. Nearby SNV scores may predict one, but they cannot prove one. Measuring a threshold-specific SNV-neighborhood screen therefore requires model-labelled indels, a one-sided false-negative budget, and separate gain and loss results.

## Decisions

1. Use an observed-only scored catalogue as the next precompute candidate. Exhaustive short-indel enumeration covers less real traffic for much more computation. The accepted cost is incomplete gain coverage and an unavoidable model tail.
2. Treat loss-only, gain-only, and complete gain-and-loss answers as different contracts. The exact loss-zero route is valuable only for callers that accept a loss-only answer. The accepted cost is a clearer but larger API surface.
3. Defer a learned classifier. Exact gnomAD v4.1.1 membership comes close to the gain target and exact loss logic exceeds the loss target. The accepted cost is that gain and complete-score traffic may remain above 10 milliseconds until the residual miss set is measured.
4. Continue the full-autosome public measurement and time the independent residual misses. Chromosome 22 cannot make the implementation decision alone. The accepted cost is more measurement before feature work.
5. Keep public scores at hundredths during this performance work. The Zenodo SNV corpus contains exact hundredths. The model retains floating-point values internally, but three-decimal output would create route-dependent precision and has no established clinical calibration. The accepted cost is that thresholds such as 0.106 cannot be represented exactly.

Ian can overturn decisions 2, 3, and 5 without invalidating the retained counts. Decision 1 should change only if the full-genome recurrence curve reverses the chromosome 22 result.

## Remaining gates

- Finish all autosomes and calculate one global person-weighted catalogue curve. Per-chromosome curves cannot substitute for a globally ranked catalogue.
- Count and score the full catalogue for gain or complete answers. Count the smaller boundary-near remainder separately for loss-only answers. Measure bytes per scored indel record and build cost.
- Score a stratified sample of independent residual misses. Measure their real one-, four-, and eight-thread latency instead of applying the common-variant pilot cost.
- Measure gain-only and complete-score threshold resolution at caller-supplied 0.1, 0.2, and 0.5 scenarios. Population membership alone cannot answer a threshold.
- Run an end-to-end whole-genome waterfall with cold and warm cache states.

## Reproduction artifacts

The executable measurement sources are [`indel_recurrence_2026_09_12.py`](indel_recurrence_2026_09_12.py), [`aggregate_indel_recurrence_2026_09_12.py`](aggregate_indel_recurrence_2026_09_12.py), [`indel_catalogue_transfer_2026_09_12.py`](indel_catalogue_transfer_2026_09_12.py), and their adjacent `test_*.py` files. Sixteen focused tests cover genotype counting, supported shapes, deterministic splits, gene and deletion-span membership, catalogue leakage, distribution accounting, global cross-chromosome ranking, literal transfer, official sites-catalogue loading, gnomAD annotation interpretation, and the conservative exon-boundary rule.

The aggregate public chromosome 22 result identities are:

| Output | Bytes | SHA-256 |
|---|---:|---|
| `chr22-recurrence.json` | 144,393 | `d887b399634307571afea7cda8647e772aceccfb6f5253eed48ca26b649425f0` |
| `chr22-recurrence.tsv` | 855 | `cd85f905396230cd93255da2d292f4394ecd01ee5f6c1a2298bce3f14657c120` |
| `chr22-variants.tsv.gz` | 1,263,783 | `df2fa34775a4add518689e7808749cf52af434ff8082250c4ff111da94e5c9db` |
| `chr1-recurrence.json` | 148,875 | `d283824d8797cca661251c1c74c5cc5e04e9bd349446f751f7017ad2770ea414` |
| `chr1-recurrence.tsv` | 1,014 | `e578215ac451a1aedeaca578ae57c3b00ba4be8cf46e3fd9fdd96f3c0655437f` |
| `chr1-variants.tsv.gz` | 6,528,204 | `ba5659fe6e2ed96d2dab0f2d8692d5f73ba837b3e6fcf2d669ec66e21baff822` |
| independent aggregate `chr22-transfer.json` | 2,746 | `a965eaf233a5369a14fd4ba3b49314a7535f612a470efa4d97e0820d17683a1d` |
| official gnomAD v4.1.1 transfer aggregate | 2,744 | `2a9f2db2fe4d4b045219a3e952c8e6c8d96a81ed813ee88d725169175dd00c92` |

No sample identifiers enter a retained public artifact.
