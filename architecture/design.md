# Design Rules

## Product boundary

The first finished slice accepts a literal GRCh38 genomic SNV, finds every
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
- unversioned `EnsemblGeneId` for the published SNV source and distinct exact
  versioned/PAR `GencodeGeneId` for model masking;
- exact centi-scores and relative positions;
- gene-specific result records and source provenance;
- narrow provider capabilities and typed errors.

The shipped `ReferenceProvider` copies an exact one-based uppercase-IUPAC
window into caller-owned storage and reports immutable bundle provenance. It
does not expose paths, mmap lifetimes, offsets, aliases, or FASTA details.

Structs and newtypes represent data. Traits represent capabilities. Avoid a
trait for every type: the likely first abstraction is one score provider with
gene-filtered and all-overlap lookup operations.

A normal `GenomicSnv` requires concrete `A/C/G/T` reference and alternate
alleles. The source archive's 30 `REF=N` loci are preserved for lossless source
certification but are not silently reinterpreted as SNVs. An affected lookup
returns a typed ambiguous-source-reference outcome unless a future, separately
documented reference-remapping policy is adopted.

The owned `Grch38Variant` represents the model scorer's literal concrete
allele tuple. It permits supported shapes up to 100 bases but performs no
trimming, left alignment, equivalence collapsing, HGVS parsing, or transcript
projection. `ModelScoreResult` keeps expected rejections separate from
reference, mask, and model-provider failures and preserves exact GENCODE result
order.

### `pangopup-index`

Owns the runtime side and shared codec of the private storage contract:

- file magic, versions, sections, packing, checksums, and manifest identity;
- cheap open validation, the mmap lifecycle, checked byte decoding, and offline
  full-payload verification;
- direct-address and interval-assisted lookup.

No consumer is allowed to know an offset or cast mapped bytes. The format may
change without changing the provider contract.

The same crate exposes the byte-identical selected `PGMBEN01` domains member
through its production `mask` module. `MaskDomainsOpen` rejects both unselected
codecs and implements a `Send + Sync` `MaskProvider` over caller-owned reusable
storage. It preserves exact versioned/PAR identity, `(start,end]` membership,
plus-before-minus and authenticated within-strand order, optional stable-gene
filtering, and normalized exon boundaries. The historical writer, alternate
codecs, and source-format qualification APIs have been removed; retained
reports and exactness fixtures preserve the selection evidence. Ordinary open
remains cheap; a separate member-authentication open hashes the bounded bytes
through the same held descriptor that is mapped and returns its verified
identity. As specified by ADR 0013, the verified inode must remain immutable;
concurrent in-place mutation or truncation is outside the supported threat
model.

### `pangopup-build`

Owns streaming `.tsv.gz` ingestion, full-source validation, deterministic index
writing, reference certification, and atomic bundle publication. Gzip/TSV and
other build-only dependencies stay here and do not enter runtime consumers.

The one-time GENCODE observation, candidate construction, and qualification
program is not a supported builder surface and has been removed from the
workspace. Its exact selection report, query manifest, and historical ADRs are
retained. Runtime mask consumers need only the selected immutable member;
Python, SQLite, gffutils, and GTF parsing are not runtime or build-crate
dependencies for that member.

### `pangopup-assets`

Owns the byte-exact notice, exhaustive installed-bundle certification, strict
SNV transport manifest, pinned Zstandard codec, streaming part verification,
atomic local pack/unpack, and the Linux local-user store: XDG path resolution,
dirfd-relative no-follow state, locking, receipts, staging/reconciliation, and
active selection. Dependency direction is `pangopup-core <-
pangopup-index <- pangopup-assets <- pangopup-build`. The build CLI is a thin
adapter over the shared certification and transport APIs. `pangopup-index`
supplies the sole bounded canonical installed-manifest parser; assets does not
duplicate that grammar.

The discarded reference candidates and their benchmark adapter have been
removed. Their retained reports explain why the selected payload was
re-specified as the distinct `PGRREF01` production format. Runtime code uses
`ReferenceBundleOpen`; the production reader owns one read-only mmap and the
bounded ambiguity table. The build crate owns authenticated FASTA/report
parsing, full logical certification, and atomic publication.

### `pangopup-cli`

Owns arguments, narrow genomic-variant input parsing, output rendering, and
exit codes. It opens one configured Pangopup bundle for the process and reuses
it across one validated batch. It does not parse source TSV
files. The binary and performance harness call the same library renderer, so
measured JSONL/table serialization is the production byte path rather than a
benchmark copy.

`pangopup-model` owns the authenticated three-file model bundle, bounded
A/C/G/T/N context encoding, and one mutable CPU ONNX Runtime session returning
twelve raw replicate channels in genomic orientation. It intentionally does
not construct genomic variants or hide ensemble and masking behavior.

### `pangopup-engine`

Owns the mutable single-owner composition of `ReferenceProvider`,
`MaskProvider`, and `ModelKernel`. It fixes GRCh38, masked output, and distance
50; constructs exact reference/alternate contexts; retains `f32` for
equal-length/insertion arithmetic and upstream-promoted `f64` for deletions;
and masks one shared gain/loss pair in authenticated plus-then-minus gene
order. Its kernel test seam is private. It owns no lookup routing, gene filter,
asset path discovery, delivery, cache, pool, CLI, HTTP, or concurrency claim.

A future `pangopup-http` crate must consume the same eventual routed provider
rather than leak model runtime, HTTP, or cache types into the scoring API. The
shipped assets crate owns explicit pinned remote sync plus its Linux
XDG installation adapter. `pangopup-core` performs no network or home-directory
access. The future HTTP adapter runs in the
foreground; process lifecycle belongs to external managers as described in
[`service.md`](service.md).

## Query identity and multiplicity

A genomic allele alone is not always one Pangolin annotation. The source is
partitioned by Ensembl gene, genes can overlap, and annotation masking can make
scores gene-specific. The core result is therefore zero, one, or several
gene-specific score records. Precomputed lookup sorts by stable Ensembl ID;
model scoring preserves authenticated plus-before-minus query order because
masking makes that order semantic.

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

The shipped variant scorer deliberately has no gene filter: it queries and
masks every containing exact GENCODE identity before a future router may apply
an unversioned filter. That later filter must match stable components only on
the already selected contig and must not merge chrX and chrY `_PAR_Y` copies.
Exact mask query order is semantic because upstream Pangolin mutates shared
score arrays while visiting overlapping same-strand genes.

The current CLI slice accepts concrete SNVs only and tries the precomputed index.
A concrete tuple whose reference does not match an ordinary indexed key is a
typed-context `not_found` result because the lookup-only route does not consult
the shipped GRCh38 sequence provider. The model scorer instead compares the
literal REF at the context anchor and returns typed `ReferenceMismatch`; a
later router will choose between these already defined behaviors. Every
current lookup result identifies the precomputed bundle and source provenance.

See [`runtime-data.md`](runtime-data.md) for the small set of standalone assets
needed by lookup and model execution.

## Exactness

The source scores are decimal hundredths. Relative positions span `-50..=149`
for the fixed model boundary so a 100-base reference allele remains fully
representable; existing SNV bytes remain within `±50`. Core types preserve gain magnitude and loss
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

Today the CLI opens either one explicitly supplied bundle or the atomically
selected active Linux installation. Local install/status and active discovery
and pinned remote sync are shipped; no HTTP adapter exists. In the target service,
before serving, an adapter asks remote sync to obtain one binary-pinned
compatible transport and passes it through the same local installer. The core
then opens one immutable bundle. A replacement bundle requires a new process.

The installed flow checks transport and reconstructed hashes while streaming
once. Ordinary startup performs cheap receipt, identity, version, size, and
structural checks so it does not page through every multi-gigabyte member. The
shipped build-time verification command owns exhaustive semantic scanning.

The operating-system page cache is the first lookup cache; Pangopup does not add
an application result cache until measurements show a miss it can improve.

Lookups are deterministic and thread-safe through one `ScoreProvider: Send +
Sync`. Results own small sorted record and ambiguity collections rather than
exposing mmap lifetimes. The selected installed profile is decompression-free
fixed-width mmap; transport compression is removed once at installation and
never appears on the query path.

The current sync profile provisions only the SNV transport. A future full
service must pin a coherent set of SNV, compiled GRCh38 sequence index, mask,
and converted model identities before it can report readiness. Existing
sequence, mask, raw-model, and variant-scoring providers still need
lookup-first routing and coherent asset activation before product fallback is
operational.

## Planned extensions not yet shipped

- lookup-first routing through one typed result/provenance API;
- stable CLI model output over the routed result;
- application-level model-result caching only if measurements justify it;
- separately versioned converted model, compiled GRCh38 sequence index, and mask
  publication and pinned sync;
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
