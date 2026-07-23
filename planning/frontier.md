# Frontier

Updated: 2026-07-23

## Current boundary

Repository onboarding, source ingestion, format selection, full-corpus build
and certification, and typed SNV lookup are established. Pangopup is standalone
open-source software. Its shipped CLI accepts an explicit GRCh38 SNV and
optional gene filter and returns all matching source records by default from an
explicit fixed-v1 bundle or the active Linux user-data installation. Speed
leads memory and download size. Deterministic local transport, atomic install,
status, active discovery, cheap reuse, and the fast 1,000-case regression are
established. Immutable publication and pinned resumable remote sync are also
complete; model fallback and HTTP remain future.

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

## Next outcome — upstream compatibility corpus

Before selecting or porting a runtime, inventory the upstream Pangolin tests and
behavior and retain representative golden cases for SNVs, insertions,
deletions, delins, strands, masked/unmasked output, overlapping genes,
boundaries, unsupported inputs, and errors. Pin the upstream source and model
identities that produced those expectations.

## Later outcome — model/reference/mask assets

Package pinned model checkpoints plus compact, indexed RefSeq GRCh38.p14
sequence and GENCODE masking members. Builders or conversion tools must be
reproducible, bounded-memory, independently verifiable, and license-complete.
The service should not parse raw FASTA/GTF or open gffutils/SQLite at runtime.

## Later outcome — model-backed fallback

Implement the pinned model on CPU and prove the retained compatibility corpus,
including indels and the overlapping-gene mask-order issue, before optimizing
or adding routes. Only then measure accelerator backends such as MPS or CUDA
and alternative runtimes/quantization. Adopt them only with explicit numeric
tolerances, equal behavior, and measured end-to-end benefit. Keep every
precomputed SNV hit authoritative whenever it exists.

## Later outcome — lookup-first routing and evidence-gated caching

Try SNV lookup first, route lookup misses and supported non-SNVs to inference
through one typed API, and report route and asset provenance. Measure repeated
model workloads before adding a cache. If justified, cache only complete model
results under a key that includes normalized variant, gene/masking context,
model checkpoint, reference/mask identity, window, and inference parameters;
prove bounds, concurrency, corruption handling, and invalidation.

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

## Explicitly outside Pangopup

- HGVS parsing and genomic/transcript/protein projection;
- gene descriptions, aliases, disease knowledge, or clinical interpretation;
- GRCh37 and liftover.

These are rolling outcome boundaries, not a ticket backlog or promises about
unsettled implementation details. Only the next coordinator-authored,
independently reviewed, bounded ticket is implementation scope.
