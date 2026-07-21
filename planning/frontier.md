# Frontier

Updated: 2026-07-21

## Current boundary

Repository onboarding, the initial architecture, and complete-corpus entropy
evidence are established. The product is a standalone service. Its canonical
input is an explicit GRCh38 genomic variant; a gene filter is optional; all
matching source records are returned by default. Speed leads size, and generated
lookup/model data ship as verified release assets. Service adapters automatically
install their binary-pinned asset set into durable platform data storage.

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
validates ordinary payload only when touched. This evidence is comparative and
warm; definitive cold-I/O waits for the complete artifact.

## Active front — full streaming build and certification

Extend the accepted format to all 19,913 source files without loading the whole
dataset into heap memory. Certify every input invariant and source row, publish
one immutable bundle atomically, and expose gene-filtered plus all-overlap SNV
lookups through the CLI and executable spec.

## Later front — reproducible release assets

Package the executable and generated data separately. Add explicit install and
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

Later fronts remain outcomes, not implementation tickets, until the indexed SNV
path is correct and measured.
