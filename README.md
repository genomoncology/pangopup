# Pangopup

Pangopup is a standalone GPL-3.0 Rust project for high-performance,
Pangolin-compatible splice scoring on GRCh38 genomic variants. Today it ships
an exact precomputed-SNV library and CLI, deterministic local release
transport tooling, atomic Linux/XDG installation, and pinned resumable sync of
the immutable public SNV transport. Model inference and an HTTP service are
planned but not implemented.

The target service will answer each request through one of two paths:

1. **SNV lookup:** return an exact precomputed Pangolin result from a compact,
   memory-mapped index.
2. **Model fallback (not implemented):** when no lookup record exists, run a
   bundled Pangolin model against a local GRCh38 sequence window and splice-site
   annotation.

An SNV is a single-nucleotide variant: one reference base replaced by one
alternate base. The published Zenodo dataset already contains masked Pangolin
scores for every SNV it covers, so recomputing those values with the neural
network would be slower and could introduce small numeric differences.

## How one request works today and in the target service

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

The shipped CLI validates an SNV and performs the left-hand lookup path below.
The right-hand fallback is the planned routing behavior:

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
consistently faster on the equal candidate harness after direct was corrected
to use ranked zero-copy mmap lookup, and query speed is the first accepted
priority. The
installed file is memory-mapped, so Pangopup reads only the directory and
record pages needed by a query rather than copying the file into heap.

## Runtime assets

A lookup-only installation today can use an explicitly supplied certified SNV
bundle or the active SNV bundle installed in Linux user data. The target full
service will use four versioned assets:

| Asset | Used for | Original source | Installed form |
|---|---|---|---|
| SNV score index | Shipped fast path | Zenodo precomputed scores | Certified three-file bundle with a fixed 11-byte mmap member |
| Model weights | Planned fallback | Upstream Pangolin checkpoints | Planned verified Rust-runtime representation |
| GRCh38 sequence | Planned fallback sequence window and REF validation | NCBI RefSeq GRCh38.p14 FASTA | Planned compact indexed mmap file |
| Splice mask | Planned gene strand, spans, and exon boundaries | GENCODE release 38 annotation | Planned compact interval/boundary mmap file |

NCBI supplies the reference genome sequence; it does not supply the Pangolin
model. The target release process will publish a pinned copy or verified
conversion of the upstream model as a separate asset.

For the planned model path, the original NCBI reference will be downloaded as
FASTA when the reference asset is built. A target full installation downloads
the compiled reference member, not the raw FASTA, and performs bounded indexed
sequence reads rather than parsing FASTA during a request. The same principle
applies to GENCODE: GTF/gffutils is build input, not a runtime database.

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

Publication maintainers have a separate coordinator-only
`pangopup-build release upload-asset` command. It accepts exactly one reviewed
asset, the prepared and transport directories, a positive release ID, and an
absolute official GitHub CLI 2.45.0 path. It validates the reviewed CLI bytes
and executes them from a sealed in-memory snapshot. Small selected assets are likewise
sealed before validation; a large payload remains content-blind behind a
monitored Linux read lease until the upload child exits. The one request has a
21,600-second deadline. `SIGINT`, `SIGTERM`, lease breaks, and deadline failure
all use process-group kill and direct-child reap cleanup; child-side
parent-death protection prevents the direct request from surviving abrupt
coordinator death.
This is not a runtime downloader and is never used by lookup or installation.

The runtime can sync the exact binary-pinned public transport or install an
already available transport without networking:

```text
pangopup assets sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]
pangopup assets install --transport <TRANSPORT_DIR> [--data-dir <ABSOLUTE_PATH>]
pangopup assets status [--data-dir <ABSOLUTE_PATH>]
```

`assets sync` never asks GitHub for “latest.” The binary contains the exact
`snv-grch38-v1` profile: five literal HTTPS URLs, sizes, and SHA-256 digests.
It downloads sequentially through a bounded buffer, follows only a short
allowlisted HTTPS redirect chain, and resumes an interrupted member only when
a strong ETag and exact byte range agree. `--offline` forbids network access
and can install a previously completed cached transport.

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
4. expose `pangopup assets sync` to resolve that observed pinned release
   manifest and safely resume/download its exact parts through the same
   installer (shipped).

Publication is blocked unless GitHub immutable releases are enabled and the
completed release reports `immutable=true`; a mutable release is never a
fallback. Remote-sync work begins only after that public contract has been
observed and recorded.

The current lookup CLI resolves and reuses a complete compatible local
installation without networking. A future service provisioning step can invoke
the same pinned sync/installer boundary. It will memory-map installed members,
initialize the selected model provider, and only then report ready. It will
never fetch an unpinned “latest” release.

The target first start may therefore also be a provisioning operation. A
persistent progress/status surface remains future work. Later starts use the
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

- the five-crate Rust workspace and strict lint/test/spec gates;
- CLI help/version behavior with two executable smoke specs;
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
  corpus, selecting and hardening the fixed 11-byte private v1 format;
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
- Linux local `pangopup assets install` and `assets status`, with strict XDG
  discovery, private dirfd-relative state, a nonblocking lock, single-stream
  reconstruction, canonical receipts/stage markers, immutable bundles, atomic
  active selection, crash reconciliation, and transport-free score reuse;
- Linux `pangopup assets sync`, pinned to the compiled `snv-grch38-v1`
  profile, with sequential bounded TLS downloads, strong-ETag range resume,
  private atomic cache publication, offline reuse, and the same installer as
  the final publication boundary;
- a checked 1,000-request source-derived JSONL regression fixture exercised
  through one real provider open and seven CLI batches;
- the standalone API, runtime-data, delivery, and performance decisions;
- an object-safe, thread-safe typed score provider over one long-lived mmap;
- transactional `pangopup lookup` JSONL/table batches with strict GRCh38
  aliases, optional source-gene filtering, all-overlap results, typed misses,
  and explicit source-reference ambiguities.

Not implemented yet: model runtime/fallback, HTTP service, container,
persistent download progress/status, repair/GC/rollback, or result
cache. In this slice a syntactically valid concrete REF that
does not match an ordinary indexed key is `not_found`; runtime FASTA validation
begins only with the future model/reference slice.

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
9. an upstream Pangolin compatibility corpus;
10. pinned model, compact RefSeq GRCh38.p14, and compact GENCODE mask assets;
11. CPU inference parity, followed only then by measured accelerator options;
12. lookup-first model routing and evidence-gated result caching;
13. a foreground HTTP/status service plus Docker/systemd lifecycle integration;
14. observability, security, performance, and release hardening.

These are outcome boundaries rather than a prewritten ticket backlog. Only the
next coordinator-authored and independently reviewed ticket is active work.

See [`planning/frontier.md`](planning/frontier.md) for the current boundary and
[`architecture/README.md`](architecture/README.md) for the durable design.

## Workspace

- `pangopup-core` — public typed vocabulary, routing, and provider capabilities;
- `pangopup-index` — private format codec and validated mmap reader;
- `pangopup-assets` — installed-bundle certification, deterministic local
  transport, pinned resumable TLS sync, and secure Linux local-store/activation
  state;
- `pangopup-build` — offline source validation and deterministic artifact
  builders plus the thin maintenance CLI adapter;
- `pangopup-cli` — shipped lookup, pinned asset sync, local install/status, and
  output adapter; service commands remain future;
- future `pangopup-model` — model execution behind the core provider contract;
- future `pangopup-http` — long-lived HTTP adapter over the same core.

## Development

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
pangopup assets status
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

Release builders use explicit, read-only inputs and never download data or
discover a home directory:

```bash skip
pangopup-build build --source <PANGOLIN_SOURCE_DIR> --reference <GRCH38_FASTA_OR_GZIP> --output <NEW_BUNDLE>
pangopup-build verify <BUNDLE>
pangopup-build transport pack --bundle <BUNDLE> --output <ABSENT_DIR>
pangopup-build transport verify --transport <TRANSPORT_DIR>
pangopup-build transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
pangopup-build release prepare --transport <TRANSPORT_DIR> --receipt <PROOF_RECEIPT_JSON> --output <ABSENT_DIR>
pangopup-build release upload-asset --transport <TRANSPORT_DIR> --prepared <PREPARED_DIR> --gh <ABSOLUTE_PINNED_GH_BINARY> --release-id <POSITIVE_GITHUB_ID> --asset <EXACT_ASSET_NAME>
```

Each successful command writes exactly one JSON line. A bundle contains only
`manifest.json`, `NOTICE`, and `scores.pgi`; publication never mutates or
replaces an existing different bundle. Atomic no-replace publication is
currently Linux-only; other targets return a typed unsupported publication
failure and remove their staging directory.

Pangopup source is licensed under GPL-3.0-only. Pangolin model/source notices
and the score dataset's separate CC BY 4.0 attribution are recorded in
[`NOTICE`](NOTICE) and must travel with applicable release assets.
