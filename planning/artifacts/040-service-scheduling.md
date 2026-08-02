# Ticket 040 — Measured service model partition

Date: 2026-08-02

## Plain-English result

Pangopup compared one wide model session with several narrower independent
sessions at equal physical-CPU budgets. The selected shapes are `1×1`, `1×2`,
`1×4`, and `2×4`, where the first number is sessions and the second is ONNX
intra-op threads per session.

Most multi-session candidates made an ordinary single request more than 25
percent slower than the fastest candidate at their budget and were rejected.
At eight cores, `2×4` stayed inside that guard and completed the eight-request
batch materially faster than `1×8`, so the mechanical rule selected `2×4`.
This evidence does not change the portable `1×1` default. It qualifies these
four mappings only for this exact AMD Ryzen 7 5825U host under the named
non-SMT affinities.

The SNV mmap stayed independent and fast while the model batch was in flight.
Across the selected mappings, median round p50 was 682–741 ns for one lookup,
6,693–7,304 ns for ten, and 67,562–89,185 ns for 100 while a model batch was
in flight. The measurement dispatcher is ignored test code, not a production
scheduler.

## Contract and host

All 30 fresh processes authenticated the same production SNV, singleton ONNX
model, RefSeq GRCh38.p14 reference, GENCODE v38 mask, frozen compatibility
manifest, and the manifest-declared 220,071-byte `cases.jsonl` member with
SHA-256 `2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8`.
Every process ran on Linux x86-64, kernel 6.17.0-35-generic, ONNX
Runtime 1.24.2, `ort` 2.0.0-rc.12, and the AMD Ryzen 7 5825U. Affinities were
`0`, `0,2`, `0,2,4,6`, and `0,2,4,6,8,10,12,14` for budgets 1, 2, 4, and 8.
The harness rejects any other CPU model, online set, package/core mapping, SMT
sibling pairing, or process affinity before measurement.

Every raw row also binds the complete measurement implementation with SHA-256
`22a6271153a0cfb3795e392c472a4de2ae595b927aff40b6712310b1698a5bd4`.
The executable computes this value directly over the exact UTF-8 bytes of
`service_scheduling_measurement.rs` embedded at compile time with
`include_bytes!`; no expected digest is stored in that source, so the identity
is reproducible without a self-referential hash. A normal test compares the
embedded digest with the checked-out file.

Each candidate ran three fresh-process rounds. Workers opened sequentially,
warmed once with M09, worker zero served three measured M09 requests, and the
workers then served M07–M14 from one common release with stable round-robin
assignment. The coordinator measured the frozen 1/10/100-SNV prefixes idle and
while every loaded sample began and ended with a nonzero active-worker count.
Before either series it performed one unmeasured exact pass over all 100 rows,
so “warm” does not depend on incidental host page-cache state.

## Aggregates and mechanical selection

Times below are the specified three-round aggregates. Throughput is eight
divided by aggregate batch elapsed. RSS and concurrent p95 are maxima.

| Budget | Candidate | Lone M09 p50 ns | Batch elapsed ns | Requests/s | Concurrent p95 ns | Max RSS KiB | Result |
|---:|---:|---:|---:|---:|---:|---:|---|
| 1 | `1×1` | 4,266,758,794 | 47,480,958,861 | 0.168 | 47,865,019,625 | 140,760 | selected |
| 2 | `1×2` | 2,186,217,554 | 24,143,709,328 | 0.331 | 24,769,608,658 | 140,812 | selected |
| 2 | `2×1` | 4,396,620,592 | 27,865,464,645 | 0.287 | 28,493,924,813 | 220,108 | lone latency rejected |
| 4 | `1×4` | 1,376,777,576 | 17,531,606,442 | 0.456 | 20,383,252,801 | 141,092 | selected |
| 4 | `2×2` | 2,698,302,021 | 19,925,044,672 | 0.402 | 21,444,973,812 | 220,292 | lone latency rejected |
| 4 | `4×1` | 4,606,837,701 | 15,292,440,768 | 0.523 | 20,175,455,694 | 378,692 | lone latency rejected |
| 8 | `1×8` | 1,343,534,400 | 14,385,072,994 | 0.556 | 17,059,043,134 | 141,716 | throughput rejected |
| 8 | `2×4` | 1,323,834,680 | 10,880,465,250 | 0.735 | 12,737,435,167 | 220,760 | selected |
| 8 | `4×2` | 2,336,260,838 | 9,708,862,560 | 0.824 | 10,235,027,101 | 379,064 | lone latency rejected |
| 8 | `8×1` | 4,157,944,107 | 10,759,352,973 | 0.744 | 11,710,191,416 | 696,044 | lone latency rejected |

All candidates completed three exact operational rounds and stayed below the
1-GiB RSS ceiling. Maximum major faults were one for `1×1` and zero for every
other candidate; the raw artifact retains every round's minor and major counts.

Selected-mapping median round SNV p50 values were:

| Mapping | State | 1 SNV ns | 10 SNVs ns | 100 SNVs ns |
|---|---|---:|---:|---:|
| `1×1` | idle | 691 | 6,824 | 68,525 |
| `1×1` | loaded | 701 | 7,104 | 69,727 |
| `1×2` | idle | 762 | 7,545 | 71,941 |
| `1×2` | loaded | 682 | 6,693 | 67,562 |
| `1×4` | idle | 782 | 7,986 | 78,034 |
| `1×4` | loaded | 741 | 7,304 | 89,185 |
| `2×4` | idle | 742 | 7,435 | 67,512 |
| `2×4` | loaded | 691 | 6,923 | 70,889 |

## Exactness rerun

Fresh selected-mapping processes reran all 14 scored compatibility cases. Each
of `1×1`, `1×2`, `1×4`, and `2×4` reproduced all 21 ordered public records
exactly, with no score tolerance or rounded spot check. Qualification records
are appended to the retained raw JSON Lines artifact.

## Reproduction boundary and limits

The ignored test requires explicit candidate, round, affinity, and four asset
paths. It is not a CLI command and normal lint/test/spec execution never runs
production inference. The retained raw evidence is
[`040-service-scheduling-raw.jsonl`](040-service-scheduling-raw.jsonl).

This was one host, one runtime, warmed SNV pages, one eight-case concurrency
mix, and no HTTP transport. It selects a model partition only. The next service
ticket still owns queue capacity, backpressure, admission, dispatch, SQLite
connection ownership, concurrent-fill coalescing, and failure fan-out. It must
keep SNV lookup outside model admission.
