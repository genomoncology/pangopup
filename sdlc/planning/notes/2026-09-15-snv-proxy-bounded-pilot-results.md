# SNV neighborhoods do not yet provide a safe indel fast path

Date: 2026-09-15

## Decision

Do not ship a two-path indel screen based on nearby published SNV scores. The measured ten-base flank lookup has a limited coverage ceiling. The labelled pilot contains no indel with a model gain or loss score at or above 0.10. It cannot measure the false-dismissal rate that a safe screen requires. Ian can overturn the experiment scope or the performance target. No clinical cutoff or product behavior changed.

The next useful experiment must deliberately include model-positive indels and independent quiet regions. It must count false dismissals separately for insertions, deletions, genes, and caller thresholds. A full-genome traffic study cannot replace that safety test. Do not expand this pilot just to collect more model-zero examples.

## What the lookup did

For each literal genic indel, the probe validated the reference and queried every gene returned by the mask. It read all three published SNV substitutions at the anchor and at positions ten bases to either side of the changed span. It kept gain and loss separate. A request qualified only when every gene had all expected SNV records. Missing, ambiguous, and other-gene records did not become zero. This lookup asked whether a local SNV maximum could screen an indel. It did not translate SNV scores into an indel score.

The input was a fixed, hash-ranked sample of 1,000 insertions and 1,000 deletions from a public chromosome 22 1000 Genomes aggregate. The aggregate supplied 110,478 held-out carrier occurrences. The balanced unique-allele sample and its carrier weights do not estimate the insertion/deletion mix or indel traffic in a patient genome. The sample contained no person identifiers. The published SNV bundle and runtime profile were pinned. The model panels used the same v0.5.0 installation.

## What the census found

| Condition for the combined gain/loss route with ten-base flanks | Distinct indels | Share of 2,000 |
| --- | ---: | ---: |
| Complete SNV records for every mask gene | 1,463 | 73.15% |
| Complete and every nearby SNV gain/loss score zero | 534 | 26.70% |
| Complete and every nearby SNV gain/loss score at most 0.05 | 1,168 | 58.40% |
| Complete and every nearby SNV gain/loss score at most 0.10 | 1,268 | 63.40% |
| Complete and every nearby SNV gain/loss score at most 0.20 | 1,358 | 67.90% |
| Complete and every nearby SNV gain/loss score at most 0.50 | 1,434 | 71.70% |

These are possible fast-path shares if the rules later prove safe. They are not achieved safe shares. Complete records covered 736 deletions and 727 insertions. Strict combined zeros covered 269 deletions and 265 insertions. Carrier-weighted completeness was 82,576/110,478 (74.74%). Carrier-weighted strict zeros were 28,525/110,478 (25.82%). Incomplete windows included 426 requests with missing records and 111 with records assigned to another gene. The categories did not overlap in this sample. No ambiguous records appeared.

## What full-model comparisons found

The pilot scored 88 distinct indels through the model. All returned `found`. The panels contained 40 hash-sampled observed indels, 12 observed indels chosen for high nearby SNV scores, 16 alternative indels at two complete all-zero SNV loci, and 20 observed indels near annotated exon boundaries. Fifteen model results had at least one nonzero gain or loss score. None reached 0.10. No panel can therefore estimate sensitivity or false dismissals at 0.10, 0.20, or 0.50. Zero observed misses at those thresholds has a zero-positive denominator.

All 16 alternatives at the two quiet loci also produced model zeros. That supports those exact alleles only. It does not establish an all-indel dead zone. In the high-SNV panel, nearby SNV maxima reached 0.89 but all 12 indel model results stayed below 0.10. A high nearby SNV score does not imply a high indel score in that panel. At `chr22:17991753:CTCA:C`, all three anchor SNVs scored zero, but the deletion model gain was 0.01. An anchor-only zero check cannot promise exact indel zero. The wider flank SNV window had gain maximum 0.52 at that request.

## The exact loss rule and the speed ceiling

The configured mask reports loss-of-annotated-site as exactly zero when its reporting window with 50-base flanks cannot include an annotated exon boundary. The conservative full-GENCODE check used the inclusive interval `[POS−50, POS+49+len(REF)]`. It found this potential partial answer for 1,915/2,000 sampled indels (95.75%). It covered 105,399/110,478 carrier occurrences (95.40%). The complete SNV-loss-zero rule added only 33/2,000 candidates (1.65%) beyond that exact boundary rule. Those extra candidates have no safety validation. The exact loss rule does not answer gain, a complete score record, or clinical concern. PangoPup has not shipped it as a pre-model route.

The local release probe read the 2,000 requests in 0.33 seconds on its first pass and 0.06 seconds on the next pass, including mask lookup and JSON output. That was about 0.165 and 0.030 milliseconds per request. Those numbers are local process measurements, not HTTP latency or cold-disk guarantees. A prior single-worker, eight-thread model measurement was about 653 milliseconds per indel under a different CPU policy. Average cost follows `fast lookup cost + slow-call fraction × model cost`. If that slow cost holds, even the 73.15% SNV-input coverage ceiling leaves about 175 milliseconds average model cost per sampled indel. Strict combined zeros leave about 479 milliseconds. A 10-millisecond average with a 0.030-millisecond fast path and 653-millisecond slow path needs about 98.47% safe fast resolutions. This SNV route alone falls far short even before safety is tested.

## Reproduction and limits

The retained inputs and generated outputs are under `data/pangopup/snv-proxy-pilot-2026-09-15/`. The public aggregate SHA-256 is `df2fa34775a4add518689e7808749cf52af434ff8082250c4ff111da94e5c9db`. The census input SHA-256 is `de289dcc5a70d9b94bd692b0eefed86a29e73d54d53ffdaf9a2faaa4f9c21900`. The probe output SHA-256 is `62ee8a3fef806b01fcf9beea363d558b9e15654118dd792a56b37071d7c96f9c`. The revised census analysis SHA-256 is `45d956aa90367ae35f295b978ac6f880a13bf076fd1f1baf3d17b71c51114540`. The model bundle ID is `4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`. The SNV bundle ID is `c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`. The runtime profile ID is `0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`.

The chromosome 22 aggregate, its anchor-genic selection, and the ten-base flank window do not represent all whole genomes. The model's published two-decimal output also limits an exact-zero comparison. These measurements establish a ceiling for this one SNV-window design. They do not reject other indel shortcuts, observed-indel precomputation, or a narrower loss-only API. They do reject treating nearby SNV zeros as a proven universal indel answer.

## Repository checks

All 26 focused Python fixture tests passed. The Rust format and Clippy portions of `make lint` passed. The dependency portion failed on `RUSTSEC-2026-0285` in the existing `rustls 0.23.42` lockfile entry. Workspace Rust tests passed within `make test`; the following portable shell check stopped on macOS `find`, which does not support its `-writable` option. `make spec` reported 186 passes and three failures: the container image block, an x86_64 expectation on this aarch64 Mac, and a BSD `sed` incompatibility in a repository-sourcing block. No gate is reported as fully green. These gate failures need their own dependency and portability decisions. They do not change the pilot counts or justify a publication concern.
