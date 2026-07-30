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

## Artifact provenance

The SNV score bundle and production-reference bundle are independently built
assets. New builds record separate source identities under
`pangopup.snv-builder-source.v1` and
`pangopup.reference-builder-source.v1`. Each identity covers only the checked
source and locked dependency evidence that can affect its artifact. Changing
mask runtime, delivery, CLI, or another unrelated subsystem therefore
does not create a new SNV or reference identity.

That source fingerprint is descriptive build evidence, not a requirement that
the current executable match the artifact's historical builder. Existing
public and qualified v1 bundles remain immutable and readable after the
provenance split. Runtime compatibility continues to come from the bundle
schema, format identity, manifest claims, and member-integrity checks.

## Model-scoring assets

Variant-level model scoring is implemented as a library composition over held
installed providers or one complete explicit override. The four assets are bound by one canonical
path-free compatibility profile. The offline Linux installer copies the model,
compiled reference, and mask into private immutable XDG storage, reuses the
certified active SNV object, and atomically selects one coherent profile. The
CLI admits that profile only after lookup requires inference. Explicit paths
remain an all-or-nothing override:

Running Pangolin for a lookup miss or non-SNV genuinely needs:

1. **Model weights.** Pangopup authenticates the twelve version-2 checkpoints
   loaded by the current upstream program and converts them without
   quantization into one bounded ONNX bundle. The production bundle remains an
   unpublished build output; original checkpoint containers are not runtime
   inputs.
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
closed speed-first comparison. ADR 0013 promotes the exact retained
6,703,320-byte domains member behind a domains-only production mmap provider;
it creates no second format or builder. SQLite, gffutils, and the GTF remain
in the historical qualification evidence only; they are not current runtime or
build-crate dependencies for the selected member.

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

## Coherent compatibility authority

`pangopup.runtime-profile.v1` binds exact bundle/member identities for the SNV
index, singleton model, compiled RefSeq reference, selected GENCODE mask, and
the closed distance-50 sequential `1/1` scoring policy. Its SHA-256 is computed
over exact RFC 8785/JCS bytes. The document contains no paths or URLs.

The maintainer prepare command authenticates the three model-side members
through held descriptors. It reads only bounded SNV manifest/notice metadata
and the held score file's size; it does not scan or hash `scores.pgi`.
Installation/certification remains the full SNV payload authority. The
maintainer command does not install, activate, download, publish, initialize
ONNX, or infer. `pangopup assets runtime install` consumes its exact output plus
three trusted local derived assets. It streams each fallback source once into
staging while hashing, validates the staged structures, publishes immutable
objects, and then replaces `runtime/active.json`. Runtime status performs
bounded metadata and size checks.

`pangopup-build runtime-transport` is the local delivery boundary for those
already accepted bytes. `pack` copies the profile and bounded component
metadata exactly, carries the checked GENCODE attribution notice, and creates
separate deterministic Zstandard frames for `model.onnx`, `reference.pgr`, and
`domains.pgm`. The canonical manifest binds stored and reconstructed identities
and the runtime-profile identity. The SNV facts remain profile metadata; the
15 GB score member is never opened.

`verify` streams all three frames without materializing runtime files. `unpack`
uses the same one-pass authentication while writing private staged outputs,
then publishes the complete reconstructed layout by atomic no-replace rename.
This proves transport integrity, not publisher identity or trusted-production
admission. Public URLs, remote sync, and install-from-transport remain later
delivery policy.

`pangopup-build runtime-release prepare` promotes only the exact production
transport identity into a controlled publication source. It does not inspect
raw NCBI, GENCODE, Zenodo, or checkpoint inputs. It performs one bounded
streaming decode of each frame to authenticate the manifest-declared
reconstructed identities before copying the stored bytes. Its release profile
records where the exact upstream model
source and checkpoints can be obtained and binds the Pangopup converter paths
to the target commit.

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

The separate checked `pangolin-model-v1` trust root now qualifies the raw model
kernel itself. It authenticates every state tensor from each checkpoint and
retains 45,756 independently generated selected-channel `f32` values. The
reviewed converter produces one fixed-order, twelve-channel ONNX graph;
`pangopup-model` authenticates the three-file bundle and preserves the selected
v1 singleton grammar. Two explicit closed v2 contracts and their
representation-specific probes remain maintainer-only experiment machinery;
ordinary runtime rejects them. Qualification compares all retained values
through ONNX Runtime on CPU. Normal
tests use a tiny same-schema synthetic graph rather than rerunning Python or
shipping production weights.

`pangopup-engine` composes these providers for the literal supported allele
subset. For position `P` and REF length `R`, it requests `10,100 + R` bases
starting at `P - 5,050`, validates the anchor, constructs the alternate, queries
every containing gene without a filter, and invokes plus-reference,
plus-alternate, minus-reference, then minus-alternate. It preserves `f32`
equal-length/insertion arithmetic and deletion-promoted `f64`, then performs
shared-array masking and exact hundredth conversion.

The engine router tries a one-base substitution against the published provider
first. Any record or source-reference ambiguity is authoritative. With CLI
fallback enabled, a pure miss or non-SNV consumes one identity-bound scorer;
the optional stable-gene filter is applied only after all-gene scoring. The CLI
reports the exact model/reference identities and the observed SHA-256/length of
the same descriptor-held mask mmap used by scoring. Explicit fallback also
hashes the complete bounded `reference.pgr`, verifies the manifest-declared
member digest, and scores through an mmap created from that same retained
descriptor. This full reference read happens only when a batch actually needs
explicit fallback. Ordinary installed-reference open remains structural and
cheap: installation authenticated the member once before activation, and
runtime maps the held installed descriptor without rehashing the dense payload.

Successful model fallback now stores complete unfiltered typed records in a
disposable SQLite cache. Its full key binds the literal variant to every model,
reference, mask, scoring, window, and CPU-policy identity. A reopened hit still
validates bounded identity inputs but does not open the dense reference or
ONNX model, initialize ONNX Runtime, infer, or write SQLite. Inserts and
explicit updates define deterministic eviction order; ordinary valid hits do
not refresh it. Precomputed SNV hits never open this cache. The default lives
under XDG cache, not XDG data, because it can always be deleted and recomputed.

That compatibility corpus proves its selected overlap and masking cases; it is
not a complete all-gene order inventory. Ticket 012's retained authenticated
canonical-export report and complete-domain comparison are the authority for
the complete GENCODE mask. The retained run certified all three layouts and
selected domains by speed over its fixed 1,000-query workload.

The one-time GTF/gffutils capture, candidate writer/readers, qualification
lifecycle, and mask-builder fingerprint have been removed from the current
source tree. Their detailed receipts and reports remain historical evidence.
Runtime consumers receive none of the GTF, SQLite, Python, promotion, or
failure material. The shipped local transport contains only the exact selected
domains member, its identity metadata, and attribution. Immutable publication
uses those same derived bytes; automatic sync remains future.

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

Raw CPU-kernel compatibility is proved with maximum observed absolute error
well below the accepted `1e-5` tolerance. Variant-level compatibility is also
proved across the 14 scored cases, 36 raw evaluations, six rejections, and four
controlled cases. Lookup routing and explicit-path CLI model output are
shipped. Complete-request CPU qualification retains sequential `1/1` as the
portable ordinary policy because affinity-aware `auto/1` failed the two-strand
improvement gate. Fixed `8/1` is the measured winner for the retained host, not
a portable default or part of model asset identity.
MPS, CUDA, alternative runtimes, quantization, or other optimizations are
accepted only if they preserve the same result/error behavior and improve
measured end-to-end performance or resource use.

The current Python implementation mutates its gain/loss arrays while masking
each gene. The Rust scorer preserves the retained SQLite gene order and proves
that a later same-strand gene sees earlier mutations. An improved
independent-per-gene policy requires a separately named profile.

## What Pangopup deliberately does not ship

- a transcript-alignment or general sequence database;
- an HGVS parsing or coordinate-projection engine;
- transcript and protein sequences;
- gene descriptions, aliases, disease knowledge, or consequences;
- PostgreSQL or gffutils as a runtime dependency. SQLite is used only for the
  disposable model-result cache, never for reference, mask, or SNV lookup.

The shipped standalone lookup deployment is the executable plus the fixed-v1
score bundle. Explicit-path or activated-profile fallback additionally needs
the converted model bundle, compact GRCh38 sequence bundle, and compiled
Pangolin mask member. The shipped coherent profile and offline XDG installer
install and activate those four identities together; its local transport is
also shipped, and its exact public assets are immutable. Automatic provisioning
and direct install-from-transport integration remain future work. Lookup-only
use continues to omit the latter three assets.
