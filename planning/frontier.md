# Frontier

Updated: 2026-08-04

## Current boundary

Repository onboarding, source ingestion, format selection, full-corpus build
and certification, and typed SNV lookup are established. Pangopup is standalone
open-source software. Its shipped CLI accepts an explicit GRCh38 SNV and
optional gene filter and returns all matching source records by default from an
explicit fixed-v1 bundle or the active Linux user-data installation. Speed
leads memory and download size. Deterministic local transport, atomic install,
status, active discovery, cheap reuse, and the fast 1,000-case regression are
established. Immutable publication and pinned resumable remote sync are also
complete. Sync now uses progress-reset blocking reads, typed four-attempt
transient retry, exact committed byte accounting, and terminal/forced bounded
progress without changing final JSON output. The strict upstream Pangolin
compatibility corpus is established.
Retained GENCODE-mask qualification evidence authenticates the exact upstream
semantics and records the selection of constant-membership domains from three
mmap candidates in one full-source comparison. The one-time candidate and
qualification source has been removed. The exact retained domains
member is now available through a production, domains-only typed mmap provider
and local transport and is public in `runtime-grch38-v1`; typed pinned sync is
shipped, and combined CLI provisioning/status is established. An authenticated combined
ONNX representation and qualified single-owner CPU kernel now return the
twelve raw Pangolin channels. `pangopup-engine` now composes the sequence,
mask, and kernel providers into compatible masked distance-50 scores for the
supported literal GRCh38 subset. Typed lookup-first routing and installed or
explicit-path CLI model output are established. A complete-request CPU comparison retains
sequential `1/1` as the portable ordinary default and records fixed `8/1` as
this host's frontier result. The corrected reference/alternate batching
comparison retained singleton after both policies exceeded the drift limit and
neither candidate met the independent replacement gates. Persistent exact
SQLite result reuse, one canonical four-asset production compatibility
profile, offline coherent XDG installation/activation, and lazy held-descriptor
runtime consumption are established. The explicit `--model-only` route now
bypasses precomputed lookup for complete batches and can consume either the
activated model-side profile without the SNV object or a self-sufficient
explicit model/reference/mask tuple. It reuses the shipped exact SQLite cache
and modeled output contract. Deterministic local packaging of the three
model-side assets is established. The exact 691,884,908-byte read-only
`runtime-grch38-v1` release stage has been prepared from reviewed, remotely
green code, with byte equality and all declared checksums qualified. Its
complete exact GPL preferred-source supplement is also retained and qualified;
the exact 15-asset set is now public and immutable. Pinned typed model-side
sync and combined CLI provisioning are shipped; the released executable passed
clean-container installation and inference. A foreground HTTP service now
opens the coherent installed profile once, keeps lookup/cache hits outside a
fixed bounded FIFO model queue, and exposes stable score/health/status JSON.
One retained
service-partition experiment now compares all ten equal-budget worker/thread
shapes for 1, 2, 4, and 8 physical cores. It selects `1×1`, `1×2`, `1×4`, and
`2×4` on the retained Ryzen host,
preserves ADR 0017's portable `1×1` default elsewhere, and demonstrates that
separately opened warm SNV lookup remains fast while a model batch is in flight.
The service defaults to portable `1×1`, 16 waiting requests, immediate 429
backpressure, and graceful signal drain. A pinned distroless Dockerfile now
packages that same executable without assets, runs as UID/GID 65532, and is
qualified natively on AMD64 and ARM64 without publication authority.
The compiled production GRCh38 sequence-index bundle, authenticated builder,
and typed mmap provider are established; its local transport, installation,
and immutable public release are shipped. Future SNV and sequence-index builds use
separate causal source/dependency fingerprints; their existing production
assets remain immutable and readable.
The checked repository side now has a read-only workflow token, full-SHA action
references, digest-authenticated maintenance tools, and an advisory, license,
bans, and source policy in `make lint`. The policy preserves the
builder-causal versionless workspace path edges after cargo-deny 0.19.4 proved
it cannot exempt them separately from registry wildcard linting for publishable
workspace crates; current locked sources remain only workspace paths and the
canonical crates.io registry. Ticket 032 also applied and verified the
independently writable live GitHub settings: read-only Actions defaults,
disabled Actions PR approval, Dependabot security updates, three
secret-scanning controls, unbypassed history protection, and the
administrator-bypassed pull-request/`gate` contribution policy. Validity checks
remain disabled and optional; they are an alert-triage feature, and Pangopup
has no configured repository secrets or open secret alerts. The repository
security issue is closed and no organization authority is required for asset
publication.

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
  21 ordered gene records under the selected ordinary CPU policy. The bundle
  is the selected graph after the corrected batching comparison retained it.

The GRCh38 sequence index, mask, and model therefore have local qualified
content, one exact compatible four-asset profile, one deterministic common
local transport, and one qualified 15-asset publication set. The transport
uses three independent frames, reconstructs accepted bytes exactly, and never
touches the 15 GB SNV payload. The publication set adds complete exact
Pangolin/Pangopup preferred-source archives and the standalone GPL license. It
is now the immutable public `runtime-grch38-v1` GitHub release with a pinned
typed remote-sync policy and combined top-level CLI provisioning. Ticket 038's
clean-machine CLI inference qualification is complete. No raw
Zenodo, NCBI, or GENCODE source input is a Pangopup release asset. Original
checkpoints are preserved as preferred source, not installed runtime members.

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
The custom coordinator upload wrapper and its process/lease/signal supervisor
have been deleted. Pangopup retains deterministic local preparation but ships
no release uploader; future public effects use a separately reviewed
draft-first lifecycle around the authenticated official `gh` executable.

## Established — pinned remote sync

`pangopup sync` composes the compiled exact SNV and runtime release profiles,
sequentially streams their reviewed URLs into private XDG cache, verifies size and
SHA-256, resumes only through an exact strong-ETag range response, atomically
publishes a closed cache transport, and feeds the shipped installer. It never
selects “latest.” Exact active reuse and `--offline` perform no network work;
lookup remains network-free. Healthy long streams are no longer capped by the
completed response-header timer; transient failures retry visibly, and CLI
progress is terminal-auto with explicit `--progress` and `--quiet` controls.

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
promotes the exact selected domains member behind the production runtime API.
It is installable through the shipped coherent offline profile and packaged by
the shipped local transport; it is not yet a published or remotely provisioned
asset.

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
benchmark executables, CLIs, and dedicated specs no longer compile. Reference
byte production and post-build certification now live in separate modules, so
retained evaluation policy is no longer part of future byte-producing
provenance. No further repository-diet work is implied by this outcome.

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
seconds p50. The probe suggested Rust was faster than like-for-like
single-thread Python but said nothing reliable about complete variants. Its
command and raw output were not retained, so it is historical diagnosis rather
than qualification or release evidence. The later retained complete-request
policy outcome is the authority for current CPU behavior. Production model
delivery is not part of this outcome.

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

Ticket 019 itself did not add lookup routing, delivery, caching, CLI product
output, CPU tuning, or HTTP; the following outcome has since added routing and
CLI model output.

## Established — lookup-first routing and CLI model results

`pangopup-engine` now owns one typed lookup-first router. Authoritative
precomputed records and source-reference ambiguities win; with caller-enabled
fallback, a pure SNV miss or supported non-SNV is completed by one scorer
bound to the exact reference, identified mask, and model identities. Model
masking still sees all containing GENCODE genes before an optional stable-gene
filter is applied.

The CLI enables fallback through the activated installed profile or the
complete explicit `--reference-bundle`/`--mask`/`--model-bundle` tuple.
Authoritative hits inspect neither runtime profile, cache, nor model-side
assets. An explicit tuple wins, and an explicit SNV `--bundle` never borrows
installed model-side assets. Mixed/model batches lazily open reference, mask,
and model once, buffer the complete batch, and emit stable modeled JSONL or the
existing table shape.

Normal tests use one checked synthetic 25-contig route reference, a 260-byte
domains member, the miniature authenticated ONNX bundle, and the established
SNV fixture. They exercise the real PGRREF01, mask mmap, ONNX Runtime, scorer,
router, renderer, and CLI without production assets. A separate benchmark
derives 994 authoritative hits from the frozen regression and repeats its first
six hits only for the 1,000-row diagnostic; the original 1,000-case regression
remains unchanged and is not mislabeled hit-only.

## Established — measured complete-request CPU policy

One retained comparison ran eight bounded ONNX Runtime policies through exact
M09 one-strand and M10 two-strand `VariantScorer` requests. Every candidate
matched the frozen public records and stayed below twice the sequential `1/1`
RSS. Fixed sequential `8/1` was this host's frontier winner at p50 about 1.05
seconds for M09 and 2.64 seconds for M10, with a worst baseline ratio of 0.305.

Affinity-aware sequential `auto/1` improved M09 but did not improve M10, so it
failed the portable-default gate. Ordinary `ModelKernel::open` therefore
remains sequential `1/1`; fixed `8/1` is host characterization for later
service scheduling, not user configuration or a universal default. A fresh
ordinary-policy rerun and the retained 14-case/21-record production
qualification passed exact accepted identities. ADR 0017 and
[`artifacts/021-measured-cpu-policy.md`](artifacts/021-measured-cpu-policy.md)
retain the policy and evidence.

## Established — measured service model partition

Thirty fresh processes compared all integer model-session/intra-op-thread
partitions of 1, 2, 4, and 8 physical cores. All ten candidates reproduced the
reviewed cases, stayed below 1 GiB RSS, and retained valid idle/loaded
1/10/100-SNV measurements. The selected host mappings are `1×1`, `1×2`,
`1×4`, and `2×4`. Most multi-session candidates failed the 125-percent
single-request latency guard; `2×4` passed it and won the eight-core throughput
comparison.

Each selected mapping then reproduced all 14 scored compatibility cases and
21 ordered records exactly. ADR 0024 and
[`artifacts/040-service-scheduling.md`](artifacts/040-service-scheduling.md)
retain the decision and evidence. This does not change the portable `1×1`
default or implement a scheduler. The service still owns bounded admission,
backpressure, dispatch, SQLite connections, fill coalescing, and failure
fan-out, and must keep SNV lookup outside model admission.

## Established — corrected reference/alternate graph batching

The first candidate run is retained as historical evidence but cannot select a
graph: the v2 manifests declared dynamic batch/length axes that were applied
only to final graph metadata, not passed into PyTorch's exporter. Corrected v2
conversion gave the exporter those axes, changed both candidate identities, and
repeated the complete raw/public/performance matrix. Both policies were
inconclusive from singleton drift; independently, neither candidate met the
M09/M10 improvement and no-regression gates. Ordinary dispatch and the accepted
model identity remain singleton. Both tiny checked candidates remain because
retained experimental execution requires normal real-ORT coverage.

## Established — persistent exact model-result caching

Successful complete unfiltered model results now persist in bundled SQLite
under a full scoring-identity key. The default 10,000-entry database survives
restarts, uses deterministic insertion/update-order eviction, keeps valid hits
read-only, safely discards malformed rows, and never caches precomputed hits,
rejections, failures, or rendered bytes.
Bounded manifest/mask admission lets a reopened hit bypass dense-reference
authentication and all ONNX initialization/inference. Coordinator production
measurements remain the final host-specific acceptance evidence.

## Established — one compatible installed runtime profile

The path-free authority binds the independently versioned SNV, model, compiled
GRCh38 sequence index, mask, and scoring policy. The Linux installer reuses the
active certified SNV object, streams the three fallback assets once into a
private immutable store, and atomically selects one profile. Bounded status,
idempotent reuse, shared-lock behavior, and transition failure/retry are
covered by miniature tests. Lookup/runtime discovery and held provider
consumption are also shipped. Remote provisioning, rollback/GC, and
publication are deliberately separate later outcomes.

## Established — deterministic model-side local transport

One canonical ten-file directory packages the exact runtime profile,
byte-exact bounded model/reference metadata and notices, a checked GENCODE v38
mask notice, and independent pinned-Zstandard frames for `model.onnx`,
`reference.pgr`, and `domains.pgm`. The manifest declares the other nine
members; its canonical-byte SHA-256 is the transport identity. Pack
authenticates internal profile consistency without opening the SNV member.
Verify streams stored and reconstructed identities without writing outputs.
Unpack decodes each frame once into private staging and atomically publishes a
byte-exact layout. Public upload, download, and install-from-transport policy
remain later outcomes.

## Established — deterministic runtime release preparation

`pangopup-build runtime-release prepare` accepts only the exact retained
production transport and a lowercase 40-character target commit. It verifies
and copies the ten compressed transport members without rebuilding,
recompressing, or materializing decoded payloads. Verification streams each
frame through one bounded decode before the same held stored members are
copied; the command then atomically publishes a private read-only stage
containing those bytes, a canonical runtime release profile,
`SHA256SUMS`, and release notes. The profile binds the model/reference/mask/SNV
identities, exact immutable URLs, upstream preferred model source, and
target-commit converter paths. Normal tests use a hidden miniature contract;
the public CLI rejects nonproduction transports. The complete preferred-source
supplement and exact draft-first operation are now qualified without a GitHub
mutation. The coordinator subsequently published those exact bytes as the
immutable `runtime-grch38-v1` release.

## Established — reference reader/provenance boundary

Installed-profile consumption exposed an existing coupling:
`pangopup-index::reference` mixes the runtime mmap reader with the
byte-producing writer, while `pangopup.reference-builder-source.v1` hashes the
whole module. Do not work around that coupling with a second PGRREF01 decoder
or a public raw-file trust bypass.

Ticket 027 mechanically separated builder-causal wire/writer inputs from
reader-only code, defined
`pangopup.reference-builder-source.v2` over only byte-producing inputs, and
added one shared held-descriptor reader entry point behind opaque installed
admission. Miniature v1/v2 payload and notice bytes are identical, the compiled
25-contig projection and causal inventory are independently checked, and the
production/profile identities remain statically pinned. The retained 772 MB
production member was not opened, copied, repacked, or rebuilt.

## Established — installed-profile consumption

Ordinary lookup binds fallback admission to the exact already-open active SNV
identity. A model-required request admits the compatible activated model,
reference, and mask through held descriptors; an authoritative hit never
inspects that state. Explicit fallback remains all-or-nothing and takes
precedence. Reopened SQLite hits repeat bounded identity admission but avoid
dense reference/model reads and ONNX initialization.

Explicit model-only admission instead validates the canonical activated
profile and opens only its model-side members; it neither requires nor opens
the active SNV object. A complete explicit model tuple needs no installation.

## Established — immutable model-side asset publication

The exact model, GRCh38 sequence index, mask, target release identity,
attribution, and preferred modification source are frozen by deterministic
release preparation. The reviewed operation publishes only that 15-asset set—
never raw NCBI, GENCODE, or Zenodo inputs—using authenticated official `gh`, a
non-public draft, remote inventory/digest comparison after every upload, and
one-way immutable finalization. Release `runtime-grch38-v1` now contains the
exact reviewed 15 assets and reports `immutable=true`; its tag points to the
pinned target. Pinned typed runtime sync and direct one-pass installation are
now established. Combined SNV/runtime CLI provisioning/status is established;
the released executable passed clean-container installation and inference.
Offline restart and service-level qualification remain with HTTP/container
work.

Repository and dependency controls do not replace release evidence. Remote
inventory/digest comparison, immutable finalization, model-side runtime sync,
and clean-machine inference are tied to the exact bytes and commit actually
published. Container dependency inventory and SBOM remain with
their later publication, not this data-artifact release.

## Established — foreground HTTP service

`pangopup serve` is one foreground process with stable ordered batch JSON,
64-KiB bodies, 100-variant batches, at most 10 uncached model variants per
request, health/readiness/status routes, bounded FIFO admission, fail-closed
worker loss, and graceful SIGINT/SIGTERM drain. It keeps authoritative mmap and
completed SQLite hits outside model admission. It has no daemon commands,
durable jobs, polling, in-memory LRU, automatic sync, authentication, TLS, or
server-side inference timeout.

## Later outcome — deployment

The minimal non-root Docker image is shipped and Apple Silicon qualification
proved native Linux/ARM64 build, resumable provisioning, offline reuse, lookup,
model fallback, persistent SQLite reuse, HTTP service operation, and named-
volume persistence. It also exposed one deterministic deployment-contract bug:
combined `pangopup status` requested write access to `.install.lock`, so the
documented read-only data-volume command failed even though lookup and service
operation worked with the same mount. Reviewed Ticket 043 fixed that defect:
installed-state observation now opens the existing lock authority read-only
and holds a shared guard for the coherent SNV/runtime snapshot, while the
installer retains its exclusive guard. Generic crate tests cover missing
authority, hostile locks, concurrent observers, installer contention, partial
state, and fixture-only combined readiness. The literal non-root,
network-disabled final-image command with `/var/lib/pangopup:ro` passed as its
acceptance proof.

Focused runtime help is now established. The six public leaves and both asset
namespaces accept exact trailing `-h`/`--help`, render from one checked catalog,
and dispatch before paths, assets, model/cache initialization, or service
startup. Root help remains byte-compatible with the pre-ticket executable, and
the stripped non-root final image proves the full matrix without networking or
mounted assets.

Resilient synchronization is established. Active reads use progress-reset
timeouts and four bounded transient attempts, safe partials resume across
processes, and optional phase/byte progress remains separate from the unchanged
final JSON. The Apple M5 Max retest at exact commit `e5d7d1a` proved focused
help, read-only status without mutation, an exact 263,438,336-byte resumed
prefix, monotonic progress matching final totals, lookup/model/cache behavior,
and the HTTP service on native Linux/ARM64 Docker. It did not deliberately
induce a within-process retry. The retained Apple friction is ONNX Runtime
1.24.2's unknown-CPU-vendor warning on stderr.

Ticket 048 then isolated the warning's cause with matched source-built ONNX
Runtime 1.28 images on the Apple M5 Max Docker host. The baseline pinned
cpuinfo reproduced the exact 76-byte warning, while the otherwise-identical
first Apple-aware cpuinfo revision produced empty stderr and byte-identical
version stdout. This confirms the dependency-pin hypothesis only. Production
still uses the qualified 1.24.2 runtime. The accepted decision is to wait for
an upstream ONNX Runtime release carrying Apple-aware cpuinfo instead of
maintaining a custom native runtime solely to suppress a harmless warning.

The root README is now a bounded first-use guide rather than an engineering
ledger. It separates immutable public `v0.1.0` behavior from current `main`,
documents sync, CLI, HTTP, Docker/Apple Silicon, update, and independently
selectable manual removal, and links detailed architecture and planning. A
local static executable spec keeps its commands, endpoints, XDG paths, disk
guidance, security boundary, and attribution present without networking,
Docker, large downloads, or destructive removal.

The compact public `v0.2.0` executable release is now immutable and Latest.
Its exact-commit package, anonymous release files, tagged curl installer,
lookup/model/cache behavior, focused help, sync output, and foreground HTTP
surface passed the clean production qualification. The next bounded outcome is
reviewed multi-architecture container publication and clean-machine acceptance.

Docker, systemd, Kubernetes, or another external manager owns
start/stop/restart; Pangopup does not become its own process supervisor.

## Later outcome — production and release hardening

Measure concurrency, startup, resident memory, page faults, and tail latency.
Add structured logs, useful metrics, resource limits, read-only runtime posture,
dependency/license inventory, SBOM and provenance, signing where practical,
upgrade/rollback rules, and cleanup of superseded immutable assets. Re-run the
complete clean-machine acceptance proof for releases.

Before a replacement model-side runtime, executable, or container publication,
repeat the applicable reviewed publication lifecycle and release-specific
checks. The repository-security and deleted-uploader issues are closed, and
the first model-side runtime publication is complete. These requirements do
not block local scoring or packaging work.
The maintainer-interface/documentation drift is closed: `pangopup-build`
dispatch and successful root, namespace, and leaf help share one checked
catalog, version reporting is conventional, and executable acceptance preserves
the established operational error bytes and streams. New maintenance commands
must enter that catalog and its executable coverage.

## Unknowns that require evidence

The Linux x86_64 direct-binary installer and deterministic six-file release
are shipped as immutable
[`v0.2.0`](https://github.com/genomoncology/pangopup/releases/tag/v0.2.0).
The installer verifies and smoke-tests an atomic
replacement while preserving an existing binary on failure, and does not
download data or mutate `PATH`. The read-only exact-commit workflow qualifies a
private artifact against pinned Ubuntu 24.04 and GLIBC 2.39. Ticket 038's
public installer qualification passed through clean production sync, exact SNV
and model oracles, public verification, and a tagged non-root install. HTTP and
the native AMD64/ARM64 Dockerfile are shipped; a registry image is not yet
published. Other package managers remain later roadmap slots. Read-only status, focused runtime help, resilient
resumable synchronization feedback, and the exact-commit Apple Silicon retest
are complete. Ticket 047 proved that stock `ort` rc.13 / ONNX Runtime 1.28.0
still emits the same Apple Docker warning. Ticket 048's matched source-built
A/B probe then confirmed that advancing only cpuinfo to its first Apple
Linux-aware revision removes the warning on that Mac. `main` remains on the
qualified 1.24.2 runtime, and the accepted maintenance decision is to wait for
an upstream release rather than carry a custom runtime for warning suppression.
The compact first-use README and immutable `v0.2.0` executable release are
established. Reviewed multi-architecture container publication follows.

Ticket 038's completed publication includes a credential-free operation/evidence
record, exact reviewed release notes, an independently
derived M09 JSONL oracle, and checked production-qualification helpers. The
helpers run online/offline sync, ready status, the seven retained 1,000-SNV
batches, and M09 in an isolated environment, then compare results without
walking the production assets. Release `v0.1.0` is public and immutable at the
exact reviewed commit.

Repeated complete requests and asset grouping are no longer unknowns. The
shipped persistent SQLite cache retains 10,000 complete model results by
default. The model, compiled GRCh38 sequence index, and mask are grouped in the
immutable public `runtime-grch38-v1` release, separately from the immutable
`snv-grch38-v1` release.

- whether MPS, CUDA, quantization, or another runtime adds material value;
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
