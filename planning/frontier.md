# Frontier

Updated: 2026-07-21

## Current boundary

Repository onboarding, the initial architecture, and complete-corpus entropy
evidence are established. The product is a standalone service with no Genome
dependency. Its canonical input is an explicit GRCh38 genomic variant; a gene
filter is optional; all matching source records are returned by default. Speed
leads size, and generated lookup/model data ship as verified release assets.

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

## Parked fronts

- HGVS and coordinate projection;
- model-backed non-SNV scoring;
- persistent cache for model results;
- ONNX Runtime and hardware-provider evaluation;
- REST service and container packaging.

These remain outcomes, not tickets, until the indexed SNV path is correct and
measured.
