# Frontier

Updated: 2026-07-25

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
Private, feature-gated GENCODE-mask qualification has authenticated the exact
upstream semantics and selected constant-membership domains from three mmap
candidates in one retained full-source comparison. The exact retained domains
member is now available through a production, domains-only typed mmap provider;
its delivery and model integration remain future. Model inference and HTTP
remain future. The
production reference bundle, authenticated builder, and typed mmap provider are
established; its transport, installation, and publication remain future
delivery work. Future SNV and reference builds now use separate causal
source/dependency fingerprints; their existing production assets remain
immutable and readable.

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

The checked fixture round-trips exactly through every candidate. A deterministic
134-gene real lab corpus compared hierarchical direct, fixed 11-byte, Zstd/LZ4
at 1,024/2,048/4,096 loci, and fair in-process Tabix. Fixed won the accepted
speed-first priority after direct was corrected to ranked zero-copy mmap lookup,
and is the only hardened product codec. Its reader maps the
artifact, opens without a payload-wide scan, uses a balanced overlap tree, and
validates ordinary payload only when touched. The complete artifact now has
retained warm open, lookup, CLI, and serialization measurements. Cold I/O
remains unmeasured because the host provided no defensible way to prove the
queried pages were nonresident.

## Established — full streaming build and certification

The production builder canonicalizes one gene at a time, spools the fixed-v1
payload and normalized primary reference to disk, certifies every ordinary
source reference against pinned RefSeq GRCh38.p14, and publishes one immutable
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

Three closed sequence payloads now share one benchmark-only container and a
checked miniature oracle. Focused tests prove exact IUPAC decoding,
caller-buffer reads, corruption rejection, deterministic logical page traces,
and the pure ranking rule. One retained five-round run over the preserved
six-contig RefSeq input selected `acgt2-rle-v1` by speed: headline p50/p95 were
4,469/4,880 ns, compared with 16,272/18,366 for ASCII and 34,267/41,522 for
IUPAC4. This selects the payload; it does not claim a production reference
reader or asset.

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
The private feature-gated lifecycle authenticated the GTF, gffutils database,
Python/SQLite environment, canonical ordered export, and complete candidate
set. It retained 60,649 genes, 88,202 domains, and 591,404 boundaries on 25
primary contigs. Normal gates use an independent miniature without Python,
gffutils, production inputs, or a network request.

The single retained balanced comparison exhaustively certified interval-tree,
constant-membership domains, and binned postings, then selected `domains` at
the first p95 speed step. Headline p50/p95 were 171/331 ns, versus 241/401 and
241/431 ns. Every warmed lookup round allocated zero bytes. The exact report,
1,000-query manifest, identities, and limitations are retained in
[`artifacts/012-gencode-mask-format-selection.md`](artifacts/012-gencode-mask-format-selection.md).
The two unselected `PGMBEN01` members and qualification lifecycle remain
benchmark evidence. ADR 0013 promotes the exact selected domains member behind
the production runtime API; it is not yet an installable or published asset.

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
No production asset was opened or rebuilt, and the exact Ticket 012 mask
fingerprint remains unchanged. See
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
No GTF, Python, SQLite, builder, copied member, bundle, transport, publication,
or model work occurred. The retained 1,000-query oracle matches exactly.

## Current outcome — repository diet

Remove the completed candidate, qualification, bespoke publication, and
source-fingerprint machinery that is no longer needed by the runtime product,
while preserving the selected readers, fixtures, release compatibility, and
durable evidence. This is a roadmap slot, not a drafted or authorized ticket.

## Next outcome — CPU runtime, safe model representation, and inference

Choose the CPU tensor runtime together with the checkpoint representation
rather than converting weights before a consumer exists. Authenticate every
raw checkpoint before parsing; pin tensor names, shapes, dtypes, ordering, and
per-tensor identities; define numeric tolerance and public-rendering acceptance;
and add discriminating internal goldens beyond the end-to-end corpus. Then
implement CPU inference against explicit compatible reference, mask, and model
paths and prove the retained compatibility corpus. The supported non-SNV input
and normalization contract must be explicit before it becomes a public or
cache identity.

Only after CPU compatibility is established should Pangopup measure
accelerator backends such as MPS or CUDA, alternative runtimes, or
quantization. Adopt one only with the same defined behavior and measured
end-to-end benefit. Keep every precomputed SNV hit authoritative whenever it
exists.

## Later outcome — lookup-first routing and evidence-gated caching

Try SNV lookup first, route lookup misses and supported non-SNVs to inference
through one typed API, and report route and asset provenance. Measure repeated
model workloads before adding a cache. If justified, cache only complete model
results under a key that includes normalized variant, gene/masking context,
model checkpoint, reference/mask identity, window, and inference parameters;
prove bounds, concurrency, corruption handling, and invalidation.

## Later outcome — one compatible installed runtime profile

Define one immutable compatibility profile that binds the independently
versioned SNV, model, reference, and mask identities. Install each asset into a
private immutable store, then atomically select the coherent tuple rather than
four unrelated active pointers. Prove offline restart, rollback, partial-
upgrade failure, bounded provisioning/cancellation, durable non-secret
progress, and descriptor-held read-only opens. This outcome also transports
and publishes the already qualified reference without rebuilding it. It must
precede production HTTP readiness.

## Later outcome — foreground service and deployment

Add a foreground `pangopup serve` HTTP process with stable batch JSON, bounded
requests, health/readiness/status endpoints, timeouts, backpressure, and clean
shutdown. Expose `pangopup status` as the CLI view of the same non-secret
runtime and asset identities. Add a minimal non-root Docker image and
documented systemd example.
Docker, systemd, Kubernetes, or another external manager owns
start/stop/restart; Pangopup does not become its own process supervisor.

## Later outcome — production and release hardening

Measure concurrency, startup, resident memory, page faults, and tail latency.
Add structured logs, useful metrics, resource limits, read-only runtime posture,
dependency/license inventory, SBOM and provenance, signing where practical,
upgrade/rollback rules, and cleanup of superseded immutable assets. Re-run the
complete clean-machine acceptance proof for releases.

Before the next public model, reference, executable, or container publication,
close the recorded release-process and repository-security issues. Those
issues do not block local mask-format selection.

## Explicitly outside Pangopup

- HGVS parsing and genomic/transcript/protein projection;
- gene descriptions, aliases, disease knowledge, or clinical interpretation;
- GRCh37 and liftover.

These are rolling outcome boundaries, not a ticket backlog or promises about
unsettled implementation details. Only the next coordinator-authored,
independently reviewed, bounded ticket is implementation scope.
