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
  -> stable CLI JSONL/table or future HTTP JSON output
```

The CLI is the shipped observable adapter: JSON Lines is its stable default and
exact tab-separated output is available explicitly. A future HTTP service calls
the same Rust API and owns no scoring, normalization, or file-format rules.

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

### `pangopup-assets`

Owns the byte-exact notice, exhaustive installed-bundle certification, strict
SNV transport manifest, pinned Zstandard codec, streaming part verification,
and atomic local pack/unpack. Dependency direction is `pangopup-core <-
pangopup-index <- pangopup-assets <- pangopup-build`. The build CLI is a thin
adapter over the shared certification and transport APIs. `pangopup-index`
supplies the sole bounded canonical installed-manifest parser; assets does not
duplicate that grammar.

### `pangopup-cli`

Owns arguments, narrow genomic-variant input parsing, output rendering, and
exit codes. It opens one configured Pangopup bundle for the process and reuses
it for every request in batch or streaming modes. It does not parse source TSV
files. The binary and performance harness call the same library renderer, so
measured JSONL/table serialization is the production byte path rather than a
benchmark copy.

Future `pangopup-model` and `pangopup-http` crates should be
added only when their own observable slices begin. They must consume the same
core types rather than leak a model runtime, HTTP, or cache types into the
scoring API. The shipped assets crate intentionally has no network,
home-directory discovery, or managed-install policy; those capabilities extend
its verified local primitive in later bounded work. `pangopup-core` performs no
network or home-directory access. The future HTTP adapter runs in the
foreground; process lifecycle belongs to external managers as described in
[`service.md`](service.md).

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

The current CLI slice accepts concrete SNVs only and tries the precomputed index.
A concrete tuple whose reference does not match an ordinary indexed key is a
typed-context `not_found` result, because no runtime FASTA is present. A later
model slice may route misses and supported non-SNVs through pinned reference and
mask assets; it must define its own reference-mismatch behavior. Every current
result identifies the precomputed bundle and source provenance.

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

## Target runtime behavior

Today the CLI opens one explicitly supplied bundle and no HTTP adapter or asset
manager exists. In the target service, before serving, the CLI or HTTP adapter
asks the asset manager to ensure one binary-pinned compatible bundle. Missing
assets are downloaded, verified, staged, and atomically installed by default;
offline mode fails with a precise missing-asset error. The core then opens one
immutable bundle. A replacement bundle requires a new process.

The planned installation flow performs full archive and member hash
verification. Ordinary startup will perform cheap identity, version, size, and
structural checks so it does not page through every multi-gigabyte member. The
shipped explicit verification command already owns repeat full hashing.

The operating-system page cache is the first lookup cache; Pangopup does not add
an application result cache until measurements show a miss it can improve.

Lookups are deterministic and thread-safe through one `ScoreProvider: Send +
Sync`. Results own small sorted record and ambiguity collections rather than
exposing mmap lifetimes. The selected installed profile is decompression-free
fixed-width mmap; transport compression is removed once at installation and
never appears on the query path.

## Planned extensions not yet shipped

- model conversion and CPU execution for supported lookup misses and non-SNVs;
- lookup-first routing through one typed result/provenance API;
- application-level model-result caching only if measurements justify it;
- pinned asset installation and remote sync;
- foreground HTTP serving plus container and native service-manager
  integration; and
- measured accelerator backends only after CPU compatibility is proved.

These extensions must preserve the exact, compact, fast SNV index rather than
complicate or replace it.

## Permanent non-goals

- GRCh37 or liftover;
- transcript HGVS, protein HGVS, projection, or normalization;
- threshold-based clinical interpretation or general gene annotation; and
- an internal daemon supervisor or in-process hot reload. External process
  managers replace a running foreground process when assets or software change.
