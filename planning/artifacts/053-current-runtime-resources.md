# Ticket 053 — Current runtime resource measurements

## Result

This is a five-round, warm-page-cache observation of the current complete Linux product. It is an observed baseline, not a universal minimum or a cold-cache benchmark.

## Identity

- Commit: `d4af71ac91e7d65ae6c7546fc3fc01aa481d3d3f`
- Binary version: `0.2.0`
- Binary SHA-256: `db9dcd3d7346af1132290edc6b5ebd3ced26c0a685b0b8fd6b3e247ee6534e60`
- Host: `beelink`
- Kernel: `6.17.0-35-generic`
- CPU: `AMD Ryzen 7 5825U with Radeon Graphics`
- Rust: `rustc 1.93.1 (01f6ddf75 2026-02-11)`
- Model policy: `sequential:1/1` (one worker, one model thread)

- SNV transport manifest SHA-256: `f9b7501087226fb35cbfa66fa9b903cc21eb8bbbacb067363b9eeef487ee9e9a`
- Runtime transport manifest SHA-256: `415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3`
- Model workload: `M09-insertion-short-plus`, `GRCh38:chr12:6801303:G:GA`, filtered to `ENSG00000010610` with one exact expected record

## Exact asset sizes

| Component | Download bytes | Installed runtime-member bytes |
|---|---:|---:|
| SNV lookup | 1,931,694,270 | 15,033,158,255 |
| Model/reference/mask | 691,874,664 | 812,662,222 |
| Combined | 2,623,568,934 | 15,845,820,477 |

Including receipts, manifests, notices, and lock files, the complete installed data tree is 15,845,837,837 bytes.

The 15 GB SNV index is a fixed-width, direct random-access file. PangoPup maps it into virtual address space; Linux loads only pages that a query touches. The mapped virtual size is therefore not the same thing as RAM use. File-backed resident pages reported in RSS/PSS are reclaimable by the operating system.

## Measurements

Medians and maxima are across five fresh-process rounds. Memory values are KiB.

| Checkpoint | elapsed ms median / max | RSS median / max | PSS median / max | virtual median / max | peak RSS median / max |
|---|---:|---:|---:|---:|---:|
| service-cache-ready | — | 108,336.0 / 108,500.0 | 104,803.0 / 104,991.0 | 16,722,196.0 / 16,722,196.0 | 140,264.0 / 140,576.0 |
| service-model-cached | 0.7 / 0.9 | 108,404.0 / 108,568.0 | 104,871.0 / 105,059.0 | 16,789,796.0 / 16,789,796.0 | 140,264.0 / 140,576.0 |
| service-model-uncached | 4,301.2 / 5,704.6 | 109,760.0 / 109,940.0 | 106,227.0 / 106,405.0 | 16,789,800.0 / 16,789,804.0 | 140,208.0 / 140,472.0 |
| service-ready | — | 108,344.0 / 108,532.0 | 104,721.0 / 105,002.0 | 16,722,200.0 / 16,722,204.0 | 140,208.0 / 140,472.0 |
| service-snv-1 | 0.8 / 1.4 | 108,480.0 / 108,644.0 | 104,814.0 / 105,114.0 | 16,789,800.0 / 16,789,804.0 | 140,208.0 / 140,472.0 |
| service-snv-10 | 0.5 / 2.1 | 108,504.0 / 108,684.0 | 104,858.0 / 105,154.0 | 16,789,800.0 / 16,789,804.0 | 140,208.0 / 140,472.0 |
| service-snv-100 | 1.1 / 3.6 | 108,660.0 / 108,836.0 | 105,026.0 / 105,306.0 | 16,789,800.0 / 16,789,804.0 | 140,208.0 / 140,472.0 |
| snv-1 | 5.9 / 6.2 | — | — | — | 12,292.0 / 12,588.0 |

## Method and limitations

- Every round started a fresh service with the production model loaded before the ready checkpoint.
- The same service then served ordered 1-, 10-, and 100-SNV precomputed requests and pinned compatibility case `M09-insertion-short-plus`, filtered to its expected `ENSG00000010610.10` record. A second fresh service reused that round's single SQLite entry.
- Download bytes were derived from and checked against every digest-authenticated member in the retained SNV and runtime transport manifests; they were not copied from documentation constants.
- `/proc/<pid>/smaps_rollup`, `/proc/<pid>/status`, and `/proc/<pid>/stat` supplied service memory and fault counters. GNU `time` observed the short CLI process.
- GNU `time` retained its elapsed counter for the CLI, but it rounded these sub-10-ms processes to 0.00 seconds; the table uses the observer's higher-resolution monotonic wall clock for CLI elapsed time.
- The host page cache was warm and could not be defensibly cleared. Timing is descriptive, not a cold-start guarantee.
- RSS includes reclaimable file-backed mmap pages; PSS apportions shared resident pages. Virtual size includes mappings and is not physical-memory demand.
- The disposable measurement caches were isolated and removed after successful collection. Retained installed data was read only.
