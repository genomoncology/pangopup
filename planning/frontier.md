# Frontier

Updated: 2026-07-20

## Current boundary

Repository onboarding, the initial architecture, and complete-corpus entropy
evidence are established. The first feature ticket has deliberately not been
drafted: the public lookup multiplicity and input contract need Ian’s priority
decision. The on-disk choice is now narrowed to measured sparse candidates but
still needs actual lookup benchmarks rather than a size-only decision.

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
to compare the hierarchical sparse direct layout, independently compressed
Zstd/LZ4 blocks around 1,024–4,096 loci, and the 11-byte fixed baseline. Measure
size, reproducible cold-I/O lookup, warm lookup, allocations, decoded bytes, and
page faults using identical query sets. Accept one v1 format only from that
evidence.

## Following front — full streaming build and exact CLI lookup

Extend the accepted format to all 19,913 source files without loading the whole
dataset into heap memory. Certify every input invariant and source row, publish
one immutable bundle atomically, and expose gene-filtered plus all-overlap SNV
lookups through the CLI and executable spec.

## Parked fronts

- broader HGVS through Genome;
- model-backed non-SNV scoring;
- persistent cache for model results;
- ONNX Runtime and hardware-provider evaluation;
- REST service and container packaging.

These remain outcomes, not tickets, until the indexed SNV path is correct and
measured.
