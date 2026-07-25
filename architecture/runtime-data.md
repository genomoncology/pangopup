# Standalone Runtime Data

## Lookup path

An indexed SNV lookup needs only the Pangopup fixed-v1 score bundle. The variant's
GRCh38 contig, position, reference, and alternate select the record. The bundle
already contains the source Ensembl gene identity, masked gain/loss values, and
their relative positions. It does not need a FASTA, GTF, transcript database,
or network call on this path.

On Linux, `pangopup assets sync` downloads the exact compiled-in public
transport into disposable XDG cache and passes it to the same
`pangopup assets install` boundary that reconstructs a supplied transport under
XDG user data. The installer records its canonical receipt and atomically
selects it in `active.json`. Normal lookup discovers that active bundle without a
`--bundle` argument and performs only cheap manifest/size/structure checks.
`--bundle` remains an explicit override. Lookup never downloads data or scans
the complete score payload at startup; only the explicit sync command uses the
network.

The request's reference allele remains part of the key. A wrong reference
therefore fails or misses rather than returning the score for a different
allele. The full model reference is not loaded merely to repeat that check for
an indexed SNV.

## Model fallback assets

Model fallback is not implemented. Its accepted data boundary requires three
additional local facts; the second is now implemented as a provider and build
artifact, but is not yet installed or routed into inference:

Running Pangolin for a lookup miss or non-SNV genuinely needs:

1. **Model weights.** The twelve version-2 checkpoints loaded by the current
   upstream program, or a verified equivalent conversion.
2. **GRCh38 reference bases.** Pangolin reads a long DNA window around the
   variant, verifies the submitted reference allele, and scores reference and
   alternate sequences. Pangopup pins NCBI's RefSeq GRCh38.p14 assembly,
   accession `GCF_000001405.40`. The source FASTA is compiled into a compact,
   indexed mmap member; normal installations do not parse or retain raw FASTA.
3. **Gene strand and exon boundaries.** Pangolin first finds every gene body
   containing the variant and runs the appropriate strand. In masked mode it
   keeps splice loss at annotated exon boundaries and splice gain away from
   annotated boundaries. Without these facts Pangopup could return unmasked
   neural-network output, but not the same masked product as the precomputed
   archive.

Upstream Pangolin obtains item 3 from a gffutils database generated from
GENCODE. Its documented GRCh38 default is GENCODE release 38 with
`Ensembl_canonical` transcripts. Ticket 012 compiled those facts into three
private candidate mmap members and selected constant-membership domains by the
closed speed-first comparison. This selects a representation for later
hardening, not a production format: no runtime mask provider exists. SQLite,
gffutils, and the GTF remain build inputs only.

The logical mask is deliberately richer than a coordinate set. It retains the
exact versioned GENCODE identifier and optional `_PAR_Y` suffix, its
unversioned stable component, contig, strand, inclusive GTF span, Pangolin's
effective `(start,end]` point membership, observed point-query rank, and the
canonical exon-boundary set. Stable filtering happens after contig selection;
it never merges chrX and chrY pseudoautosomal copies. Ordered same-strand
results must be preserved because upstream masking mutates shared arrays as it
visits genes.

The pinned sequence source is the [NCBI RefSeq GRCh38.p14 assembly](https://www.ncbi.nlm.nih.gov/datasets/genome/GCF_000001405/).
The masking source is the archived [GENCODE release 38](https://www.gencodegenes.org/human/release_38.html)
annotation used by the upstream instructions, not a moving "latest" release.

The runtime representation was selected by measurement rather than assumption.
The closed comparison used uppercase ASCII, exact four-bit IUPAC, and two-bit
ACGT plus exact ambiguity runs in one common mmap container. The retained
six-contig result selected `acgt2-rle-v1` by speed: headline p50/p95 were
4,469/4,880 ns versus 16,272/18,366 for ASCII and 34,267/41,522 for IUPAC4.
A checked 25-contig miniature proves symbol, boundary, corruption, allocation,
and logical-page behavior in normal tests. The production `PGRREF01` bundle is
not a benchmark container: cheap open validates bounded structure and all
ambiguity runs, while private build certification decodes every base and checks
the manifest-bound member hashes and compatibility contexts.

RefSeq and GENCODE have different roles here. RefSeq supplies the GRCh38 DNA
bases. GENCODE supplies the gene/exon map required by Pangolin's masking rules
and exact versioned/PAR identities used during model masking. The Zenodo lookup
source separately uses unversioned Ensembl identifiers. This does not introduce
general gene annotation into the public API.

Pangopup now pins that boundary in `tests/fixtures/pangolin-compat-v1`. The
227,060-byte corpus contains 14 scored genomic cases, six rejection cases, and
four controlled post-processing cases captured from source commit `5cf94b8`
and twelve exact checkpoints. It retains exact RefSeq GRCh38.p14 contexts,
GENCODE v38 gene/exon facts, typed raw arrays, masked and unmasked output,
overlapping-gene order, and rejection witnesses. Its Rust inspector replays the
semantics offline. This corpus—not architectural similarity—is the acceptance
oracle for CPU inference and any later conversion. Its controlled vectors and
expectations are fixed independently from replay, and its provenance capture path
authenticates the live helper, the held base Python executable, the venv
launcher relationship and `pyvenv.cfg`, and all loaded file-backed or
interpreter-owned modules before and after execution. Capture uses `-S` plus an
authenticated held-prefix import path so venv `.pth` startup code is not an
unrecorded input; bytecode lookup is redirected away from existing caches.
Every schema, query-plan, and compile-option cursor is required to return an
actual `sqlite3.Row`, then converted positionally with exact per-column scalar
types. A duplicate-column in-memory control proves sequence semantics before
the production database is observed. The schema digest keeps the previously
measured pipe/NULL/LF transformation as a named legacy secondary observation;
the exact database SHA-256 remains the primary identity.

That compatibility corpus proves its selected overlap and masking cases; it is
not a complete all-gene order inventory. Ticket 012's authenticated canonical
export and complete-domain comparison are the authority for the complete
GENCODE mask. The retained run certified all three layouts and selected domains
by speed over its fixed 1,000-query workload.

Environment failure is not capture success. Before the final observed
environment can determine the capture contract ID, a handled failure is sealed
once in a deterministic mode-0700 preflight-failure stage containing bounded,
path-free evidence. It has no source snapshot or phase receipt and cannot be
automatically retried or consumed as runtime data.

The complete captured environment is allowed to exceed generic metadata: its
formal maximum is 320 KiB, derived from 512 canonical module identities at 512
bytes each plus a 64 KiB non-module envelope. The final contract is bounded at
384 KiB. The measured pinned environment is 79,641 bytes with 254 modules.
These bounds preserve the full authenticated inventory rather than truncating
it, while receipts, reports, inventories, and reuse authorizations remain
64 KiB. Full observation JSONL retains a separate 4 MiB per-record bound.

The fourth production launch sealed that complete ordered observation before
prepare failed. The pinned GENCODE v38 GTF has a closed mixed attribute grammar:
strings are quoted, while only `level` and `exon_number` are bare canonical
positive decimals. This is both fully inventoried and consistent with the
official [GENCODE data format](https://www.gencodegenes.org/pages/data_format.html).
The corrected build-only parser does not accept arbitrary bare values.

Changing that parser necessarily changed the mask-builder identity. The
completed capture promotion was therefore not an exception to identity
checking: it derived a new contract whose sole changed field was builder
provenance and required independently reviewed authorization binding the exact
old contract, capture receipt, and prepare-failure receipt. Only sealed
source/capture members were eligible. Runtime consumers receive none of the
GTF, SQLite, Python, promotion, or failure material; a future production bundle
will contain only a separately hardened domain representation and provenance.

## Reproduction boundary

The precomputed dataset publisher calls its coordinates hg38 and says scores
were masked with GENCODE annotations, but does not identify the exact FASTA,
GENCODE files, Pangolin package commit, or checkpoint identity. A local check of
1,023,901 positions across ten genes found no reference-base differences from
RefSeq GRCh38.p14, so the reference is compatible over that checked region; it
does not prove the publisher used the same FASTA.

Pangopup therefore versions the lookup artifact and the fallback model artifact
separately. Before claiming parity, the checked corpus must compare both routes.
Small numeric differences with identical masking are more likely to come from
model/checkpoint or numeric-runtime differences than from reference bases once
the submitted reference allele has been verified.

CPU compatibility is proved before accelerator selection. MPS, CUDA,
alternative runtimes, quantization, or other optimizations are accepted only if
they preserve the defined result/error behavior within explicit retained
tolerances and improve measured end-to-end performance or resource use.

The current Python implementation also mutates its gain/loss arrays while
masking each gene. The strict compatibility profile retains observed SQLite
gene order and proves that a later same-strand gene sees earlier mutations. A
Rust fallback claiming this profile must preserve that behavior; an improved
independent-per-gene policy requires a separately named profile.

## What Pangopup deliberately does not ship

- a transcript-alignment or general sequence database;
- an HGVS parsing or coordinate-projection engine;
- transcript and protein sequences;
- gene descriptions, aliases, disease knowledge, or consequences;
- PostgreSQL, SQLite, or gffutils as a runtime dependency.

The shipped standalone lookup deployment is therefore the executable plus the
fixed-v1 score bundle. The target complete deployment adds weights, a compact
GRCh38 sequence member, and a compact Pangolin masking member; a future
lookup-only profile can omit those three.
