# Complete Pangolin precompute distribution and entropy

Date: 2026-07-20
Status: evidence for the first format ticket; no wire format selected

## Source

- directory:
  `/home/ian/workspace/data/pangolin-precompute/Pangolin_hg38_snvs_masked/`
- 19,913 per-gene `.tsv.gz` files
- Zenodo archive checksum: `md5:679ef0b50e511b6102b4b88fbf811108`
  (the local ZIP was verified to match on 2026-07-20)

The analysis streamed every decompressed line with the retained optimized Rust
[`entropy analyzer`](2026-07-20-entropy-analyzer/README.md). It parsed scores as
exact decimal hundredths, never as floats; grouped
each three-row locus; canonicalized alternate order; and merged per-worker
histograms. It did not write a multi-gigabyte derivative. The retained source,
lockfile, invocation, toolchain, and dependency versions reproduce the report.
A production version belongs in `pangopup-build` with a checked-in fixture.

## Complete shape

| Measure | Result |
|---|---:|
| Files | 19,913 |
| SNV rows | 4,099,255,665 |
| Three-alternate loci | 1,366,418,555 |
| Raw TSV bytes | 146,143,911,627 |
| Downloaded ZIP archive | 12,988,141,317 |
| Source gzip bytes | 13,754,573,181 |
| Ascending files | 10,073 |
| Descending files | 9,840 |
| Median loci/gene | 27,395 |
| p95 loci/gene | 265,887 |
| Maximum loci/gene | 2,473,538 |

Reference locus counts:

| A | C | G | T | N |
|---:|---:|---:|---:|---:|
| 396,378,342 | 286,074,869 | 286,408,985 | 397,556,329 | 30 |

The 30 `N` loci occur in 12 gene files. Their published three-alternate sets
omit `A` at nine loci and `T` at 21 loci. They need an explicit exception path;
they cannot use the ordinary “reference implies the other three bases” rule.

Only two files have coordinate gaps:

- `ENSG00000010610`: two omitted single bases, between 6,801,301/6,801,303 and
  6,801,537/6,801,539;
- `ENSG00000185974`: one 50,000-base omission, between
  113,673,020/113,723,021.

The builder should represent five contiguous segments for these gaps rather
than store a position for every locus.

## Sparsity and correlation

| Observation | Count | Fraction |
|---|---:|---:|
| Gain score is zero | 3,601,508,144 records | 87.858% |
| Loss score is zero | 4,050,033,791 records | 98.799% |
| Both score magnitudes are zero | 3,573,060,040 records | 87.164% |
| Exact default full record | 3,572,234,833 records | 87.143% |
| Exact default `(score=0, position=-50)` pair | 7,649,316,481 pairs | 93.301% |
| Zero score with a nondefault position | 2,225,454 pairs | 0.027% of pairs |
| All six pairs exact-default | 1,056,149,297 loci | 77.293% |
| Three alternate records identical | 1,065,300,477 loci | 77.963% |
| Same-alt record equals preceding locus | 3,304,322,749 comparisons | 80.609% |

The count of loci by number of nondefault gain/loss pairs is:

| Nondefault pairs | Loci |
|---:|---:|
| 0 | 1,056,149,297 |
| 1 | 159,376,048 |
| 2 | 79,760,656 |
| 3 | 62,335,184 |
| 4 | 3,774,326 |
| 5 | 1,943,631 |
| 6 | 3,079,413 |

There are 4,799,323 distinct complete score records and 27,908,628 distinct
three-alternate locus patterns. The most common complete record alone accounts
for 87.143% of rows; the top 256 cover 95.947%, and the top 4,096 cover 99.359%.
This makes a bounded dictionary plus escape stream plausible, but its table,
random-access decode, and long tail need comparison with the simpler sparse
layout.

## Empirical entropy

All values below are empirical zero-order Shannon entropy over the complete
corpus.

| Symbol/model | Bits per symbol |
|---|---:|
| Reference base | 1.980969 per locus |
| Gain score | 0.857117 per SNV |
| Loss magnitude | 0.142385 per SNV |
| Gain genomic offset | 1.311157 per SNV |
| Loss genomic offset | 0.175843 per SNV |
| Joint gain score/offset | 1.642482 per SNV |
| Joint loss magnitude/offset | 0.223017 per SNV |
| Complete four-field score record | 1.848462 per SNV |
| Complete reference + three-alternate locus | 5.995913 per locus |

The separate reference/record model totals 1,285,518,889 bytes. The more useful
joint-locus model captures cross-alternate correlation and totals
1,024,115,911 bytes (0.954 GiB, 7.446% of source gzip).

This is a memoryless empirical floor, not an absolute bound. It omits small
directories and coding tables. Spatial/run context may reduce it; block indexes
and selective decoding add overhead.

## Measured representation sizes

| Representation | Bytes | GiB | Extracted gzip members |
|---|---:|---:|---:|
| Fixed 11-byte locus | 15,030,604,105 | 13.998 | 109.277% |
| Hierarchical sparse direct payload (calculated) | 1,706,199,888 | 1.589 | 12.404% |
| Six-bitmap sparse raw, 4,096-locus blocks | 2,498,441,878 | 2.327 | 18.164% |
| Six-bitmap sparse + Zstd-1, 256 loci | 1,963,855,485 | 1.829 | 14.278% |
| Six-bitmap sparse + Zstd-1, 4,096 loci | 1,617,984,690 | 1.507 | 11.763% |
| Six-bitmap sparse + Zstd-1, 65,536 loci | 1,574,311,857 | 1.466 | 11.446% |
| Six-bitmap sparse + LZ4, 4,096 loci | 1,834,809,589 | 1.709 | 13.340% |
| Joint-locus zero-order entropy | 1,024,115,911 | 0.954 | 7.446% |

Compressed totals include one eight-byte offset per block and raw fallback when
a compressed block would expand. They exclude small format, gene, segment,
`REF=N` exception, and source metadata. Blocks reset at gene boundaries.

The 4,096-locus Zstd corpus has 343,739 blocks. Its mean compressed block is
4,699 bytes and its mean decoded sparse block is 7,268 bytes. The 65,536-locus
choice saves only 43.7 MB while increasing mean compressed data touched to
46,721 bytes. The 256-locus choice increases size by 345.9 MB and has 42.8 MB
of block-offset entries alone.

## Design conclusion

The fixed 11-byte record should not be the primary format. Two candidates sit
on the useful frontier:

1. hierarchical sparse direct lookup at about 1.59 GiB plus small indexes;
2. independently compressed blocks at about 1.51 GiB with Zstd or 1.71 GiB with
   LZ4, plus small indexes.

The direct design is especially compelling: approximately 88 MB buys removal of
decompression from every lookup. Its exact implementation needs a locus bitmap,
compact six-bit masks, rank checkpoints, pair-value planes, two-bit references,
and an `N` exception section. The next format ticket should benchmark that path
against block compression around 1,024–4,096 loci, not assume that maximum
compression means maximum speed.

The remaining gap to the 0.954 GiB joint entropy result is real but not free.
Closing it likely requires a trained static dictionary/FSE/rANS-style codec or a
context model over three-alternate locus patterns. With 27.9 million distinct
locus patterns, a naive global dictionary is not attractive. Such a codec is a
later experiment unless the direct sparse result misses the deployment target.
