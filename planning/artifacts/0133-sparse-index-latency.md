# Complete sparse-index latency

The complete sparse candidate passed every predeclared latency gate on the retained Ryzen reference host. It was 3.61 to 3.80 times slower than fixed-v1, but remained below both the independent ten-times control and the historical absolute ceiling for every workload.

| Requests | Fixed-v1 p50 | Sparse p50 | Sparse/fixed | Absolute ceiling | Result |
|---:|---:|---:|---:|---:|---|
| 1 | 461 ns | 1,663 ns | 3.61× | 2,100 ns | pass |
| 10 | 4,379 ns | 16,222 ns | 3.70× | 19,640 ns | pass |
| 100 | 41,341 ns | 157,189 ns | 3.80× | 195,880 ns | pass |

The benchmark opened each complete file once, proved that both readers returned identical ordered results for all 100 retained gene-filtered SNV requests, and then measured the first 1, 10, and 100 requests. Each reader received 20 warmups and 20 retained samples per workload. Reader order alternated for every sample. The report uses nearest-rank p50 and records every nanosecond sample.

The command ran from clean pushed commit `142787ce82471544a9bb8356b8dc8ef834eef1ca` with release executable SHA-256 `416cd8f7a43a30d27bd946fe3c70046624086ee2a51af8b349e067d24ab9fe7d`. The executable was 27,736,056 bytes. Its embedded builder-source SHA-256 was `e9f514ea9a5d32a08c4b5fc7132e2d9445e0f1c95ce99520fbb815fe0755fb12`. It used Rust 1.93.1 for `x86_64-unknown-linux-gnu`.

The fixed input was 15,033,158,255 bytes with SHA-256 `6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27`. The sparse input was 2,035,371,437 bytes with SHA-256 `354343dc1a9f6558e46693e2481b5181461be2115cd92e029ffd4d4abef4a01e`. The query manifest and retained gene selection matched the identities declared by Ticket 0133.

The host ran Ubuntu 24.04 on an AMD Ryzen 7 5825U with 16 allowed logical CPUs, 29,340,880,896 bytes of memory, and a Crucial CT1000P3PSSD8 device. The kernel was Linux 6.17.0-35-generic. The CPU governor was `powersave` with `balance_performance` energy preference. The benchmark kept the historical default `0-15` process affinity.

Immediately before the run, five one-second observations reported 96% to 98% idle CPU after the cumulative first line. No build, test, database load, or other project measurement was active. The retained process list contains long-lived interactive programs whose displayed CPU values are lifetime averages. The run exited successfully after 18.61 seconds, used at most 29,740 KiB of resident memory, reported 88 major page faults, and reported a GNU time file-system input counter of 28,727,712. GNU time does not label that counter as bytes. File hashing and preflight work account for most of the wall-clock run and file input. They sit outside the retained lookup samples.

Before measurement, the exact pushed Linux checkout passed all 19 focused sparse-latency tests, the command parser test, strict all-target `pangopup-build` Clippy, and `git diff --check`. The release executable then built from that clean checkout. After retaining the result, macOS passed `make lint`, `make test`, `make spec`, and `git diff --check`. The configured duplicate-dependency notices from `cargo deny` remained warnings.

The exact command was:

```text
target/release/pangopup-sparse-latency \
  --fixed /home/ian/workspace/data/pangopup/bundles/c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3/bundle/scores.pgi \
  --candidate /home/ian/workspace/data/pangopup/sparse-candidate-2026-09-16/scores.pgi \
  --queries planning/artifacts/002-query-manifest.tsv \
  --selection planning/artifacts/002-selected-genes.tsv \
  --output planning/artifacts/0133-sparse-index-latency-report.json \
  --command-commit 142787ce82471544a9bb8356b8dc8ef834eef1ca
```

The canonical report is [`0133-sparse-index-latency-report.json`](0133-sparse-index-latency-report.json). It is 3,214 bytes with SHA-256 `b37b26a9802668b66491abb456c6b25e919d082394d5dea35e594a5d3d66af2b`.

This result covers warm gene-filtered SNV library lookups after one open. It does not measure unfiltered lookup, whole-genome throughput, cold storage, program startup, HTTP service behavior, activation, packaging, or rollback. The sparse unfiltered route traverses a different directory path and cannot inherit this result. This measurement proves only the latency gate. ADR 0027 still requires the separate corruption, installed identity, semantic service, and rollback evidence before this format can replace fixed-v1.
