# Ticket 023 persistent model-result cache qualification

## Result

The retained `r5` CPU-0 production qualification passed with the selected
insertion/update-order SQLite cache and read-only valid-hit path.

| Case | Phase | Samples | Complete p50 | Complete p95 | SQLite open + validated hit p50 | SQLite p95 | SQLite max |
|---|---:|---:|---:|---:|---:|---:|---:|
| M09 | uncached | 3 | 10,965.831 ms | 11,869.797 ms | — | — | — |
| M09 | first fill | 3 | 10,798.751 ms | 11,450.008 ms | — | — | — |
| M09 | same-process hit | 3 | 0.173 ms | 0.189 ms | 0.167 ms | 0.181 ms | 0.181 ms |
| M09 | reopened hit | 30 | 7.572 ms | 8.995 ms | 0.640 ms | 0.827 ms | 0.907 ms |
| M10 | uncached | 3 | 16,192.534 ms | 16,242.435 ms | — | — | — |
| M10 | first fill | 3 | 14,904.621 ms | 20,195.928 ms | — | — | — |
| M10 | same-process hit | 3 | 0.247 ms | 0.259 ms | 0.237 ms | 0.247 ms | 0.247 ms |
| M10 | reopened hit | 30 | 7.677 ms | 12.927 ms | 0.679 ms | 1.084 ms | 1.188 ms |

Both reopened cases performed zero model-kernel constructions, ONNX sessions,
initialization probes, and inference calls. Exact phase output hashes matched.
The reopened-hit speedups over uncached p50 were 1,448.23× for M09 and
2,109.20× for M10. First-fill p50 was lower than uncached p50 in both cases, so
measured miss overhead was below zero rather than approaching the 10-percent
ceiling.

The 1,000-entry resource case used 2,142,208 bytes for database plus WAL plus
SHM and 3,186,688 bytes of incremental RSS. Both are below their independent
16 MiB bounds.

## Final schema-hardening follow-up

Code review subsequently required established opens to validate the complete
STRICT v1 table shapes and metadata sequence, not only column names. That
changes the reopen path measured by `r5`, so the final release binary received
a narrow fresh-process spot check against the preserved prepared production
caches. This did not rerun inference or restore the removed qualification
harness.

| Case | Fresh processes | Complete p50 | Complete p95 | Complete max | Exact output |
|---|---:|---:|---:|---:|---:|
| M09 | 30 | 11.406 ms | 11.790 ms | 11.966 ms | yes |
| M10 | 30 | 10.945 ms | 15.153 ms | 16.255 ms | yes |

The complete process interval includes bounded component identity admission,
SQLite open and schema validation, validated lookup, rendering, and process
startup. It therefore upper-bounds SQLite open plus validated hit and remains
below 20 ms even at the observed maximum. Every fresh invocation returned the
retained exact output in milliseconds, versus `r5` uncached p50 values of
10.966 seconds for M09 and 16.193 seconds for M10; this demonstrates the final
path did not construct or execute the model.

The reviewer's raw M09 microsecond samples are 180 bytes with SHA-256
`33bf161da73ce8faae02b1754996f9d906dd25c3163b9e175fca69c21fde9f60`.
The compact M10 receipt is retained at
`/home/ian/workspace/data/pangopup-cache-023-final-schema-spot.json`
(1,136 bytes, SHA-256
`1a3fea5c9e603f29dfbf1921ded169dc89cd9d88f353e1ae3717a35b39ea7bce`);
all 30 outputs had SHA-256
`0c3db84b045df0b4d13966da5a1e25ceb83407541f417c458493b28e5e1029ce`,
matching `r5`.

## Pinned identity

- scoring: `pangopup-variant-score-v1`, distance 50
- CPU policy: `sequential:1/1`
- model bundle:
  `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`
- model profile: `pangolin-1.0.2-5cf94b8-onnx-cpu-v1`, singleton
- reference bundle:
  `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`
- reference sequence set:
  `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`
- mask: 6,703,320 bytes,
  `sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`

## Reproduction and retained receipt

The one-time ignored harness ran with `taskset -c 0`, three rotated M09/M10
inference rounds, and ten independent reopened-process hit samples per case per
round. It used the pinned SNV, model, reference, and mask assets and a new
absent private root:

```text
PANGOPUP_CACHE_HARNESS_ROOT=/home/ian/workspace/data/pangopup-cache-023-r5
taskset -c 0 cargo test --locked --release -p pangopup-cli --bin pangopup \
  tests::model_cache_production_measurement -- --ignored --exact --nocapture
```

The preserved complete log is
`/home/ian/workspace/data/pangopup-cache-023-r5.log` (102,605 bytes,
SHA-256 `02cc8ebcda532e4554273a9c570898eadc5a37f53ad9dd8161a5bd08e51485cd`).
The extracted complete machine receipt is
`/home/ian/workspace/data/pangopup-cache-023-r5-receipt.json` (51,413 bytes,
SHA-256 `e8d53b18225925c278f07905058e6591c8c70e048733610bdf3b7ca807abe416`).

The expensive harness was removed after this passing receipt. Normal gates
retain fast unit, integration, CLI, and executable-spec coverage; production
qualification is evidence, not a tempting repeatable verify-all command.
