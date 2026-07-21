# Frontier

Updated: 2026-07-21

## Current boundary

Repository onboarding, the initial architecture, and complete-corpus entropy
evidence are established. The product is a standalone service. Its canonical
input is an explicit GRCh38 genomic variant; a gene filter is optional; all
matching source records are returned by default. Speed leads size, and generated
lookup/model data ship as verified release assets.

## Active front — characterize and pin the smallest truthful corpus

The next slice should check in a tiny CC BY-attributed source fixture selected
from the downloaded data and make the source invariants executable. It should
include:

- ascending and descending source position order;
- each reference base and all three alternate bases;
- score zero, nonzero score, and relative positions at useful boundaries;
- two overlapping gene records for one genomic SNV if present in the source;
- malformed synthetic rows for header, grouping, contiguity, and range failures.

Its observable result is a reproducible characterization command or test report,
not yet a production index. The fixture proves correctness only; the complete
source evidence in
[`artifacts/2026-07-20-full-dataset-entropy.md`](artifacts/2026-07-20-full-dataset-entropy.md)
drives compression and size decisions.

## Near front — compare lossless physical layouts

After the corpus is pinned, write the smallest builder/reader round trip needed
to prove the hierarchical sparse direct baseline and compare it with
independently compressed Zstd/LZ4 blocks around 1,024–4,096 loci and the 11-byte
fixed baseline. Measure size, reproducible cold-I/O lookup, warm lookup,
allocations, decoded bytes, and page faults using identical query sets. Retain
the direct format unless measurements expose a speed or operational failure.

## Following front — full streaming build and exact CLI lookup

Extend the accepted format to all 19,913 source files without loading the whole
dataset into heap memory. Certify every input invariant and source row, publish
one immutable bundle atomically, and expose gene-filtered plus all-overlap SNV
lookups through the CLI and executable spec.

## Later front — reproducible release assets

Package the executable and generated data separately. Add explicit install and
verify commands, immutable manifests, checksums, attribution, platform-standard
data directories, and GitHub release publication. Transport compression is
removed at installation; the lookup path maps the direct representation.

## Later front — model-backed fallback

Pin the upstream commit and checkpoint hashes, encode the model in a Rust-usable
runtime, and build compact GRCh38 sequence and GENCODE masking members. Prove
CPU parity first, including indels and the overlapping-gene mask-order issue.
Then measure accelerator backends without changing score semantics. Keep the
precomputed SNV result authoritative whenever it exists.

## Later front — unified service

Route SNVs to lookup and supported non-SNVs to inference through one typed API,
then add the HTTP adapter and container. Measure end-to-end concurrency,
startup, memory, and tail latency. Add a persistent model-result cache only if
those measurements demonstrate useful repeated misses beyond the operating-
system page cache.

## Explicitly outside Pangopup

- HGVS parsing and genomic/transcript/protein projection;
- gene descriptions, aliases, disease knowledge, or clinical interpretation;
- GRCh37 and liftover.

Later fronts remain outcomes, not implementation tickets, until the indexed SNV
path is correct and measured.
