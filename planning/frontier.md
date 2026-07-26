# Frontier

Updated: 2026-07-26

## Current boundary

Repository onboarding, source ingestion, format selection, full-corpus build
and certification, and typed SNV lookup are established. Pangopup is standalone
open-source software. Its shipped CLI accepts an explicit GRCh38 SNV and
optional gene filter and returns all matching source records by default from an
explicit fixed-v1 bundle or the active Linux user-data installation. Speed
leads memory and download size. Deterministic local transport, atomic install,
status, active discovery, cheap reuse, and the fast 1,000-case regression are
established. Immutable publication and pinned resumable remote sync are also
complete. The strict upstream Pangolin compatibility corpus is established.
Retained GENCODE-mask qualification evidence authenticates the exact upstream
semantics and records the selection of constant-membership domains from three
mmap candidates in one full-source comparison. The one-time candidate and
qualification source has been removed. The exact retained domains
member is now available through a production, domains-only typed mmap provider;
its delivery remains future. An authenticated combined
ONNX representation and qualified single-owner CPU kernel now return the
twelve raw Pangolin channels. `pangopup-engine` now composes the sequence,
mask, and kernel providers into compatible masked distance-50 scores for the
supported literal GRCh38 subset. Lookup-first fallback, CLI model output, CPU
tuning, model delivery, caching, and HTTP remain future.
The compiled production GRCh38 sequence-index bundle, authenticated builder,
and typed mmap provider are established; its transport, installation, and
release remain future delivery work. Future SNV and sequence-index builds use
separate causal source/dependency fingerprints; their existing production
assets remain immutable and readable.

## Asset readiness — preserve, package, then publish

The large source work is not a rebuild queue:

- The SNV lookup is complete and public as the immutable eight-asset
  `snv-grch38-v1` GitHub release. Its installed mmap member is
  15,033,158,255 bytes; the accepted transport is two payload parts plus the
  reviewed small members.
- The exact compiled 25-contig GRCh38 sequence index is built and qualified.
  Its mmap member is 772,091,760 bytes and its measured pinned-Zstandard
  representation is 656,781,805 bytes. Reuse the retained bundle; do not
  rebuild it or distribute its raw NCBI FASTA input.
- The exact selected GENCODE domains member is built and qualified. It is
  6,703,320 bytes, with SHA-256
  `714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`,
  and its measured pinned-Zstandard representation is 3,933,486 bytes. Package
  these retained bytes later; do not regenerate them.
- The current authenticated ONNX bundle is built and qualified through
  raw-kernel evidence and normal-fixture variant-scoring parity. It totals
  33,871,613 bytes and passed all 45,756 PyTorch comparisons. The retained
  coordinator-owned production scorer qualification passed all 14 cases and
  21 ordered gene records. The bundle is not release-final until
  CPU-policy work decides whether reference/alternate batching requires a
  different graph contract.

The GRCh38 sequence index, mask, and model therefore have local qualified
content but no transport, XDG installation, compatible four-asset profile, or
GitHub release. The next publication must reuse the accepted compiled GRCh38
sequence index and mask bytes, freeze the final model identity, bind all four
derived runtime assets into one compatible profile, and pass clean-machine CLI
inference.
Compatible-profile activation must first close the recorded runtime asset
trust/durability blocker; publication must also close the release-process and
repository-security blockers. No raw Zenodo, NCBI, or GENCODE source input is a
Pangopup release asset. Original checkpoints are not installed runtime members;
their separate source-availability policy is a model-publication prerequisite.

## Established — pinned source ingestion contract

Six CC BY-attributed excerpts from the verified archive now make source
semantics executable. The fixture includes:

- ascending and descending source position order;
- each reference base and all three alternate bases;
- score zero, nonzero score, and relative positions at useful boundaries;
- two overlapping gene records for one genomic SNV if present in the source;
- malformed synthetic rows for header, grouping, contiguity, and range failures;
- both real `REF=N` alternate shapes and every real coordinate gap family.

`pangopup-build inspect` validates gzip members without materializing a file's
rows and emits a deterministic per-gene and corpus report. The fixture proves
correctness only; the complete source evidence in
[`artifacts/2026-07-20-full-dataset-entropy.md`](artifacts/2026-07-20-full-dataset-entropy.md)
drives compression and size decisions.

## Established — measured fixed 11-byte private v1

The retained fixture and report record exact round trips through every
historical candidate. A deterministic 134-gene real lab corpus compared
hierarchical direct, fixed 11-byte, Zstd/LZ4 at 1,024/2,048/4,096 loci, and fair
in-process Tabix. Fixed won the accepted speed-first priority after direct was
corrected to ranked zero-copy mmap lookup, and is the only hardened product
codec. The one-time candidate implementations and benchmark target have been
removed. Its reader maps the
artifact, opens without a payload-wide scan, uses a balanced overlap tree, and
validates ordinary payload only when touched. The complete artifact now has
retained warm open, lookup, CLI, and serialization measurements. Cold I/O
remains unmeasured because the host provided no defensible way to prove the
queried pages were nonresident.

## Established — full streaming build and certification

The production builder canonicalizes one gene at a time, spools the fixed-v1
payload and normalized primary reference to disk, certifies every ordinary
source reference against pinned RefSeq GRCh38.p14, and creates one immutable
three-file bundle only after complete offline verification. The canonical
manifest binds source/reference identities, exact member hashes, corpus counts,
attribution, and matching independent source/decoded logical-stream digests.

## Established — typed SNV lookup

The cheap bundle open scans all fixed metadata and exceptions but does not hash
members or traverse ordinary score payload. One long-lived typed provider owns
the mmap and safely serves filtered or all-overlap requests. The CLI opens once,
validates the complete batch, returns exact JSONL or table bytes, and reports
misses, source-reference ambiguities, mixed results, incompatible bundles, and
touched-payload corruption distinctly. Full hashing and payload scans remain an
explicit `pangopup-build verify` operation.

## Established — deterministic lookup transport

Package the executable and generated data separately. A historical
tar+Zstandard experiment measured 1,935,000,209 bytes and showed that one
lookup archive leaves too little headroom below GitHub's per-asset ceiling; tar
is not the accepted lookup transport. Instead compress only the exact
`scores.pgi` stream as one deterministic Zstandard frame, split it into ordered
1,000,000,000-byte parts bound by a canonical manifest, and reassemble the
unchanged installed fixed-v1 bundle. Local pack, integrity-only verify, and
semantically certified atomic unpack now implement that boundary. Transport
compression is removed at installation; the lookup path continues to map the
fixed representation.

## Established — Linux local installation and fast semantic regression

`pangopup assets install` consumes a caller-supplied transport under one
nonblocking lock, streams reconstruction once into private no-follow staging,
publishes immutable receipt-bound bundles, and atomically replaces the active
profile. Status probes the lock without waiting; lookup is lock-free and uses
the last active bundle. Reuse performs bounded metadata and cheap structural
validation without opening transport parts or scanning `scores.pgi`. A
source-derived 1,000-request fixture proves the real provider and seven CLI
batches against a direct-TSV oracle in normal tests and CI.

## Established — immutable public SNV release

`pangopup-build release prepare` binds the retained production receipt to the
strict transport inspection result and atomically emits the checked release
profile, byte-identical proof, checksums, and release notes without opening a
payload part. CI installs the exact pinned ripgrep needed by the executable
spec, and the exact publication-ready commit passed the closed public-hygiene
audit. The public `snv-grch38-v1` release contains the exact eight reviewed
assets, GitHub reports every size/digest and `immutable=true`, and bounded
unauthenticated reads plus the documented five-file manual path are proved.

## Established — pinned remote sync

`pangopup assets sync` uses the compiled exact release profile, sequentially
streams its five reviewed URLs into a private XDG cache, verifies size and
SHA-256, resumes only through an exact strong-ETag range response, atomically
publishes a closed cache transport, and feeds the shipped installer. It never
selects “latest.” Exact active reuse and `--offline` perform no network work;
lookup remains network-free.

## Established — upstream compatibility corpus

`pangopup-compat-v1` freezes 24 exact cases from Pangolin 1.0.2 commit
`5cf94b8`: 14 scored SNV/MNV/insertion/deletion cases, six closed rejection
cases, and four controlled post-processing cases. Exact checkpoint, RefSeq
GRCh38.p14, GENCODE v38, helper, and numeric-environment identities are bound
to the corpus. Normal gates use a bounded Rust inspector to replay typed raw
arrays, masking order, extrema, positions, and rendering without model assets.
The profile records helper inference at forced PyTorch `1/1` separately from
the auxiliary unmodified CLI witness's observed `1/16` execution.
Complete controlled vectors are pinned independently of replay. Future capture
authenticates the live helper and all tracked imported Pangolin Python modules,
and its atomic publisher distinguishes unpublished cleanup from a reported
post-publication parent-sync failure. The one-time capture is not a routine
validation command.

## Established — compact reference payload selection

One retained five-round run over the preserved six-contig RefSeq input selected
`acgt2-rle-v1` by speed: headline p50/p95 were 4,469/4,880 ns, compared with
16,272/18,366 for ASCII and 34,267/41,522 for IUPAC4. Historical reports and
decisions retain the exactness, corruption, page-trace, and ranking evidence.
The discarded candidate codecs, miniature, benchmark executable, bench, and
CLI have been removed from the compiled workspace; the independently hardened
production implementation is the maintained path.

## Established — production GRCh38 reference bundle and provider

`PGRREF01` hardens the selected two-bit/ambiguity-run payload independently of
the benchmark container. The registered production profile binds the exact
RefSeq GRCh38.p14 FASTA/report, all 25 required sequences, ignored-record and
logical sequence digests, exact member hashes, and NCBI attribution. A
caller-buffer `ReferenceProvider` owns one read-only mmap; cheap open validates
bounded structure and every ambiguity run without touching dense pages, and a
window performs no heap allocation. The authenticated streaming builder
privately hashes and decodes the complete stage before atomic publication.
Normal gates use a separate 25-contig synthetic profile. One retained full
build established the complete source identities, logical digest, contexts,
member size, and builder heap. After an observable latency failure exposed a
production decoder regression and per-query audit overhead, the production
reader adopted the selected aligned four-base decoder and constructor-only
audit scope without changing v1. Exhaustive bounded scalar equivalence and
focused page, ambiguity, corruption, concurrency, and zero-allocation controls
are green.

The single authorized optimized CPU0 reuse qualification passed every
unchanged threshold under contract `edaee037…b35af`: p50/p95 `1864/2084 ns`,
zero allocations, open heap `16060` bytes, zero dense bytes during open,
benchmark RSS `20938752` bytes, exact eight dense pages / page-count sum 20,
installed size `772096272` bytes, and pinned-Zstandard size `656781805` bytes.
All 25-contig logical identities remain unchanged. The original full build and
both failed qualification roots are preserved; no second full build occurred.
Reference transport/XDG/release is not part of this outcome.

## Established — exact GENCODE mask semantics and format selection

Ticket 012 defines the exact versioned/PAR identity, effective
`(gene_start,gene_end]` membership, contig-local filtering, exon boundaries,
and observed same-strand order that a compact GENCODE v38 member must preserve.
The historical private lifecycle authenticated the GTF, gffutils database,
Python/SQLite environment, canonical ordered export, and complete candidate
set. It retained 60,649 genes, 88,202 domains, and 591,404 boundaries on 25
primary contigs. Current normal gates use a pinned 880-byte domains member and
independently authored miniature query expectations without Python, gffutils,
production inputs, a writer, or a network request.

The single retained balanced comparison exhaustively certified interval-tree,
constant-membership domains, and binned postings, then selected `domains` at
the first p95 speed step. Headline p50/p95 were 171/331 ns, versus 241/401 and
241/431 ns. Every warmed lookup round allocated zero bytes. The exact report,
1,000-query manifest, identities, and limitations are retained in
[`artifacts/012-gencode-mask-format-selection.md`](artifacts/012-gencode-mask-format-selection.md).
The unselected formats and qualification lifecycle remain historical benchmark
evidence only; their compiled implementations have been removed. ADR 0013
promotes the exact selected domains member behind the production runtime API;
it is not yet an installable or published asset.

## Established — artifact-specific builder provenance

Future SNV and production-reference builds use separate, versioned domains over
small checked inventories of only their causal source and exact locked Linux
dependency evidence. The evidence is compiled into the builder; construction
does not inspect a checkout or invoke Cargo recursively. Discriminating tests
prove artifact-only inputs affect only their owner, shared causal inputs affect
both, and representative mask/candidate/sync/release/CLI inputs affect neither.

Checked legacy manifests remain readable with unchanged miniature payloads.
The SNV regression's six source members, reference, requests, `NOTICE`, and
`scores.pgi` stayed byte-identical; only manifest provenance and its copied
bundle ID changed. The reference member and notice also stayed byte-identical.
No production asset was opened or rebuilt. Ticket 015 later retired the
completed mask-only fingerprint without changing either artifact-specific
identity. See
[`artifacts/013-artifact-builder-provenance.md`](artifacts/013-artifact-builder-provenance.md)
and ADR 0012.

## Established — production GENCODE mask provider

`pangopup_index::mask` opens the exact retained 6,703,320-byte Ticket 012
domains member through one read-only mmap. It rejects the interval-tree and
binned-postings discriminators and exposes a `Send + Sync` provider with
caller-owned reusable storage, plus/minus slices, exact gene identities, and
boundary slices. Misses and errors clear output; warmed queries allocate
nothing with sufficient capacity.

ADR 0013 supersedes ADR 0011's separate-format requirement. Ordinary open
validates bounded header, section, and directory structure; queries validate
touched records; asset SHA-256 stays a later download/install responsibility.
No GTF, Python, SQLite, builder, copied production member, bundle, transport,
publication, or model work is in the runtime provider. The obsolete mask
builder and qualification surfaces have now been removed. The retained
1,000-query oracle matches exactly.

## Established — repository diet

The three bounded experiment-removal slices are complete. The production SNV,
mask, and reference readers, selected assets, focused production fixtures, and
durable evidence remain, while their discarded candidate codecs, writers,
benchmark executables, CLIs, and dedicated specs no longer compile. The
reference evaluator still coupled to the recorded production source fingerprint
has no adapter and remains until a separately reviewed provenance change can
remove it without silently changing asset identity. No further repository-diet
work is implied by this outcome.

## Established — authenticated raw CPU model kernel

`pangopup-model` authenticates a closed three-file
`pangopup-model-bundle-v1`, opens one ONNX Runtime 1.24.2 CPU session, and
returns twelve selected raw channels for bounded plus/minus A/C/G/T/N contexts
in genomic orientation. The exact checkpoint/channel mapping, graph metadata,
fixed thread policy, output shape/range, and descriptor-held model load are
enforced at runtime.

A locked evidence helper independently authenticates and executes each
checkpoint, records all 3,024 state tensors, and retains 45,756 selected-channel
`f32` values. The converted graph matched every value with maximum absolute
error `5.364418029785156e-7`. Normal gates execute real ONNX Runtime against a
tiny checked same-schema graph; they do not invoke Python, PyTorch, production
weights, or a network.

An informal same-host probe on the AMD Ryzen 7 5825U observed direct
twelve-checkpoint PyTorch raw-context inference at about 0.66–0.68 seconds p50
with its default thread settings and about 3.03–3.19 seconds p50 when forced to
`1/1`. The accepted Rust/ONNX Runtime `1/1` kernel measured about 2.22–2.33
seconds p50. The probe suggests Rust is faster than like-for-like single-thread
Python, but the current forced-single-thread Rust policy is substantially
slower than multithreaded PyTorch. Its command and raw output were not retained,
so it informs the roadmap but is not qualification or release evidence. These
are single raw-context calls, not complete variant, concurrent, CLI, or HTTP
latency. Production model delivery is not part of this outcome.

## Established — variant-level CPU scoring

`pangopup-engine` accepts an owned literal `Grch38Variant` and composes the
existing GRCh38 sequence provider, mask provider, and mutable raw CPU kernel.
It supports SNVs, equal-length MNVs, and left-anchored insertions/deletions up
to 100 bases without normalization. Fixed distance-50 contexts, exact REF
validation, A/C/G/T/N reference policy, all-gene mask queries, plus-before-minus
execution, dtype-aware indel reconciliation, first extrema, shared-array
masking, and exact hundredths are now one typed result boundary.

Normal tests replay all 14 modeled cases and 36 exact raw evaluations from the
two frozen trust roots, all six rejections, and all four controlled cases. A
3,026-byte receipt fixes exact post-ensemble array identities derived from the
independent kernel goldens; public masked scores, positions, and gene order
match the compatibility oracle. One checked synthetic ONNX integration uses
in-memory reference/mask providers. The coordinator-owned ignored harness
opened the accepted model and reference identities plus the descriptor-hashed
mask member and matched all 14 cases and 21 ordered gene records without
rebuilding an asset.

This outcome does not add lookup routing, delivery, caching, CLI product
output, CPU tuning, or HTTP.

## Next outcome — lookup-first routing and CLI model results

The route must try the precomputed SNV lookup first, use the model only for
supported misses/non-SNVs, and report exact route plus model, GRCh38 sequence
index, and mask provenance. Expose the same typed result through stable CLI
JSON and prove both the precomputed and modeled routes. This outcome must not
add a cache or HTTP boundary.

## Later outcome — measured CPU policy and evidence-gated acceleration

The established single-thread CPU kernel is the reproducible baseline, not the
selected production policy. With complete variant parity established, measure
bounded ONNX Runtime CPU thread policies first. Then measure reference/alternate
batching or scheduling, including strand work, because the current graph fixes
batch size one. Set the session/concurrency budget against the complete request
workload. Consider MPS/CUDA, alternative runtimes, quantization, or another
backend only if those CPU changes remain insufficient. Adopt a change only with
preserved behavior and a material measured benefit.

## Later outcome — evidence-gated model-result caching

Measure repeated complete model requests before adding a cache. If justified,
cache only complete model results under a key that includes the literal variant,
gene/masking context, model checkpoint, GRCh38 sequence-index and mask
identities, window, and inference parameters; prove bounds, concurrency,
corruption handling, and invalidation.

## Later outcome — one compatible installed runtime profile

Define one immutable compatibility profile that binds the independently
versioned SNV, model, compiled GRCh38 sequence index, and mask identities.
Install each asset into a private immutable store, then atomically select the
coherent tuple rather than four unrelated active pointers. Prove offline
restart, rollback, partial-upgrade failure, bounded provisioning/cancellation,
durable non-secret progress, and descriptor-held read-only opens. This outcome
transports the already qualified compiled GRCh38 sequence index without
rebuilding it or including the raw FASTA. It must precede production HTTP
readiness and close the recorded runtime asset trust/durability issue before
activation.

## Later outcome — release-ready asset publication

Freeze exact model, GRCh38 sequence index, and mask release identities only
after the complete variant scorer and CPU policy are selected. Close the
release-upload lifecycle and repository-security blockers; complete the
applicable advisory/license policy, dependency inventory, SBOM, provenance,
attribution, and immutable release review before public distribution. Publish
only the reviewed derived runtime assets—never their raw upstream inputs—then
prove pinned sync, clean-machine CLI inference, and offline restart from that
exact release. This outcome precedes HTTP and container publication.

## Later outcome — foreground HTTP service

Add a foreground `pangopup serve` HTTP process with stable batch JSON, bounded
requests, health/readiness/status endpoints, timeouts, backpressure, and clean
shutdown. Expose `pangopup status` as the CLI view of the same non-secret
runtime and asset identities.

## Later outcome — deployment

Add a minimal non-root Docker image and documented systemd example.
Docker, systemd, Kubernetes, or another external manager owns
start/stop/restart; Pangopup does not become its own process supervisor.

## Later outcome — production and release hardening

Measure concurrency, startup, resident memory, page faults, and tail latency.
Add structured logs, useful metrics, resource limits, read-only runtime posture,
dependency/license inventory, SBOM and provenance, signing where practical,
upgrade/rollback rules, and cleanup of superseded immutable assets. Re-run the
complete clean-machine acceptance proof for releases.

Before the next public model, compiled GRCh38 sequence index, mask, executable,
or container publication, close the recorded release-process and
repository-security issues. Those issues do not block local scoring or
packaging work.
Close the recorded maintainer-interface/documentation drift before adding more
public maintenance commands.

## Unknowns that require evidence

- the routed gene-filter and unified provenance boundary;
- the winning CPU thread and reference/alternate batching policy;
- whether repeated complete requests justify any result cache;
- whether MPS, CUDA, quantization, or another runtime adds material value;
- the service session-pool and backpressure shape;
- the final release grouping and compression for the GRCh38 sequence index,
  mask, and model;
- production resource limits derived from complete-request measurements.

These are intentional roadmap slots, not tickets. Do not select or implement
one before its prerequisite evidence exists.

## Explicitly outside Pangopup

- HGVS parsing and genomic/transcript/protein projection;
- gene descriptions, aliases, disease knowledge, or clinical interpretation;
- GRCh37 and liftover.

These are rolling outcome boundaries, not a ticket backlog or promises about
unsettled implementation details. Only the next coordinator-authored,
independently reviewed, bounded ticket is implementation scope.
