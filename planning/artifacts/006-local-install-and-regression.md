# Ticket 006 local installation and SNV regression evidence

Date: 2026-07-23

## Semantic regression

The checked fixture contains exactly 1,000 requests: 970 gene-filtered rows
selected round-robin from the six attributed source excerpts plus the 30 fixed
overlap, filtered-overlap, `REF=N`, and miss cases in Ticket 006. One integration
test opens the fixed-v1 fixture bundle once and sends every request through the
real `ScoreProvider`; a second runs the same corpus through seven CLI batches
(one unfiltered and one for each gene). Both compare complete JSONL bytes with
the independent direct-TSV oracle. The focused run completed successfully with
1,000/1,000 library expectations and all seven CLI batches in 0.05 seconds.

The byte-reproduction test regenerates the locus-closure source, fixture-only
reference, fixed-v1 bundle, request manifest, complete oracle, and seven oracle
subsets into an isolated directory and compares every file with the checked
fixture.

## Non-gating benchmark

Command:

```text
cargo bench --locked --package pangopup-cli --bench snv_regression
```

Host run:

| Mode | Requests | Results | p50 µs | p95 µs | p99 µs | Allocation calls/sample | Allocated bytes/sample | Minor faults/sample | Major faults/sample | RSS delta KiB | Output bytes |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| fresh open | 0 | 0 | 109 | 137 | 144 | 1,598 | 76,949 | 1 | 0 | 0 | 0 |
| fresh CLI process | 1 | 1 | 1,909 | 2,250 | 2,250 | 20 | 1,542 | 0 | 0 | 0 | 491 |
| warm provider + JSONL | 1 | 1 | 2 | 2 | 2 | 23 | 1,510 | 0 | 0 | 0 | 491 |
| warm provider + JSONL | 10 | 10 | 17 | 18 | 24 | 152 | 21,236 | 0 | 0 | 0 | 4,919 |
| warm provider + JSONL | 100 | 100 | 193 | 202 | 218 | 1,389 | 179,456 | 0 | 0 | 0 | 49,116 |
| warm provider + JSONL | 1,000 | 1,006 | 1,639 | 1,771 | 1,809 | 13,719 | 1,532,752 | 217 | 0 | 0 | 491,629 |

Fresh-open used 25 retained samples, fresh-process used 10, and warm batches
used 100. Allocation counts are process-allocator requests in the measured
parent operation; the fresh child process's internal allocations are not
visible to that counter. Each run first rebuilds the debug CLI from the current
workspace for the fresh-process row. Percentiles use the nearest-rank rule on
sorted retained samples (`ceil(p*n)-1`). Page-cache residency was not controlled, so none of
these rows is called cold. Page faults are `getrusage` deltas. RSS is Linux
`ru_maxrss`, a high-water mark whose delta often remains zero after prior
samples; it is not per-operation resident memory.

Timing is diagnostic only. Correctness tests contain no hardware-specific
wall-clock assertion.
