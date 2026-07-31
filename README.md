# Pangopup

Pangopup is a standalone GPL-3.0 Rust project for high-performance,
Pangolin-compatible splice scoring on GRCh38 genomic variants. Today it ships
an exact precomputed-SNV library and CLI, a production mmap GRCh38 sequence
provider and authenticated builder, deterministic local release
transport tooling, atomic Linux/XDG installation, and pinned resumable sync of
the immutable public SNV transport. It also ships a frozen, independently
captured upstream compatibility corpus plus an authenticated CPU ONNX kernel
that returns the twelve raw Pangolin channels. It now also ships a
variant-level Rust scorer and lookup-first CLI routing for the supported
literal GRCh38 SNV, MNV, insertion, and deletion subset. Callers may supply one
explicit local reference/mask/model set for fallback and receive exact modeled
JSONL or table output. Successful complete model results persist in a bounded
SQLite cache and are reused across process restarts. One canonical, path-free
runtime profile now binds the exact qualified SNV, model, reference, mask, and
scoring-policy tuple. Linux can now install the three fallback assets beside
an existing certified SNV object and atomically select that coherent profile.
Lookup now consumes the activated installed profile lazily when an SNV miss or
supported non-SNV needs inference; explicit fallback paths remain an override.
The exact derived model-side release and its GPL preferred source are public
as the immutable
[`runtime-grch38-v1` release](https://github.com/genomoncology/pangopup/releases/tag/runtime-grch38-v1).
Pinned model-side synchronization and combined top-level CLI provisioning are
shipped. A checksum-verifying Linux x86_64 direct-binary installer and
read-only exact-commit release packaging workflow are prepared, but no public
executable exists until Ticket 038. The HTTP service remains unimplemented. Ticket 012 has now
authenticated the complete GENCODE v38 mask semantics and selected the
constant-membership `domains` encoding by a retained speed-first comparison.
Ticket 014 promotes the exact selected bytes behind a domains-only production
mmap provider without rebuilding or renaming the format. The one-time
candidate writer and qualification program have since been removed; their
retained reports and exactness corpus remain as historical evidence. Compiled
GRCh38 sequence-index, mask, and model bytes are frozen in the public
`runtime-grch38-v1` publication set; pinned typed download is shipped. The
completed three-format reference experiment
has likewise been removed from the compiled workspace;
its retained selection evidence led to the independent production `PGRREF01`
reader, provider, and builder that remain. Future SNV and GRCh38 sequence-index
builds now also carry separate, artifact-local source provenance, so unrelated
tooling changes no longer churn their identities while already
published/qualified v1 assets remain readable. The completed SNV-format
comparison has also been removed from the compiled workspace; its retained
report explains why the production fixed-v1 implementation remains selected.

The target service will answer each request through one of two paths:

1. **SNV lookup:** return an exact precomputed Pangolin result from a compact,
   memory-mapped index.
2. **Model scoring:** score a supported literal variant against the activated
   installed GRCh38 sequence, splice-mask, and model providers, or one complete
   explicit override tuple.

An SNV is a single-nucleotide variant: one reference base replaced by one
alternate base. The published Zenodo dataset already contains masked Pangolin
scores for every SNV it covers, so recomputing those values with the neural
network would be slower and could introduce small numeric differences.

## How one request works

Pangopup accepts an explicit GRCh38 genomic variant:

```json
{
  "assembly": "GRCh38",
  "contig": "17",
  "position": 43106534,
  "ref": "C",
  "alt": "A"
}
```

The shipped CLI performs both paths below from an activated installation, or
when a complete explicit fallback set is supplied:

```text
GRCh38 chromosome + position + REF + ALT
                    |
                    v
             validate the variant
                    |
          +---------+----------+
          |                    |
      SNV index hit       no index record
          |               or a non-SNV
          v                    |
 exact published score         v
                       supported by model?
                            |       |
                           yes      no
                            |       |
                            v       v
                       model score  typed no-score result
```

The response identifies whether its values came from `precomputed` lookup or
`model` inference. Because genes can overlap and Pangolin masking is
gene-specific, one genomic variant can return several source-gene score records.
A caller may provide an optional Ensembl gene filter; Pangopup never guesses a
single best gene.

Lookup-only use remains valid and preserves legacy SNV misses:

```text
pangopup lookup --bundle <SNV_BUNDLE> \
  --variant GRCh38:17:43106534:C:A
```

With the coherent runtime profile installed, no model-side paths are needed:

```text
pangopup lookup --variant GRCh38:17:43106534:C:CA
```

To override the installed runtime, supply all three local model inputs
together:

```text
pangopup lookup --bundle <SNV_BUNDLE> \
  --variant GRCh38:17:43106534:C:CA \
  --reference-bundle <REFERENCE_BUNDLE> \
  --mask <DOMAINS_PGM> \
  --model-bundle <MODEL_BUNDLE>
```

The default cache is
`${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/model-results.sqlite3` and retains the
10,000 most recently inserted or explicitly updated model results. Valid hits
are read-only and do not refresh that write order. Override it with
`--model-cache <ABSOLUTE_PATH>` and change the bound with
`--model-cache-max-entries <POSITIVE_INTEGER|unlimited>`. Matching
`PANGOPUP_MODEL_CACHE` and `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` environment
variables are available; explicit flags win. Cache options are valid on the
installed route, but an explicit `--bundle` requires the complete explicit
fallback tuple. Authoritative SNV hits inspect neither SQLite nor model assets.

These flags identify already-built local assets. Maintainers can authenticate
the exact jointly qualified tuple and create its small compatibility authority:

```text
pangopup-build runtime-profile prepare \
  --snv-bundle <SNV_BUNDLE> \
  --model-bundle <MODEL_BUNDLE> \
  --reference-bundle <REFERENCE_BUNDLE> \
  --mask <DOMAINS_PGM> \
  --output <NEW_PROFILE_JSON>
```

This command reads no SNV score payload, initializes no model session, and
performs no download, installation, activation, or publication. Its output can
be installed offline with `pangopup assets runtime install`; bounded
inspection is included in `pangopup status`. Installation streams the
model, compact reference, and mask once into private immutable XDG data and
reuses the installed SNV bundle without reading its 15 GB score member.
The same three local inputs can now be packaged, verified, and reconstructed
without publishing them:

```text
pangopup-build runtime-transport pack --profile <PROFILE> --model-bundle <MODEL_BUNDLE> --reference-bundle <REFERENCE_BUNDLE> --mask <DOMAINS_PGM> --output <ABSENT_DIR>
pangopup-build runtime-transport verify --transport <TRANSPORT_DIR>
pangopup-build runtime-transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
```

The transport has three independent deterministic Zstandard frames and one
canonical manifest binding the exact profile, metadata, notices, stored bytes,
and reconstructed bytes. It never opens or packages the 15 GB SNV member.
The exact derived members are now public in `runtime-grch38-v1`; `pangopup
sync` downloads and installs them with the compatible SNV lookup.

Maintainers can prepare the exact model-side publication source without
rebuilding, recompressing, materializing decoded payloads, or uploading
anything:

```text
pangopup-build runtime-release prepare \
  --transport <EXACT_RETAINED_RUNTIME_TRANSPORT> \
  --target-commit <40_LOWERCASE_HEX> \
  --output <ABSENT_DIRECTORY>
```

The production-only command accepts the qualified transport identity, streams
each Zstandard frame once through bounded semantic verification, then copies
each stored file once into a private read-only stage plus a canonical
release profile, `SHA256SUMS`, and release notes. The profile binds exact
runtime identities, immutable future URLs, the twelve upstream model-source
checkpoints, and converter paths at the target commit. It performs no network
request or GitHub mutation. Raw Zenodo, NCBI, GENCODE, checkpoint, and
qualification inputs are not included.

Pangopup deliberately does not implement HGVS, transcript/protein projection,
clinical interpretation, or general gene annotation. Callers must identify one
concrete GRCh38 genomic variant before asking for a splice score.

## Why lookup comes first

The Zenodo score source contains 4,099,255,665 SNV rows across 19,913
protein-coding genes. Pangopup compiles those text files into a purpose-built
binary index that exploits their genomic ordering, repeated defaults, and
three-alternates-per-locus structure.

Logically this behaves like a key-value store:

```text
(GRCh38 contig, position, REF, ALT) -> one or more gene-specific score records
```

Physically it is not a generic hash table or embedded database. Private v1 uses
an immutable 11-byte record per ordinary locus, contiguous gene segments, and a
balanced per-contig overlap tree. That shape removes text parsing and
decompression from the request path and avoids loading billions of ordinary key
objects.

The complete-corpus fixed payload projects to about 14.0 GiB before small
directories and provenance. The certified member is 15,033,158,255 bytes
(about 14.0 GiB). This is deliberately larger than the 1.589 GiB
hierarchical sparse candidate: the real-corpus benchmark found fixed lookup
consistently faster on the now-removed equal candidate harness after direct was
corrected to use ranked zero-copy mmap lookup, and query speed is the first
accepted priority. The
installed file is memory-mapped, so Pangopup reads only the directory and
record pages needed by a query rather than copying the file into heap.

## Runtime assets

A lookup-only installation can use an explicitly supplied certified SNV bundle
or the active SNV bundle installed in Linux user data. A complete four-asset
tuple can be installed, inspected, and consumed automatically for fallback.
The target full service uses four versioned assets:

| Asset | Used for | Original source | Installed form |
|---|---|---|---|
| SNV score index | Shipped fast path | Zenodo precomputed scores | Certified three-file bundle with a fixed 11-byte mmap member |
| Model weights | Shipped raw CPU kernel, variant scorer, and installed/explicit CLI fallback | Upstream Pangolin checkpoints | Authenticated three-file ONNX bundle in immutable `runtime-grch38-v1` |
| GRCh38 sequence index | Shipped provider for fallback sequence windows and REF validation | NCBI RefSeq GRCh38.p14 FASTA | Certified `PGRREF01` two-bit/ambiguity-run mmap bundle |
| Splice mask | Shipped provider for fallback masking and immutable publication | GENCODE release 38 annotation | Exact selected 6,703,320-byte `domains.pgm` mmap member |

NCBI supplies the reference genome sequence; it does not supply the Pangolin
model. The maintainer converter already produces a verified, unquantized
combined ONNX graph from the twelve pinned checkpoints. The reviewed
publication set pairs that converted model with complete exact upstream and
Pangopup source archives plus a standalone GPLv3 license copy. Those
preferred-source files are release companions, not installed runtime members.

The GRCh38 sequence index maintenance builder accepts the exact pinned NCBI
FASTA and assembly report, selects the 25 required assembled molecules,
certifies every decoded base and the frozen model contexts, and creates an
immutable three-file bundle. Runtime code memory-maps `reference.pgr` and copies
only the requested window into caller-owned memory. The model scorer uses this
compiled GRCh38 sequence index directly when a caller opens it, so a full
installation needs it. Pangopup will distribute the compact `PGRREF01` bundle,
not republish the raw NCBI FASTA. The same principle applies to GENCODE: the
model scorer consumes the compiled `domains.pgm` provider; GTF/gffutils were
one-time selection inputs, not runtime data or current build-crate
dependencies.

The release boundary is the small set of derived files the shipped or future
full runtime opens. Pangopup does **not** republish the 13 GB Zenodo archive or
extracted TSVs, the NCBI FASTA/assembly report, or the GENCODE GTF/SQLite
database. It distributes, or plans to distribute, the compiled SNV index,
converted ONNX model, compact GRCh38 sequence index, and compact splice mask
because those are the files the standalone runtime maps or executes. Original
Pangolin checkpoints are conversion inputs rather than installed runtime
members; the public release preserves them in a complete exact upstream
preferred-source archive beside the converted model.

### GENCODE mask runtime and retained selection evidence

Pangolin masking needs more than a set of exon coordinates. The candidate
comparison retained each exact versioned GENCODE identifier (including `_PAR_Y`),
strand, inclusive source span, effective `(start,end]` membership, upstream
point-query rank, and normalized exon-boundary set. An unversioned gene filter
matches the stable component only after the request contig is known, so chrX
and chrY pseudoautosomal copies are never merged.

The one-time private qualification run authenticated the pinned SQLite, GTF,
Python, and SQLite environment; produced one canonical ordered export; and
certified interval-tree, domain, and binned-postings candidates. The retained
full-source evidence
contains 60,649 genes, 88,202 constant-membership domains, and 591,404 exon
boundaries on all 25 primary contigs.

The balanced six-round, 1,000-query comparison selected
`domains`: headline p50/p95 were 171/331 ns, versus 241/401 for interval-tree
and 241/431 for binned postings.
ADR 0013 promotes the exact selected `PGMBEN01` v1 bytes behind
`pangopup_index::mask`; ordinary runtime open accepts only domains and neither
rebuilds nor scans the complete member. There is still no installable mask
transport or public mask release. The candidate writer, alternate readers,
capture lifecycle, feature-gated CLI, and executable spec are no longer
compiled at HEAD. Their durable report, 1,000-query manifest, and git history
remain available without carrying the completed experiment in the product.
See the retained
[selection evidence](planning/artifacts/012-gencode-mask-format-selection.md)
and [ADR 0013](architecture/decisions/0013-byte-identical-gencode-mask-promotion.md).

The normal test path builds the checked synthetic profile:

```text
pangopup-build reference build --profile pangopup-reference-mini-v1 \
  --source tests/fixtures/reference-production-mini/source.fa.gz \
  --assembly-report tests/fixtures/reference-production-mini/assembly_report.txt \
  --output <ABSENT_DIR>
pangopup-build reference inspect --bundle <DIR>
pangopup-build reference window --bundle <DIR> --contig NC_000001.11 \
  --start 1 --length 15
```

Production uses profile `refseq-grch38p14-primary-v1`. Build performs private
exhaustive certification once; there is intentionally no public long-running
`reference verify` command. Ordinary open checks bounded metadata and the
ambiguity table without hashing or scanning dense genome bytes.

## Upstream compatibility and raw CPU kernel

Variant-level inference implementations are tested against
`tests/fixtures/pangolin-compat-v1`, a 227,060-byte offline corpus captured from
Pangolin 1.0.2 commit `5cf94b8`. Its 24 cases retain exact GRCh38 contexts,
typed raw model arrays, masked and unmasked results, overlap order, rejection
witnesses, and dtype-aware public rendering. Deletion arrays remain `f64` just
as upstream produces them; other supported shapes remain `f32`.

Maintainers validate it without Python, PyTorch, model weights, FASTA, GTF,
SQLite, or network access:

```text
pangopup-build compatibility inspect --corpus tests/fixtures/pangolin-compat-v1
```

The expensive `compatibility capture` command is a provenance tool, not a
routine test or runtime path. Normal gates replay the frozen arrays in Rust and
deliberately do not rerun the model.

The separate `pangolin-model-v1` trust root authenticates every state tensor in
the twelve checkpoints and 45,756 independently generated raw-channel values.
`pangopup-model` opens one bounded three-file ONNX bundle, executes a
single-owner ONNX Runtime CPU session at `1/1` threads, and returns the twelve
channels without hiding ensemble or masking behavior. A retained complete-
request comparison keeps that portable ordinary default: affinity-aware
`auto/1` did not improve the two-strand case, while fixed `8/1` was the
host-specific frontier winner. The typed explicit policy opener is a low-level
qualification seam, not a CLI setting. Maintainers can inspect
or qualify an already-built bundle without Python or the original checkpoints:

Ticket 022 compares the exact v1 singleton with two distinctly identified v2
candidates: zero-padded context batching and independent paired
reference/alternate inputs. The first run is retained but ineligible because
the v2 exporter did not receive its declared dynamic axes. Corrected graphs
repeated the full experiment; both policies were inconclusive from singleton
drift and neither candidate met the independent replacement gates. Ordinary
production dispatch remains singleton; maintainer conversion modes, tiny
checked candidate fixtures, and the ignored comparison harness retain
reproducibility.

```text
pangopup-build model inspect --bundle <MODEL_BUNDLE>
pangopup-build model qualify --bundle <MODEL_BUNDLE> \
  --evidence tests/fixtures/pangolin-model-v1
```

Normal gates use a tiny checked synthetic graph and never rebuild the
production model. See
[ADR 0014](architecture/decisions/0014-authenticated-onnx-cpu-kernel.md) and
the [qualification report](planning/artifacts/018-authenticated-cpu-model-kernel.md).

`pangopup-engine` constructs fixed distance-50 reference and alternate
contexts, preserves upstream dtype-specific indel arithmetic, averages the
four tissue groups, applies shared-array masking in authenticated gene order,
and returns exact hundredths with ordered versioned GENCODE identities. Normal
tests replay all 14 scored cases, 36 raw sequence evaluations, six rejections,
and four controlled cases without production assets or Python. See
[ADR 0015](architecture/decisions/0015-variant-level-model-scoring.md) and the
[scoring evidence](planning/artifacts/019-variant-level-model-scoring.md).

## Shipped SNV release, local transport, and installation

Pangopup now packages an explicitly supplied certified bundle into canonical
release-sized files and reconstructs the exact installed bytes:

```text
pangopup-build transport pack --bundle <BUNDLE> --output <ABSENT_DIR>
pangopup-build transport verify --transport <TRANSPORT_DIR>
pangopup-build transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
```

The transport directory contains canonical `transport.json`, byte-exact copies
of the bundle manifest and CC BY notice, and numbered fragments of one pinned,
checksummed Zstandard frame over `scores.pgi`. Pack and unpack stream through
unique sibling staging directories and publish with Linux atomic no-replace
rename. `transport verify` proves all declared bytes and the single frame
without creating a 15 GB scratch file; unpack additionally runs exhaustive
fixed-v1 semantic certification before publication. SHA-256 proves integrity,
not who published the files.

Release maintainers can prepare the pinned public metadata without opening or
hashing either payload part:

```text
pangopup-build release prepare \
  --transport <TRANSPORT_DIR> \
  --receipt <PROOF_RECEIPT_JSON> \
  --output <ABSENT_DIR>
```

The public command accepts only the reviewed `snv-grch38-v1` receipt and
transport identities. It atomically emits a byte-identical proof receipt, the
checked canonical release profile, `SHA256SUMS`, and release notes from bounded
metadata. This prepares publication; it does not contact GitHub, upload bytes,
change repository settings, or make the release public.

The reviewed result is published as the immutable
[`snv-grch38-v1` release](https://github.com/genomoncology/pangopup/releases/tag/snv-grch38-v1).
Its eight assets include the exact five-file installable transport plus the
proof receipt, release profile, and `SHA256SUMS`. GitHub reports
`immutable=true` and server-side SHA-256 digests matching the checked profile.
The release notes provide the exact five-download manual installation path;
the three publication-metadata assets stay outside the transport directory.

Pangopup does not ship a release uploader. A later independently reviewed
publication lifecycle will have the coordinator invoke an authenticated
official `gh` executable directly after deterministic local preparation and
verification succeed. Preparation proves the bytes produced at that point; it
does not make a local pathname immutable against later mutation. That later
publication work must define the controlled stable source, draft-first upload,
remote inventory and digest comparison, and immutable finalization before any
new public effect. Release publication remains outside lookup, installation,
and every credential-free runtime path.

For `runtime-grch38-v1`, that boundary is now public and immutable. The
release has exactly 15 assets: twelve derived runtime/profile/checksum
files, complete exact Pangolin and Pangopup source archives, and a standalone
upstream GPLv3 license. Its release body is a separate retained file and is not
uploaded as an asset. Every remote name, size, and GitHub SHA-256 matched after
one upload, and the completed release reports `immutable=true`. No raw Zenodo,
NCBI, or GENCODE source input is included. The library now has pinned
model-side sync composed with SNV sync in one public CLI command.

The current CLI synchronizes the complete compatible splice runtime or can
install an already available SNV transport without networking:

```text
pangopup sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]
pangopup status [--data-dir <ABSOLUTE_PATH>]
pangopup assets install --transport <TRANSPORT_DIR> [--data-dir <ABSOLUTE_PATH>]
```

`pangopup sync` never asks GitHub for “latest.” The binary contains exact
`snv-grch38-v1` and `runtime-grch38-v1` profiles with literal HTTPS URLs,
sizes, and SHA-256 digests. It downloads sequentially through a bounded buffer, follows only a short
allowlisted HTTPS redirect chain, and resumes an interrupted member only when
a strong ETag and exact byte range agree. `--offline` forbids network access
and can install a previously completed cached transport.

The `pangopup-assets` library separately pins the exact ten-file
`runtime-grch38-v1` download set. `sync_runtime_assets` uses the same bounded
HTTPS, redirect, resume, private-cache, and nonblocking-lock implementation.
On the successful install path it reads each completed cached member once per
attempt, decodes each
compressed model/reference/mask frame directly into the existing atomic
installation stage, and never creates a second decoded transport tree.
After a content failure, recovery deliberately rereads completed members once
to retain authenticated good files and discard only corrupt files.
Top-level CLI `sync` and `status` compose these two typed operations; lookup
still never downloads implicitly.

It resolves an explicit data directory, `PANGOPUP_DATA_DIR`, `XDG_DATA_HOME`,
or `HOME` in that order. Installation holds one nonblocking lock, validates and
decompresses every transported byte once, publishes an immutable receipt-bound
bundle, and atomically selects it in `active.json`. It then performs only cheap
structural `BundleOpen` validation—never a second whole-index scan. Reinstalling
the same bundle validates its receipt, member shapes and sizes, manifest, and
cheap-open structure without opening transport parts or hashing `scores.pgi`.
Lookup discovers this active bundle when `--bundle` is absent; the explicit
override remains available for development and offline use.

The target is built in independently proved layers:

1. deterministically package, split, verify, and reconstruct the lookup
   transport without changing the installed mmap bundle (shipped);
2. install caller-supplied transport files into Linux/XDG data storage with
   locking, staging, checksums, receipts, atomic publication, active selection,
   and cheap verified reuse (shipped);
3. publish immutable GitHub release assets and prove manual installation,
   offline restart, and lookup on a clean supported machine (shipped); and
4. expose `pangopup sync` to resolve that observed pinned release
   manifest and safely resume/download its exact parts through the same
   installer (shipped).

Publication is blocked unless GitHub immutable releases are enabled and the
completed release reports `immutable=true`; a mutable release is never a
fallback. Remote-sync work begins only after that public contract has been
observed and recorded.

The current lookup CLI resolves and reuses a complete compatible local
installation without networking. A future service can invoke the same pinned
sync/installer boundary. It will memory-map installed members,
initialize the selected model provider, and only then report ready. It will
never fetch an unpinned “latest” release.

The explicit first `pangopup sync` is the provisioning operation, while
`pangopup status` reports ready, partial, missing, syncing, and installing
state. Exact byte progress remains future work. Later starts use the
already installed bundle without contacting the network. A failed download or
checksum will never replace an older complete bundle or start with partial
data.

Transport and reconstructed score hashes are checked in the one installation
stream. Ordinary status, reuse, and startup perform cheap receipt, manifest,
size, version, and structural checks rather than rereading several gigabytes.
Complete semantic certification remains the explicit build-time
`pangopup-build verify` operation.

On Linux, durable assets live under:

```text
${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/bundles/<bundle-id>/bundle/
```

Temporary downloads may use:

```text
${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/
```

The data directory is authoritative and must not be treated as disposable
cache. `PANGOPUP_DATA_DIR` or `--data-dir` can override durable discovery;
`PANGOPUP_CACHE_DIR` or `--cache-dir` can override disposable download storage.
This shipped installer and sync client are Linux-only; macOS and Windows
behavior is not claimed. Persistent progress, signatures, repair/GC/rollback,
and container preinstall remain future work.

## Planned service operation

Pangopup will expose one foreground HTTP process, `pangopup serve`, over the
same typed lookup-first routing API used by the CLI. It will provide stable
batch JSON, bounded requests, readiness, liveness, and status information.
`pangopup status` will expose the same non-secret runtime and asset identities
to command-line operators.
Docker, systemd, Kubernetes, or another external manager will own process
start, stop, and restart. Pangopup will not daemonize or implement a second
process supervisor.

A future minimal container will run as a non-root user, use a read-only runtime
filesystem, expose a healthcheck, and either contain a verified pinned asset
profile or mount one read-only. The HTTP service, lifecycle integration, and
container are not implemented yet.

## Performance priorities

After correctness, Pangopup optimizes in this order:

1. query latency and throughput;
2. resident memory and pages touched;
3. compressed download size.

The Ticket 004 report measures the complete artifact's warm one-open library
lookup separately from fresh CLI process/open/render/write cost. Cold lookup is
not inferred from a first post-build request: it remains unmeasured unless an
OS/device procedure proves the addressed pages were nonresident. No HTTP or
model latency follows from these lookup measurements. Serialization-only
measurements invoke the same library renderer as the shipped CLI, with the
benchmark asserting byte equality against fresh CLI stdout.

Memory mapping does not mean that the index uses literally no RAM. It means the
operating system loads file pages only as they are touched and can reclaim them
under pressure. The process may show a large virtual address mapping while its
resident working set remains much smaller. Model weights and active inference
tensors consume ordinary resident memory and are measured separately.

A historical experiment compressed the complete bundle to 1,935,000,209 bytes
with GNU tar 1.35 and Zstandard 1.5.5 level 9. That measurement established the
scale but is not the accepted lookup transport. The shipped transport compresses
only the exact `scores.pgi` stream as one deterministic Zstandard frame and
cuts it into ordered 1,000,000,000-byte parts bound by a canonical manifest.
The shipped local unpack command, and later managed installation, reconstructs
the same fixed mmap member. Download encoding must never put decompression on
the query path.

## Current state

Implemented today:

- the eight-crate Rust workspace and strict lint/test/spec gates;
- a least-privilege, full-SHA-pinned CI workflow whose downloaded maintenance
  tools are authenticated by exact size and SHA-256, plus a checked Rust
  advisory/license/source policy in the ordinary lint gate;
- runtime `pangopup` CLI help/version behavior with two executable smoke specs;
- GPL-3.0 source licensing, upstream Pangolin attribution, and CC BY 4.0
  dataset attribution;
- a retained Rust analyzer that scanned the complete downloaded score corpus;
- complete-corpus entropy, sparsity, and candidate-format measurements;
- six deterministic, attributed excerpts of the real score source, including
  overlapping genes, both published `REF=N` shapes, and every coordinate gap;
- exact GRCh38 SNV, Ensembl gene, centi-score, and relative-position Rust types;
- bounded-memory gzip/TSV validation plus an observable source-inspection
  command, `pangopup-build inspect <SOURCE_DIR>`;
- measured fixed/direct/Zstd/LZ4/Tabix comparison on a deterministic real lab
  corpus, selecting and hardening the fixed 11-byte private v1 format; the
  completed comparison implementation has since been removed;
- deterministic miniature fixed-index writing, structurally checked mmap open,
  exact lookup/exception round trips, and `pangopup-build prototype-roundtrip`;
- deterministic full-corpus construction through `pangopup-build build`, with
  an explicit plain/gzip FASTA input, complete GRCh38.p14 reference
  certification, disk-backed payload/reference scratch, RFC 8785 provenance,
  and atomic immutable bundle publication;
- complete offline bundle certification through `pangopup-build verify`,
  including exact member hashes, canonical index sections and records,
  reconstructed index/source segment and exception counts, and equality of
  independent source/decoded logical streams (source direction is retained
  provenance whose checked total, not split, is reconstructable from fixed-v1);
- deterministic local `transport pack`, `transport verify`, and `transport
  unpack`, with canonical metadata, pinned bundled libzstd 1.5.7, exact decimal
  1 GB parts, bounded streaming verification, and byte-identical certified
  reconstruction;
- bounded deterministic `release prepare` metadata for the pinned
  `snv-grch38-v1` public-release contract, without payload-part reads;
- the public immutable `snv-grch38-v1` eight-asset release with exact
  server-side digests and a documented five-file manual install path;
- Linux local explicit installers and combined `pangopup status`, with strict XDG
  discovery, private dirfd-relative state, a nonblocking lock, single-stream
  reconstruction, canonical receipts/stage markers, immutable bundles, atomic
  active selection, crash reconciliation, and transport-free score reuse;
- Linux `pangopup sync`, pinned to the compiled SNV and runtime profiles, with
  sequential bounded TLS downloads, strong-ETag range resume,
  private atomic cache publication, offline reuse, and the same installer as
  the final publication boundary;
- a checked 1,000-request source-derived JSONL regression fixture exercised
  through one real provider open and seven CLI batches;
- the standalone API, runtime-data, delivery, and performance decisions;
- an object-safe, thread-safe typed score provider over one long-lived mmap;
- transactional `pangopup lookup` JSONL/table batches with strict GRCh38
  aliases, optional source-gene filtering, all-overlap results, typed misses,
  and explicit source-reference ambiguities;
- the strict `pangopup-compat-v1` upstream oracle: 14 scored cases, six
  rejection cases, four controlled post-processing cases, exact pinned source
  identities, complete independently fixed controlled vectors, and a bounded
  offline semantic inspector. Future capture separately authenticates the base
  Python executable and a generic venv launcher/`pyvenv.cfg`, executes only the
  held base descriptor, bypasses `.pth` and bytecode startup paths, and binds
  every loaded file-backed or interpreter-owned module before and after use;
- a production `PGRREF01` GRCh38 sequence-index bundle, reader, provider, and
  builder based on the retained comparison that selected `acgt2-rle-v1` by
  speed. The discarded candidate codecs, miniature, benchmark executable, and
  CLI have been removed; their reports and decisions remain historical
  evidence;
- exact versioned/PAR GENCODE identity and mask semantics, a production
  domains-only mmap provider, a checked miniature binary/oracle, and a retained
  fixed 1,000-query comparison manifest. The one retained comparison selected
  constant-membership `domains` at the first p95 speed step. The selected
  byte-identical domains member is the runtime representation; the completed
  candidate and qualification source has been removed;
- an authenticated, bounded ONNX model bundle contract, locked independent
  checkpoint evidence/conversion tooling, and one qualified raw CPU kernel
  returning twelve fixed-order channels;
- a single-owner variant scorer that composes the reference, mask, and raw
  model providers for masked distance-50 results over the supported literal
  GRCh38 subset;
- a typed lookup-first router with installed-profile and explicit-path CLI
  model fallback, lazy
  exactly-once component opens, a manifest-authenticated same-descriptor
  reference mmap, an identified same-descriptor mask mmap, complete provenance,
  stable warnings/errors, gene filtering after all-gene masking, and
  transactional JSONL/table batches.

Not implemented yet: HTTP service, container, exact persistent download
progress, or
repair/GC/rollback.
Public delivery of the compiled GRCh38 sequence index, mask, and model is
complete. Deterministic local model-side packaging, offline installation, and
coherent activation are implemented.
Without fallback flags, an activated installation sends a pure SNV miss or
supported non-SNV to its compatible model tuple. A complete explicit tuple
wins. An explicit `--bundle` never borrows installed model-side assets.

The rolling outcome order is:

1. checked source fixture and executable source contract (complete);
2. measured miniature index writer/reader (complete);
3. full streaming builder and complete index certification (complete);
4. typed SNV lookup API and CLI (complete);
5. deterministic split lookup transport (complete);
6. explicit local Linux/XDG installation and active discovery (complete);
7. immutable GitHub publication and bounded public/manual-install proof
   (complete);
8. pinned remote sync against the observed public release contract (complete);
9. an upstream Pangolin compatibility corpus (complete);
10. measure and select the compact RefSeq GRCh38.p14 payload (complete:
    `acgt2-rle-v1` selected by speed);
11. harden the selected reference payload as a complete provider and bundle
    (complete);
12. establish exact GENCODE mask semantics and select a measured compact
    representation (complete: `domains` selected by speed);
13. isolate SNV and reference builder provenance so unrelated code no longer
    churns artifact identities, without rebuilding either production asset
    (complete);
14. expose the exact selected representation through a production mask
    provider without rebuilding it (complete);
15. remove obsolete mask candidate and qualification machinery while retaining
    the selected member and durable evidence (complete);
16. remove the closed reference-format experiment while retaining the
    production reference path and durable evidence (complete);
17. remove the closed SNV-format experiment while retaining the production
    fixed-v1 path and durable evidence (complete);
18. package the pinned checkpoints and implement raw CPU-kernel parity
    (complete);
19. compose the GRCh38 sequence index, mask, raw model, and post-processing
    into compatible variant-level scoring (complete);
20. add lookup-first routing and stable CLI model JSON with route and asset
    provenance (complete);
21. measure complete-request CPU threading and select the portable ordinary
    policy (complete);
22. measure reference/alternate graph batching against that policy
    (complete: corrected experiment retained singleton);
23. measure repeated complete requests and add a bounded result cache
    (complete: persistent SQLite selected);
24. create one coherent SNV, model, GRCh38 sequence index, and mask profile
    (complete);
25. install and atomically select that profile from trusted local inputs
    (complete);
26. separate the sole reference reader from byte-producing provenance and add
    held-descriptor installed admission (complete);
27. consume the activated installed profile for lookup-first model fallback
    (complete);
28. package and locally verify the exact derived model, GRCh38 sequence index,
    and mask without touching the SNV member (complete);
29. close publication prerequisites and publish only the derived model,
    GRCh38 sequence index, and mask runtime assets (complete);
30. add pinned model-side sync and prove fresh-machine CLI inference;
31. a foreground HTTP/status service with CLI and HTTP acceptance tests;
32. non-root Docker and documented systemd lifecycle integration;
33. observability, security, performance, and executable/container release
    hardening.

These are outcome boundaries rather than a prewritten ticket backlog. Only the
next coordinator-authored and independently reviewed ticket is active work.
The compatible-profile outcome will be delivered through multiple bounded
tickets when it reaches the frontier; it is not one oversized ticket.

See [`planning/frontier.md`](planning/frontier.md) for the current boundary and
[`architecture/README.md`](architecture/README.md) for the durable design.

## Workspace

- `pangopup-core` — public typed vocabulary and provider capabilities;
- `pangopup-index` — private format codec and validated mmap reader;
- `pangopup-assets` — installed-bundle certification, deterministic SNV and
  model-side local transports, pinned resumable TLS sync, and secure Linux
  local-store/activation
  state;
- `pangopup-build` — offline source validation, deterministic artifact
  builders, the bounded compatibility-corpus adapter, and authenticated
  maintainer model evidence/conversion commands;
- `pangopup-cli` — shipped lookup, combined pinned asset sync/status, local install, and
  output adapter; service commands remain future;
- `pangopup-model` — exact v1 and closed v2 authenticated model bundles,
  context/strand encoding, bounded candidate batching, and the raw single-owner
  ONNX Runtime CPU kernel;
- `pangopup-engine` — fixed GRCh38 variant construction, compatible
  post-processing/masking, and ordered modeled results;
- `pangopup-cache` — persistent exact model-result keys and values, bounded
  insertion/update-order eviction, and disposable SQLite recovery;
- future `pangopup-http` — long-lived HTTP adapter over the same core.

## Development

Install `cargo-deny` 0.19.4 before running the repository gate. `make lint`
runs formatting, Clippy, then
`cargo deny check advisories bans licenses sources --warn unmaintained`;
`make test` and `make spec` retain their existing ownership. The checked policy
denies vulnerabilities, unsound advisories, yanked dependencies, stale ignores,
unreviewed licenses, and unknown registry or Git sources. Duplicate versions
and unmaintained advisories are visible warnings. Registry wildcard
dependencies are not present in the locked graph; cargo-deny 0.19.4 cannot
separate them from Pangopup's intentionally versionless local path edges
without changing publishability or builder-causal manifests, so the reviewed
policy leaves wildcard linting allowed and retains those path edges unchanged.

The checked workflow grants only read access to contents and installs the exact
mustmatch 0.1.0 wheel and cargo-deny 0.19.4 archive only after both size and
SHA-256 verification. The live GitHub repository now has read-only Actions
defaults, no Actions PR approval, Dependabot security updates, the three
writable requested secret-scanning controls, and the two reviewed `main`
rulesets. GitHub exposes validity checks in repository reads but not its
repository-local write schema. That optional alert-triage control remains
disabled; Pangopup has no configured repository secrets or open secret alerts,
and the publication-security issue is closed. No model-side runtime asset,
executable, container, SBOM, or release provenance was published by this
baseline.

The coordinator writes one ticket at a time from the previous shipped result
and rolling frontier. Three distinct sub-agents then provide independent ticket
review, development, and adversarial code review. Findings return to the
coordinator/ticket-reviewer pair or developer/code-reviewer pair. The
coordinator runs the final gate and commits and pushes independently approved
work; developers never commit or push. Documentation is named in the ticket,
implemented with the behavior, reviewed with the code, and checked once more
for stale claims before completion. A material final-gate or documentation
finding returns to the same developer and code reviewer; a scope defect returns
to the coordinator and same ticket reviewer.

```bash skip
make lint
make test
make spec
```

Install a local transport once, then query its active bundle:

```bash skip
pangopup assets install --transport /path/to/transport
pangopup status
pangopup lookup --variant GRCh38:17:7686072:G:T
```

Or open an explicitly supplied certified bundle as an override:

```bash skip
pangopup lookup --bundle /path/to/bundle \
  --variant GRCh38:17:7686072:G:T \
  --variant GRCh38:NC_000017.11:7686072:G:C \
  --format jsonl
```

Accepted contigs are exactly `1`…`22`, `X`, `Y`, `M`, their `chr`-prefixed
forms, or the 25 exact RefSeq accessions in the opened manifest. Add one
`--gene ENSG…` to filter the complete batch. JSON Lines is the default;
`--format table` emits exact tab-separated rows.

## Prepared executable installer

Pangopup prepares a direct Linux x86_64 executable rather than an archive. Once
Ticket 038 publishes `v0.1.0`, install the GitHub Latest executable with:

```bash skip
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/main/install.sh | bash
```

For an immutable version, use the tagged script and explicit version:

```bash skip
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.1.0/install.sh | bash -s -- --version 0.1.0
```

The installer supports Linux x86_64/amd64 with a GLIBC 2.35 baseline and
requires Bash, curl or wget, and one of sha256sum, shasum, or openssl. It verifies the adjacent checksum,
smoke-tests the replacement, and atomically installs to
`${PANGOPUP_INSTALL_DIR:-$HOME/.local/bin}`. It does not use sudo, edit `PATH`,
or download data; run `pangopup sync` afterward. There is no public executable
until Ticket 038 completes.

The publication candidate is qualified before it can become public. In an
isolated pinned Linux container it must synchronize the already-published SNV
and model-side assets, reuse them offline, report ready status, reproduce the
seven-batch 1,000-SNV corpus, and reproduce one exact non-SNV model result.
The checked runner and checker are
`scripts/run-production-qualification.sh` and
`scripts/check-production-qualification.py`. This preparation has not created
a tag, release, or public executable.

Release builders use explicit, read-only inputs and never download data or
discover a home directory:

```bash skip
pangopup-build --help
pangopup-build <COMMAND_OR_NAMESPACE> --help
pangopup-build build --source <PANGOLIN_SOURCE_DIR> --reference <GRCH38_FASTA_OR_GZIP> --output <NEW_BUNDLE>
pangopup-build verify <BUNDLE>
pangopup-build transport pack --bundle <BUNDLE> --output <ABSENT_DIR>
pangopup-build transport verify --transport <TRANSPORT_DIR>
pangopup-build transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
pangopup-build release prepare --transport <TRANSPORT_DIR> --receipt <PROOF_RECEIPT_JSON> --output <ABSENT_DIR>
```

The successful help paths above are generated from the same checked command
catalog that dispatches maintenance operations. Namespace and leaf help state
the exact required arguments and closed choices without opening a file,
starting a process, loading a model, or using the network.

Each successful command writes exactly one JSON line. A bundle contains only
`manifest.json`, `NOTICE`, and `scores.pgi`; publication never mutates or
replaces an existing different bundle. Atomic no-replace publication is
currently Linux-only; other targets return a typed unsupported publication
failure and remove their staging directory.

Pangopup source is licensed under GPL-3.0-only. Pangolin model/source notices
and the score dataset's separate CC BY 4.0 attribution are recorded in
[`NOTICE`](NOTICE) and must travel with applicable release assets.
