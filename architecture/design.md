# Design Rules

## Product boundary

The first finished slice accepts a normalized GRCh38 genomic SNV, finds every
matching gene-specific record present in one pinned source archive, and returns
the four published Pangolin values plus provenance. It is an annotation lookup,
not model inference.

```text
CLI text
  -> parse a narrow GRCh38 SNV/HGVS input
  -> typed lookup request
  -> score-provider capability
  -> validated mmap index
  -> typed gene-specific score result(s)
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

Owns arguments, input parsing at the user boundary, output rendering, and exit
codes. It opens one configured bundle for the process and reuses it for every
request in batch or streaming modes. It does not parse source TSV files.

Future `pangopup-model` and `pangopup-http` crates should be added only when
their own observable slices begin. They must consume the same core types rather
than leak ONNX, Torch, HTTP, or cache types into the lookup API.

## Query identity and multiplicity

A genomic allele alone is not always one Pangolin annotation. The source is
partitioned by Ensembl gene, genes can overlap, and annotation masking can make
scores gene-specific. The core result is therefore zero, one, or several
gene-specific score records in deterministic Ensembl-gene order.

“All overlaps” means all matching source gene records in the pinned archive. It
does not claim completeness against a newer or otherwise unspecified GENCODE
release.

An optional gene filter gives the common single-record path. A caller that does
not provide a gene receives all overlaps; Pangopup must never silently choose
one gene. Whether CLI v1 requires the gene or defaults to all overlaps remains a
product choice in planning.

## HGVS boundary

Pangopup needs only the small input subset required to identify a genomic SNV,
for example `NC_000017.11:g.43106534C>A`. It should not grow a second general
HGVS engine.

The first parser can deliberately accept only canonical GRCh38 genomic SNVs and
reject transcript HGVS, indels, uncertain positions, and noncanonical contigs
with typed errors. If the Genome project exposes a stable reusable parser and
normalizer, the CLI can adapt it later without changing the index contract.

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

One long-lived reader opens one immutable bundle. A replacement bundle requires
a new process. The operating-system page cache is the first cache; Pangopup does
not add an application cache until measurements show a miss it can improve.

Lookups must be deterministic, thread-safe, and allocation-light. Returning a
small owned score record is preferable to exposing mmap-backed lifetimes across
the public API.

## Deliberate first-slice exclusions

- non-SNV inference;
- model conversion or model execution;
- SQLite result caching;
- REST or gRPC;
- GRCh37 or liftover;
- transcript-level HGVS and normalization;
- hot reload;
- threshold-based clinical interpretation.

These are compatible extensions, but none should complicate proof that the SNV
index is correct, compact, and fast.
