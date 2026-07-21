# Precomputed SNV Index

## Complete source characterization

The downloaded source contains 19,913 gzip-compressed, tab-separated files,
one per Ensembl gene. A complete streaming scan—not a sample—validated all
4,099,255,665 rows and 1,366,418,555 three-alternate loci.

| Property | Complete-corpus result |
|---|---:|
| Uncompressed TSV | 146,143,911,627 bytes |
| Downloaded ZIP archive | 12,988,141,317 bytes |
| Per-gene gzip files | 13,754,573,181 bytes |
| Gain score is zero | 87.858% of SNV records |
| Loss score is zero | 98.799% of SNV records |
| All six score/position pairs are default | 77.293% of loci |
| Exact default score/position pairs | 93.301% of pairs |
| Three alternate records are identical | 77.963% of loci |
| Score record equals the preceding locus's same-alt record | 80.609% |

Every ordinary locus has one `A`, `C`, `G`, or `T` reference and exactly the
other three bases as alternates. Scores are exact hundredths with gain in
0…1 and loss in −1…0. Relative positions are integers in −50…+50.
Source members use the GRCh38 primary-contig spellings `chr1`…`chr22`, `chrX`,
`chrY`, and `chrM`; the mitochondrial source rows are part of the corpus rather
than an unsupported auxiliary-contig exception.

There are two small exception families that the format must represent rather
than assume away:

- 30 loci in 12 source genes have reference `N`; nine omit alternate `A` and 21
  omit alternate `T` from their three published rows;
- two genes contain three coordinate gaps: two single-base omissions and one
  50,000-base omission.

All other files are contiguous. There are 10,073 ascending and 9,840 descending
files. A builder can therefore canonicalize nearly all source rows into compact
segments and place the rare `N` loci in an explicit exception section.

The exception section is part of source-fidelity verification, not an invitation
to treat `N` as a concrete SNV allele. Normal lookup returns a typed ambiguous
source-reference outcome for an affected source-gene record; it never guesses a
reference or remaps the incomplete published alternate set.

The durable evidence and exact counts live in
[`../planning/artifacts/2026-07-20-full-dataset-entropy.md`](../planning/artifacts/2026-07-20-full-dataset-entropy.md).

## Entropy result

A score record can be represented losslessly as gain magnitude, gain genomic
offset, loss magnitude, and loss genomic offset. The gain/loss field implies
the sign. Each of the four values has 101 possibilities and fits in seven bits,
but the data is far from uniformly distributed.

The empirical zero-order entropy of one complete score record is 1.848462 bits.
Modeling the reference and three alternate score records as separate symbol
streams gives 1,285,518,889 bytes. Modeling the complete three-alternate locus
as one symbol captures their correlation and lowers the result to:

```text
5.995913 bits per locus
1,024,115,911 bytes total
0.954 GiB total
```

This is the first-principles floor for a memoryless codec over the observed
locus symbols. It excludes small directories/provenance and real coding tables.
It is not an absolute compression bound: spatial/context models may exploit
neighbor correlation, while a practical random-access format pays block and
index overhead.

## Rejected fixed-width baseline

Three 28-bit score records plus a three-bit reference fit in 87 bits, or 11
bytes per locus. Over the complete corpus that is 15,030,604,105 bytes—9.3%
larger than the existing gzip files. It is simple and fast, but it discards the
dominant fact that most score/position pairs are the default `(0, -50)`.

Fixed 11-byte loci remain a useful speed baseline; they are not the preferred
compact representation.

## Leading practical representations

### Hierarchical sparse direct lookup — selected runtime baseline

A decompression-free structure can store:

1. two reference bits per ordinary locus, with 30 `N` loci in an exception table;
2. one bit per locus indicating any nondefault score/position pair;
3. a six-bit gain/loss-pair mask only for those 310,269,258 loci;
4. one 14-bit score/position value for each of 549,194,849 nondefault pairs;
5. rank checkpoints per block so lookup uses bounded popcounts, not a scan.

The payload calculation is 1,706,199,888 bytes (1.589 GiB), before small rank,
gene, segment, and provenance directories. It is directly queryable from mmap
and is only about 88 MB larger than the measured 4,096-locus Zstd result.

Query performance is the primary product objective, resident memory and pages
touched are second, and compressed download size is third. The direct sparse
layout is therefore the v1 runtime baseline. The implementation ticket still
benchmarks it against the simpler fixed-width layout and compressed blocks to
quantify the choice and catch a surprising result, but compressed blocks do not
become the default merely because they save installed bytes.

### Independently compressed sparse blocks

A second profile uses three-bit references plus six pair-presence bitmaps and
14-bit nondefault values, then compresses each block independently. Complete
corpus measurements with Zstd level 1, including an eight-byte block-offset
entry, are:

| Loci/block | Blocks | Total bytes | GiB | Mean compressed block |
|---:|---:|---:|---:|---:|
| 256 | 5,347,441 | 1,963,855,485 | 1.829 | 359 bytes |
| 4,096 | 343,739 | 1,617,984,690 | 1.507 | 4,699 bytes |
| 65,536 | 33,690 | 1,574,311,857 | 1.466 | 46,721 bytes |

At 4,096 loci, LZ4 produced 1,834,809,589 bytes (1.709 GiB). The 4,096-locus
Zstd layout is only 43.7 MB larger than 65,536-locus blocks while touching and
decompressing far less data on a random miss. It is the current compressed
candidate, not yet a frozen format. Sizes exclude the small gene/segment/source
directories and `REF=N` exception table, and use a raw-block fallback when
compression expands a block.

The first format ticket must benchmark at least the hierarchical direct layout,
4,096-ish Zstd and LZ4 blocks, and the fixed baseline. It should also test block
sizes around 1,024–4,096 because the measured mean compressed 4,096-locus block
is slightly larger than one 4 KiB page. A contrary format selection requires a
measured speed win or evidence that the direct design is operationally invalid;
file size alone is not enough.

## Query-oriented structure

The runtime must not open 19,913 source files. A deployment bundle contains one
or a few immutable index members, a manifest, and attribution. The format ticket
must compare a monolithic payload with per-contig members; mmap itself does not
decide that question.

The logical sections are:

```text
[fixed header and source identity]
[contig/accession aliases]
[gene directory]
[contiguous segment directory]
[optional source-gene overlap index]
[rank/block directory]
[score payload]
[N-reference exceptions]
[provenance metadata]
```

The gene directory is sorted by compact Ensembl identity and points to one or
more ascending genomic segments. Within a segment, position gives a direct locus
ordinal. A gene-filtered query binary-searches the gene and its few segments,
then performs a direct sparse lookup or opens one compressed block.

A query without a gene needs all matching records present in the pinned source
archive—not all genes in an unspecified current GENCODE release. Candidate
contig interval structures must be benchmarked and must document actual
worst-case behavior; a prefix-maximum array alone does not guarantee
`O(log n + k)` for arbitrary nested spans.

## Builder contract

`pangopup-build` receives explicit read-only source, reference-certification,
staging, and final bundle paths. It streams one gzip member at a time and keeps
memory proportional to one source gene plus compact directories.

For every file it validates:

- exact header and field parse;
- one complete gzip member ending at physical EOF (no concatenated member or
  trailing payload), with bounded decompressed reads of at most 128 bytes for
  the header and 256 bytes per row, including any line ending;
- filename as a valid Ensembl gene ID;
- one canonically spelled supported primary contig (`chr1`…`chr22`, `chrX`,
  `chrY`, or `chrM`), rejecting adapter spellings such as `1` and `chr01`;
- gain sign/range and loss sign/range on the exact 0.01 grid;
- genomic-coordinate offsets inside the declared ±50 window;
- one reference and exactly three distinct alternate rows per locus;
- source order, segment boundaries, gaps, and duplicate locus keys;
- deterministic canonical order independent of source direction.

Source REF checks within and across files establish internal consistency only.
A releasable artifact also compares every unique locus with one pinned,
build-qualified GRCh38 reference and embeds the versioned chromosome/accession
alias identity and mismatch result.

The complete build records source archive size/checksum, row/locus/gene counts,
format and builder versions, build command, output hashes and sizes, reference
identity, and CC BY attribution. Corpus counts, bit/byte offsets, offset products,
and validation arithmetic use `u64`; narrower section-local values are permitted
only after checked conversion against explicit format bounds. The 4,099,255,665
rows happen to fit `u32`, but the 8,198,511,330 score/position pairs do not.

Publication depends on the selected physical shape. A one-file bundle uses a new
staged inode and atomic rename after offline verification. A multi-member bundle
uses immutable content-addressed members bound by hash into one bundle identity,
then atomically publishes the verified manifest/pointer (or atomically renames a
complete staging directory on one filesystem). A reader never combines members
from different manifests.

## Release transport is not runtime encoding

The fast installed mmap file and the downloaded release asset solve different
problems. A GitHub release may carry a `.tar.zst` transport archive. The install
command downloads it to a temporary path, verifies its digest and manifest,
expands it once, verifies the installed members, then atomically publishes the
immutable bundle. Runtime lookup maps the expanded direct sparse member and
never decompresses a query block.

The measured 1.589 GiB direct payload is below GitHub's current requirement that
each release asset be under 2 GiB, even before transport compression. The exact
final artifact size remains a release gate because directories and provenance
add bytes. If a complete archive ever approaches the ceiling, split transport
assets by contig while retaining one manifest and one logical bundle. Do not
weaken or redesign the runtime format merely to fit a hosting limit.

## Reader and verification safety

Runtime open performs cheap checks without paging through the payload: magic,
supported version, declared file length, section bounds/order/alignment, checked
offset arithmetic, directory counts/order, and embedded bundle/source identity.

Lookup validates the exact block or record it touches, including reserved bits,
allele codes, local bounds, and decode completion. A separate offline `verify`
command streams every section and proves global ordering, rank/count invariants,
payload structure, and artifact hash. Open-time validation must not defeat
selective mmap access by scanning the multi-gigabyte payload.

Mapped files are immutable trusted deployment artifacts. Concurrent in-place
truncation or mutation can cause an operating-system `SIGBUS` that Rust cannot
turn into a typed error; it is outside the reader threat model. Deployment must
publish a new verified inode atomically and never modify an open member.

## Performance proof

Correctness and performance travel together. The proof includes:

- round trips from a tiny checked-in source fixture;
- exact source-row comparison, including signed loss restoration, textual-zero
  policy, ±50 offsets, negative-strand genes, gaps, overlaps, and `N` references;
- malformed-source and mutated-index tests for structural checks;
- repeated same-block, random-block, gene-filtered, and all-source-overlap queries;
- direct sparse, Zstd, LZ4, fixed-record, and Tabix comparisons;
- separate warm-cache and reproducible cold-I/O methods;
- p50/p95/p99 latency, throughput, allocations, bytes/pages touched, page faults,
  resident memory, output size, build throughput, and build peak memory.

The correctness fixture selects edge cases. Format selection uses a stratified
large corpus and complete-source size accounting; a tiny fixture cannot predict
compression or I/O behavior. Cold testing must use a documented dataset larger
than available memory or an isolated uncached device/read method—not merely the
first query after a build, whose pages are usually already hot.
