# Design Rules

## Product boundary

The first finished slice accepts a normalized GRCh38 genomic SNV, finds every
matching gene-specific record present in one pinned source archive, and returns
the four published Pangolin values plus provenance. The complete service adds a
model provider for supported variants without an index result.

```text
CLI/HTTP structured genomic variant
  -> validate GRCh38 contig, position, reference, and alternate
  -> if SNV, try the validated mmap score index
       -> hit: exact precomputed result(s)
       -> miss: continue
  -> if supported, run model with mmap reference + masking data
  -> typed gene-specific result(s) with source provenance
  -> stable text or JSON output
```

The CLI is the first observable adapter. A future HTTP service calls the same
Rust API and owns no scoring, normalization, or file-format rules.

## Crate ownership

### `pangopup-core`

Owns the stable concepts callers use:

- a build-qualified genomic SNV;
- canonical contig/accession, 1-based position, reference, and alternate allele;
- Ensembl gene identity;
- exact centi-scores and relative positions;
- gene-specific result records and source provenance;
- narrow provider capabilities and typed errors.

Structs and newtypes represent data. Traits represent capabilities. Avoid a
trait for every type: the likely first abstraction is one score provider with
gene-filtered and all-overlap lookup operations.

A normal `GenomicSnv` requires concrete `A/C/G/T` reference and alternate
alleles. The source archive's 30 `REF=N` loci are preserved for lossless source
certification but are not silently reinterpreted as SNVs. An affected lookup
returns a typed ambiguous-source-reference outcome unless a future, separately
documented reference-remapping policy is adopted.

### `pangopup-index`

Owns the runtime side and shared codec of the private storage contract:

- file magic, versions, sections, packing, checksums, and manifest identity;
- cheap open validation, the mmap lifecycle, checked byte decoding, and offline
  full-payload verification;
- direct-address and interval-assisted lookup.

No consumer is allowed to know an offset or cast mapped bytes. The format may
change without changing the provider contract.

### `pangopup-build`

Owns streaming `.tsv.gz` ingestion, full-source validation, deterministic index
writing, reference certification, and atomic bundle publication. Gzip/TSV and
other build-only dependencies stay here and do not enter runtime consumers.

### `pangopup-cli`

Owns arguments, narrow genomic-variant input parsing, output rendering, and
exit codes. It opens one configured Pangopup bundle for the process and reuses
it for every request in batch or streaming modes. It does not parse source TSV
files.

Future `pangopup-assets`, `pangopup-model`, and `pangopup-http` crates should be
added only when their own observable slices begin. They must consume the same
core types rather than leak a model runtime, HTTP, or cache types into the
scoring API. `pangopup-assets` owns shared discovery, download, verification,
and atomic installation for both executable adapters; `pangopup-core` performs
no network or home-directory access.

## Query identity and multiplicity

A genomic allele alone is not always one Pangolin annotation. The source is
partitioned by Ensembl gene, genes can overlap, and annotation masking can make
scores gene-specific. The core result is therefore zero, one, or several
gene-specific score records in deterministic Ensembl-gene order.

“All overlaps” means all matching source gene records in the pinned archive. It
does not claim completeness against a newer or otherwise unspecified GENCODE
release.

An optional gene filter gives the common single-record path. A caller that does
not provide a gene receives all matching source records; Pangopup must never
silently choose one gene. This is the CLI and library default.

## Standalone variant boundary

Pangopup is a standalone splice service. Its canonical request is the minimum
information needed to identify a genomic allele:

```text
assembly=GRCh38, contig=17, position=43106534, ref=C, alt=A
```

The first CLI and service may accept `17`, `chr17`, or the exact primary RefSeq
accession `NC_000017.11` through a small pinned alias table. This is coordinate
parsing, not a general HGVS system.

HGVS is not required for splice scoring. Supporting transcript `c.` or protein
`p.` input would require transcript versions, exon geometry, normalization, and
projection rules that are unrelated to scoring. Protein notation is especially
insufficient: several nucleotide variants can produce the same protein change,
and a splice effect cannot generally be reconstructed from the protein result.
Callers that begin with those forms must resolve them to one concrete GRCh38
genomic allele before calling Pangopup.

A gene is also not required in a request. The source archive is partitioned by
Ensembl gene and masking is gene-specific, so the same genomic allele can have
several valid score records. Pangopup returns every matching record by default
and accepts an optional source-gene filter; it never guesses one best gene.

An SNV tries the precomputed index first. A lookup miss or a non-SNV routes to
the bundled model when that variant shape is supported and the required gene
context exists. Unsupported shapes and reference mismatches fail with typed
errors. Every success identifies whether lookup or inference produced it.

See [`runtime-data.md`](runtime-data.md) for the small set of standalone assets
needed by lookup and model execution.

## Exactness

The source scores are decimal hundredths and relative positions are integers in
the configured ±50 window. Core types preserve gain magnitude and loss
magnitude as integers, with sign implied and validated by the field, rather than
using binary floating point. Rendering gain `0.21` or loss `-0.21` from integer
21 is exact and stable.

Gain/loss positions are genomic-coordinate deltas from the input variant:
positive means a higher genomic coordinate, including for a minus-strand gene.
They locate the predicted affected splice position; they are not
transcript-oriented distances to an exon boundary.

Positive and negative textual zero are the same numeric score. If byte-for-byte
reproduction of source spelling such as `-0.0` becomes a requirement, that is a
separate presentation field and should not pollute score semantics.

Every result carries enough provenance to identify:

- the source dataset DOI and archive checksum;
- the Pangopup index format and bundle identity;
- GRCh38 as the coordinate assembly;
- masked scores, window size 50;
- the source Ensembl gene.

## Runtime behavior

Before serving, the CLI or HTTP adapter asks the asset manager to ensure one
binary-pinned compatible bundle. Missing assets are downloaded, verified,
staged, and atomically installed by default; offline mode fails with a precise
missing-asset error. The core then opens one immutable bundle. A replacement
bundle requires a new process.

Installation performs full archive and member hash verification. Ordinary
startup performs cheap identity, version, size, and structural checks so it does
not page through every multi-gigabyte member. An explicit verification command
owns repeat full hashing.

The operating-system page cache is the first lookup cache; Pangopup does not add
an application result cache until measurements show a miss it can improve.

Lookups must be deterministic, thread-safe, and allocation-light. Returning a
small owned score record is preferable to exposing mmap-backed lifetimes across
the public API. The primary installed profile is decompression-free sparse mmap;
transport compression is removed once at installation and never appears on the
query path.

## Deliberate first-slice exclusions

- non-SNV inference;
- model conversion or model execution;
- SQLite result caching;
- REST or gRPC;
- GRCh37 or liftover;
- transcript HGVS, protein HGVS, projection, or normalization;
- hot reload;
- threshold-based clinical interpretation.

These are compatible extensions, but none should complicate proof that the SNV
index is correct, compact, and fast.
