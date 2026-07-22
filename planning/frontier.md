# Frontier

Updated: 2026-07-22

## Current boundary

Repository onboarding, source ingestion, format selection, full-corpus build
and certification, and typed SNV lookup are established. Pangopup is standalone
open-source software. Its shipped CLI accepts an explicit GRCh38 SNV and
optional gene filter and returns all matching source records by default from an
explicitly supplied fixed-v1 bundle. Speed leads memory and download size.
Release assets, automatic installation, model fallback, and the HTTP service
remain future work.

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

## Next front — reproducible release assets

Package the executable and generated data separately. The measured
1,935,000,209-byte tar+Zstandard lookup archive is too close to GitHub's
under-2-GiB per-asset ceiling, so split transport deterministically while
reassembling the unchanged installed fixed-v1 member. Add explicit install and
verify commands plus automatic first-start installation, immutable manifests,
checksums, locking, attribution, platform-standard data/cache directories,
offline operation, and GitHub release publication. Transport compression is
removed at installation; the lookup path maps the fixed representation.

## Later front — model-backed fallback

Pin the upstream commit and checkpoint hashes, encode the model in a Rust-usable
runtime, and build compact GRCh38 sequence and GENCODE masking members. Prove
CPU parity first, including indels and the overlapping-gene mask-order issue.
Then measure accelerator backends without changing score semantics. Keep the
precomputed SNV result authoritative whenever it exists.

## Later front — unified service

Try SNV lookup first, route lookup misses and supported non-SNVs to inference
through one typed API, then add the HTTP adapter and container. Measure
end-to-end concurrency, startup, memory, and tail latency. Add a persistent
model-result cache only if those measurements demonstrate useful repeated
inference beyond the operating-system page cache.

## Explicitly outside Pangopup

- HGVS parsing and genomic/transcript/protein projection;
- gene descriptions, aliases, disease knowledge, or clinical interpretation;
- GRCh37 and liftover.

These fronts are outcome boundaries. Only the next independently reviewed,
bounded ticket is implementation scope.
