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

## Selected fixed-width v1

Three 28-bit score records plus a three-bit reference fit in 87 bits, or 11
bytes per locus. Over the complete corpus that is 15,030,604,105 bytes (about
14.0 GiB) before directories and the exception section—9.3% larger than the existing gzip
files. It deliberately discards the dominant default-pair sparsity.

That corpus size is arithmetic. It is the 1,366,418,555 gene loci the full
build counted, multiplied by 11 bytes. The build measured a 15,030,603,775-byte
payload, and the 330-byte difference is the 30 `REF=N` loci the format holds in
an exception section rather than in the fixed-width payload.
[`planning/artifacts/003-full-index-build.md`](../planning/artifacts/003-full-index-build.md)
records the count and the payload.

Ticket 002 measured that trade-off rather than rejecting it from size alone.
After adversarial review, the direct kernel was corrected to use zero-copy mmap
reads, packed masks, and rank-checkpoint/popcount lookup. On the equal candidate
harness, fixed still won the 1 / 10 / 100 primary warm workloads at p50 121 /
972 / 9,949 ns versus direct at 160 / 1,243 / 14,749 ns. The separately hardened
product fixed reader measured 210 / 1,964 / 19,588 ns. ADR 0004 puts query
performance first, so fixed 11-byte remains the private v1 format.

## Leading practical representations

### Hierarchical sparse direct lookup — rejected v1 alternative

A decompression-free structure can store:

1. two reference bits per ordinary locus, with 30 `N` loci in an exception table;
2. one bit per locus indicating any nondefault score/position pair;
3. a six-bit gain/loss-pair mask only for those 310,269,258 loci;
4. one 14-bit score/position value for each of 549,194,849 nondefault pairs;
5. rank checkpoints every 64 loci inside 4,096-locus blocks so lookup uses
   bounded popcounts directly over mapped bytes, not a scan or payload copy.

The payload calculation is 1,706,199,888 bytes (1.589 GiB), before small rank,
gene, segment, and provenance directories. It is directly queryable from mmap
and is only about 88 MB larger than the measured 4,096-locus Zstd result.

Query performance is the primary product objective, resident memory and pages
touched are second, and compressed download size is third. Hierarchical direct
won size and mapped-page work but lost the measured query priority to fixed v1.
Its comparison implementation has been removed; the retained report preserves
the result, and it is not a supported runtime format.

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
decompressing far less data on a random miss. It was a historical measured
candidate and is not a supported runtime format. Sizes exclude the small gene/segment/source
directories and `REF=N` exception table, and use a raw-block fallback when
compression expands a block.

Ticket 002 compared hierarchical direct, fixed 11-byte, Zstd and LZ4 at 1,024,
2,048, and 4,096 loci, plus an in-process Tabix baseline. Fixed supplied the
required measured speed win. The one-time candidate implementations and
benchmark target have since been removed. See ADR 0006 and the retained
benchmark report.

## Query-oriented structure

The runtime does not open 19,913 source files. The shipped deployment bundle
contains one immutable fixed-v1 index member, a manifest, and attribution.
Historical candidates included per-contig members, but private v1 is the
certified monolithic 11-byte representation.

The logical sections are:

```text
[fixed header and source identity]
[contig/accession aliases]
[gene directory]
[contiguous segment directory]
[optional source-gene overlap index]
[balanced per-contig interval tree]
[score payload]
[N-reference exceptions]
[provenance metadata]
```

The segment directory is sorted by `(gene, contig, start)`. Within a segment,
position gives a direct locus ordinal. A gene-filtered query performs an upper
bound search on `(gene, contig, position)` and checks the sole possible
predecessor segment, then directly decodes one fixed 11-byte locus record. It
does not scan all segments belonging to a gene.

A query without a gene needs all matching records present in the pinned source
archive—not all genes in an unspecified current GENCODE release. The shipped
reader uses the measured and adversarially tested balanced per-contig interval
tree, including nested and disjoint interval cases, for `O(log S + K)` lookup.
Candidate interval-structure benchmarking is historical; a prefix-maximum
array was not selected because it does not guarantee that bound for arbitrary
nested spans.

The shipped reader implements this through one long-lived `BundleOpen` provider.
Its manifest, computed bundle identity, frozen provenance, and mmap reader are
private after successful construction; offline verification and measurement use
read-only accessors. Open rejects manifest metadata above 1 MiB before buffer
allocation and also bounds the subsequent read. It first decodes only the
schema and index-format discriminator, so a future version with unknown fields
is typed as incompatible. Supported v1 then uses the strict closed decoder and
canonical-validates the manifest, exact member set and sizes, mmap
header/sections, every segment and interval node, and every exception. It does
not hash members or deliberately touch ordinary payload. A filtered lookup is
`O(log S + log E)` plus one constant-width decode; unfiltered enumeration is
`O(log S + K)`. Public sorting adds `O(K log K + A log A)` and owned result
allocation is `O(K + A)`. Every addressed ordinary record validates all six
score/position pairs, even when the caller selects only one alternate.

At a stored `REF=N` coordinate, any syntactically valid concrete REF/ALT query
returns the same gene-specific ambiguity and never returns the exception's
scores. Ordinary and exception records from overlapping genes can coexist in
one sorted result. A concrete REF mismatch against ordinary payload is simply a
miss on the lookup-only route; that route does not consult the separately
shipped reference provider.

## Retained GENCODE mask selection

Ticket 012 defined a separate, non-production `PGMBEN01` family for comparing
three exact mask layouts: a direct interval-tree baseline,
constant-membership domains, and binned postings. All three expose the same
caller-owned query buffer and return containing genes separated by strand in
authenticated upstream order, with exact versioned/PAR identity, source span,
query rank, and normalized exon boundaries. Membership follows Pangolin's
effective `(start,end]` rule rather than a conventional closed interval.

The comparison readers mapped one immutable member and validated bounded header,
section, and contig-directory metadata. Point queries decode only their indexed
regions; deterministic logical 4,096-byte traces record the actual decoder
pages. Exhaustive offline inspection decodes the complete logical stream,
reconstructs the canonical encoding, and owns payload-wide corruption checks.

The authenticated complete GENCODE v38 run selected constant-membership
domains at the first p95 speed step: headline p50/p95 were 171/331 ns, versus
241/401 for interval-tree and 241/431 for binned postings. Exhaustive
domain/edge comparison, corruption controls, and warmed zero-allocation checks
passed for every candidate.

ADR 0013 promotes the exact selected `PGMBEN01` v1 domains bytes behind the
production `pangopup_index::mask` API rather than rebuilding identical data
under a second magic. Runtime open accepts only the domains discriminator,
performs bounded structural checks, and leaves whole-member identity to later
download/install checksum verification. The one-time alternate readers,
candidate writer, tracing, and exhaustive inspection source have been removed;
their report, manifest, and historical commits retain the decision evidence.

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

The shipped builder exposes that contract through:

```text
pangopup-build build --source <DIR> --reference <FASTA_OR_GZIP> --output <BUNDLE>
pangopup-build verify <BUNDLE>
```

Production writing is separate from the bounded prototype API. It canonicalizes
one complete gene, immediately writes fixed 11-byte records to a disk spool,
and retains only segment/tree/exception directories in heap. The normalized
25-accession reference is also disk-backed. Once counts and offsets are final,
the writer emits the unchanged 320-byte fixed-v1 header and compact directories,
streams the payload spool, and appends the exception section.

The installed bundle contains exactly `NOTICE`, `manifest.json`, and
`scores.pgi`. The index member has no provenance extension: the closed RFC 8785
manifest binds the byte-exact notice, fixed-v1 member, pinned source archive,
observed extracted members, supplied reference bytes, canonical primary
sequence set, counts, and independent logical-stream identities. Its exact
SHA-256 is the bundle identity.

The complete build records source archive size/checksum, row/locus/gene counts,
format and builder versions, build command, output hashes and sizes, reference
identity, and CC BY attribution. Corpus counts, bit/byte offsets, offset products,
and validation arithmetic use `u64`; narrower section-local values are permitted
only after checked conversion against explicit format bounds. The 4,099,255,665
rows happen to fit `u32`, but the 8,198,511,330 score/position pairs do not.

Publication stages the three-file bundle on the destination filesystem, syncs
every member and the staging directory, completes offline verification, then
renames the directory atomically and syncs its parent on supported platforms.
An existing identical fully verified bundle is reused; an invalid or different
destination is left untouched. A reader never combines members from different
manifests.

On Linux, publication uses `renameat2(RENAME_NOREPLACE)` and directory `fsync`,
so a racing destination cannot be overwritten and a successful return includes
the strongest available directory durability. Other targets return a typed
unsupported publication failure after verification and explicit staging
cleanup; they never use an existence preflight followed by a racy rename.
Release publication remains Linux-qualified until a target-specific atomic
no-replace directory primitive is implemented.

## Builder provenance is artifact-specific

The manifest's `builder.source_sha256` describes the code and locked dependency
evidence used to construct that artifact. It is not a runtime compatibility
key. Future fixed-v1 SNV builds use
`pangopup.snv-builder-source.v1`; future production-reference builds use
`pangopup.reference-builder-source.v2`.

Each source digest covers a checked inventory declaration and artifact-local
source/dependency bytes compiled into the builder. Logical paths are sorted and
length-framed, and duplicate paths fail. Shared causal vocabulary and the
fingerprint algorithm intentionally affect both families. Production mask,
sync, release, CLI, model, and HTTP code affect neither.

Reference v2 additionally excludes mmap reader/query and post-build
certification code. It binds the shared wire codecs, writer, byte-producing
adapter, causal dependencies/wiring, and a checked behavioral projection of
all 25 compiled `Grch38Contig` codes and names instead of hashing the mixed
core source file. SNV v1 retains its existing whole-core behavior.

Existing v1 manifests with the former repository-wide source fingerprint
remain readable under the unchanged schema and integrity rules. Their members
are not rebuilt merely to change descriptive provenance. See
[ADR 0012](decisions/0012-artifact-specific-builder-provenance.md) and
[ADR 0022](decisions/0022-reference-reader-provenance-boundary.md).

## Release transport is not runtime encoding

The fast installed mmap file and the downloaded release asset solve different
problems. The shipped local lookup transport contains a canonical transport
manifest, exact copies of the installed `manifest.json` and `NOTICE`, and one
deterministic compressed `scores.pgi` stream cut into ordered exact
1,000,000,000-byte parts (except the nonempty final part). `transport verify`
checks metadata, parts, the complete compressed stream, and reconstructed score
identity without materializing it. `transport unpack` additionally performs
exhaustive fixed-v1 certification before atomically publishing the unchanged
three-file bundle. The native installer instead verifies the same transport and
reconstructed score identity in one write stream, publishes a canonical
receipt and active profile, then performs only cheap open-time validation.
Runtime lookup maps either that active expanded member or an explicit override
and never decompresses a query block.

The certified complete `scores.pgi` is 15,033,158,255 bytes. The exact GNU tar
1.35 + Zstandard 1.5.5 level-9 single-thread transport is 1,935,000,209 bytes
(`sha256:3e87d80fdad963ca6ffca646393b8bb3955214b77cd8b7f1782e48d039aba751`).
That historical experiment established the approximate compression scale; tar
is not the target lookup format. Its roughly 1.80 GiB result is also too close
to GitHub's under-2-GiB per-file ceiling for comfortable release headroom. The
accepted score-stream parts reassemble the same installed fixed-v1 member;
transport constraints do not add decompression to lookup.

## Reader and verification safety

Runtime open performs cheap checks without paging through the payload: magic,
supported version, declared file length, ordered non-overlapping section bounds
(fixed-v1 internal padding remains compatible while terminal section and payload
tails must be fully claimed), checked offset arithmetic, directory counts/order,
plus the external bundle manifest's member names, sizes, format, and provenance
shape. It does not claim checksum verification.

Lookup validates the exact block or record it touches, including reserved bits,
allele codes, local bounds, and decode completion. A separate offline `verify`
command hashes both non-manifest members, checks the embedded notice, requires
the production writer's exact section/payload contiguity and maximal segment
boundaries, streams every index section and record, reconstructs ordinary index
segments plus source segment/gap/exception totals, and requires the decoded
logical record count/hash to equal the independently computed source-side
identity. Ascending/descending member counts are source-only provenance: the
verifier uses checked arithmetic to prove that their sum equals the reconstructed
gene count, but fixed-v1's canonical ascending representation cannot recover and
independently verify that split. Open-time validation must not defeat selective
mmap access by scanning the multi-gigabyte payload.

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
- separate warm-cache measurements and a cold-I/O result only when a
  reproducible nonresidency method exists;
- p50/p95/p99 latency, throughput, allocations, bytes/pages touched, page faults,
  resident memory, output size, build throughput, and build peak memory.

The production spooler's regression is a separate single-test process with a
tracking global allocator and, on Linux, `/proc/self/statm` RSS sampling. It
pushes 3,000 genes / 3,000,000 loci before finalization and compares retained
and peak allocator state plus RSS growth with the 33,000,000-byte disk spool;
this detects retaining logical loci or an artifact-sized heap, rather than
merely asserting that a scratch file grew.

## Sparse-direct v1 candidate codec

`pangopup.sparse-direct.v1` is a candidate byte format. `PGSPRS01` and little-endian version `1` identify it. The candidate modules provide a writer and bounded mmap reader for qualification. The format has no bundle, manifest, asset profile, installed format, or runtime route.

The writer accepts complete genes in increasing numeric Ensembl order. Each gene's loci must increase strictly by contig code and coordinate. It validates ordinary and `REF=N` alternate sets and the fixed-v1 `-50..=50` relative-position range before writing that gene. Any rejected submission or spool write failure poisons the writer. A poisoned writer cannot publish its accepted prefix.

The writer encodes one bounded 4,096-locus block at a time into a create-new disk spool. A second create-new spool holds fixed-width `REF=N` entries. Heap state retains gene, segment, and block directories. It does not retain score payload. Final assembly streams the held spool descriptors into a create-new sibling staging file and syncs it. The writer then verifies and removes both exact writer-owned scratch inodes before publication. A cleanup failure prevents publication. Drop retries cleanup after failures. Successful cleanup disarms both scratch paths. The writer publishes the staged inode with an atomic no-replace hard link and syncs the output parent. A bare relative output uses `.` as its parent. Existing output and replaced scratch paths stay unchanged.

The 256-byte header has these fields:

- bytes 0..8 hold `PGSPRS01`; bytes 8..12 hold version `1`; bytes 12..16 hold header length `256`; bytes 16..24 hold exact file length;
- bytes 24..104 hold five `(offset, length)` pairs for the gene, segment, block, payload, and exception sections in that order;
- bytes 104..144 hold gene, segment, block, ordinary-locus, and exception counts;
- bytes 144..148 hold block capacity `4096`; bytes 148..152 hold rank stride `64`; bytes 152..256 are zero.

Every section starts where the preceding section ends. The exception section ends at the declared file length. Gene entries are 32 bytes and sort by gene. They hold gene number, first segment, segment count, and ordinary-locus count. Segment entries are 48 bytes and sort by gene, contig, and start coordinate. They hold gene, contig, inclusive start and end, locus count, first block, and block count. Block entries are 40 bytes and sort by segment and first locus. They hold segment index, first segment-relative locus, locus count, payload-relative offset, payload length, active-locus count, and score-pair count. Every unassigned entry byte is zero.

A payload block begins with active-locus count, score-pair count, rank count, and one zero `u32`. Two-bit reference bases and one-bit active flags follow. Each 64-locus rank entry stores the preceding active-locus and score-pair counts as two `u32` values. Active loci contribute one packed six-bit mask in canonical alternate order. Each mask bit selects gain or loss. Selected pairs use one little-endian `u16`: bits 0 through 6 hold the 0..100 magnitude, bits 7 through 13 hold relative position plus 50, and bits 14 and 15 are zero. An omitted pair means magnitude zero and position -50. This distinction preserves zero scores that carry a nondefault position.

Exception entries are 40 bytes in gene, contig, and coordinate order. They hold contig, omitted base, three canonical alternate bases, gene, coordinate, and three gain/loss pair pairs. Reserved bytes are zero. All format lengths, offsets, counts, products, sums, and numeric conversions use checked arithmetic.

The independent test decoder validates every header field, section boundary, gene range and count, segment and block order, payload offset, rank checkpoint, stored score pair, exception order, and reserved byte. Mutation cases change each directory field, rank pair, stored score pair, section declaration, and reserved byte. Every mutation fails decoding or changes the decoded logical stream.

`sparse_heap_bound` measures allocator state from writer creation through every gene submission and final assembly. A concurrent sampler covers Linux resident memory through final assembly. Its nondefault-score corpus makes the disk payload dominate directory state, then requires peak and retained heap to stay below one eighth of final bytes and Linux resident growth to stay below one half. This evidence covers bounded construction only. It makes no complete-corpus size, reader safety, lookup latency, parity, corruption, or ADR 0027 promotion claim.

The sparse reader splits validation by access tier. Open validates the header, exact adjacent sections, checked section products, every reserved metadata byte, typed genes, contigs and coordinates, ordered unique gene ownership, nonoverlapping segment coverage, complete block and payload ownership, exception encodings and order, declared exception genes, and exception separation from ordinary loci. Open reads no ordinary payload bytes. Lookup validates the selected block header and exact length, every rank checkpoint, complete reference, active and mask arrays, unused tail bits, active and pair totals, active-mask consistency, and every pair for the addressed locus before returning a score or reference mismatch. `visit_all` and `verify_all` additionally decode and validate every score pair in every block. This offline tier detects corruption that open and an unrelated lookup may leave untouched.

Gene-filtered lookup binary-searches the gene and exception directories, scans only that gene's segments and exceptions, and binary-searches the selected segment's block directory. Its cost is `O(log G + S_gene + log E + E_gene + log B)`, plus selected-block validation. Unfiltered lookup scans the gene-ordered segment and exception directories. Its cost is `O(S + E + K log B)` plus validation for the `K` selected overlapping blocks. Selected-block validation repeats across calls. Later side-by-side qualification measures whether these choices pass ADR 0027. They do not activate or qualify the candidate.

### Certified complete sparse candidate build

Maintainers create one complete candidate from an immutable certified fixed-v1 bundle with:

```text
pangopup-build sparse-candidate build \
  --fixed-bundle data/pangopup/runtime-v0.5.0 \
  --expected-bundle-id sha256:<FIXED_BUNDLE_ID> \
  --scratch data/pangopup/sparse-v1.scratch \
  --candidate data/pangopup/sparse-v1-candidate.pgi \
  --report data/pangopup/sparse-v1-candidate-report.json
```

The command opens the exact `manifest.json`, `NOTICE`, and `scores.pgi` regular files before certification. Those inodes must remain immutable for the full command. Certification checks the closed canonical manifest, exact notice, member limits and hashes, canonical fixed structure, decoded counts, and both logical identities on the held files. The retained run supplies the expected bundle identity.

Fixed-v1 traversal preflights each complete gene before allocation. It refuses more than 3,000,000 loci or 512 MiB of `InputLocus` capacity. It passes its sole gene buffer directly to the sparse writer. The report records the observed maximum length, capacity, and capacity bytes.

The sparse writer transfers its already-held finished-candidate descriptor and cleanup ownership to the command. The command hashes that descriptor, maps it through `SparseIndexReader::open_file`, verifies every record, and reconstructs the canonical decoded logical digest. Writer counts, decoded counts, fixed manifest counts, and logical identities must agree exactly.

Candidate publication creates the final path with no-replace semantics and streams only from the held verified staging descriptor. The copy must reproduce the verified byte count and SHA-256. The command syncs the candidate and its parent, then verifies that the final path still names the created inode. It creates, writes, and syncs the no-replace report second, then verifies report ownership. The report is the completion marker. This portable macOS and Linux design adds one sequential candidate copy and temporary space for both staged and final candidate bytes. Cleanup removes a path only while it still names the builder-owned inode. Candidate and report publication are not one atomic operation.

The report uses canonical RFC 8785 JSON with schema `pangopup.sparse-candidate-report.v1`. It records the fixed bundle and member identities, candidate format, size and SHA-256, writer and decoded counts, source and decoded logical identities, gene-buffer measurements, and the 3,221,225,472-byte ADR 0027 gate. It omits paths, clocks, host data, elapsed time, and resident memory.

The output remains a candidate. It has no bundle, installed asset identity, runtime profile, activation, or route. A passing size and logical-parity report does not complete ADR 0027 qualification.

The correctness fixture selects edge cases. Ticket 002 used a deterministic
stratified real lab corpus for comparative warm selection and instrumented
logical bytes, mapped page numbers, allocations, and page faults. That corpus is
smaller than available memory, so it makes no cold-I/O claim. A defensible cold
measurement requires a dataset larger than available memory or an isolated
uncached device/read method—not merely the first query after a build, whose
pages are usually already hot. The Ticket 004 host
had more available memory than the 14.0 GiB member and no privileged/device
nonresidency proof, so its retained cold result is `unmeasured`; warm one-open
library, fresh CLI, open-only, and serialization measurements are reported
separately.

Ticket 006 adds a checked source-derived fixture with exactly 1,000 requests
and independent direct-TSV JSONL expectations. One provider open exercises all
requests; seven CLI batches cover the shared unfiltered batch and one batch per
gene filter. This fast semantic corpus protects ordinary development and CI
without requiring or scanning the retained production index.

Ticket 020 keeps that lookup-only corpus byte-exact and places
`LookupFirstRouter` in the warm hit benchmark. When the caller supplies a
complete fallback set, a pure SNV miss becomes model-required; any record or
source-reference ambiguity remains authoritative. Without fallback flags the
legacy SNV-only miss remains `not_found`, so existing lookup installations and
their regression oracle do not acquire a model dependency.
