# 057 — Benchmark scoring paths and add citation metadata

Status: ready

## Why

Users need an honest, reproducible comparison of upstream Pangolin inference,
direct access to the published Zenodo per-gene files, PangoPup's Rust lookup
library, its CLI, and its HTTP service. The README currently reports isolated
PangoPup observations but does not explain the cost added by each layer or the
different storage and input contracts. The repository also lacks standard
software citation metadata naming its author and a compact prior-art section.

## Scope

- Add a retained, dependency-light benchmark harness, machine-readable
  `results.json`, README-number checker, and report under
  `planning/artifacts/057-scoring-path-benchmark/`. Authenticate the exact
  Zenodo archive by its published size and MD5, use the checked 1,000-SNV
  corpus to select deterministic 1/10/100/1,000 lookup workloads, and retain
  exact commands, software/data identities, host facts, raw measurements, and
  a concise interpretation.
- Measure two separate matrices. Stored lookup uses deterministic 1/10/100/
  1,000 rows: the first 994 authoritative hits in checked fixture order plus
  the first six repeated only for the 1,000-row case. It compares a small
  standard-library Python reader over Zenodo's per-gene TSV.GZ members, the
  admitted public Rust provider, Rust routing plus JSON rendering, a fresh
  PangoPup CLI process, and a ready foreground HTTP service (HTTP is limited to
  1/10/100 by its public contract). Model inference uses retained supported
  compatibility cases at fixed feasible 1- and 10-variant sizes: ordered case
  `M01-snv-cd4-precomputed` alone, then ordered cases M01 through
  `M10-insertion-short-both` exactly as retained in
  `tests/fixtures/pangolin-compat-v1/cases.jsonl`. Every path uses distance 50,
  masking enabled, no gene filter, and therefore must preserve every overlapping
  gene result in fixture order. Each timed sample is one batch containing one or
  ten variants, not ten singleton calls. Compare both a retained-open upstream
  Pangolin 1.0.2 Python/PyTorch helper and the unmodified fresh Pangolin CLI,
  the direct PangoPup engine/model with one admitted session, fresh
  `pangopup lookup --model-only` processes, and ready-service `model_only`
  requests. Report model initialization/startup and SQLite reuse separately
  from uncached inference.
- Use identical SNVs wherever the interfaces permit it. State explicitly that
  Zenodo lookup is given the Ensembl gene/member name and returns stored data,
  PangoPup lookup resolves a genomic key, and Pangolin inference computes new
  scores. Do not present unlike work as a speedup ratio.
- Fix measurement boundaries in the harness. Nothing is called cold: the host
  page cache remains uncontrolled and warm. In-process lookup paths receive 20
  untimed warmups and 100 timed samples; fresh CLI paths receive three warmups
  and ten timed samples; ready HTTP lookup uses one persistent HTTP/1.1 client
  connection, 20 warmups, and 100 timed request/complete-response samples,
  excluding client startup. Expensive model paths use one untimed correctness
  preflight and five timed samples. Ready-service uncached model samples use a
  newly started ready service and empty isolated SQLite path per sample; direct
  engine samples retain one admitted session. Each correctness preflight and
  each timed fresh PangoPup CLI model sample receives its own distinct empty
  `--model-cache` path; verify the database is absent/empty before launch and
  contains exactly the expected successful entries afterward. The unmodified
  fresh Pangolin CLI samples include process and checkpoint initialization.
  Separately, an authenticated upstream helper must load the exact twelve
  PyTorch checkpoints once, pass the same oracle preflight, and time the same
  one-batch 1/10 workloads with that loaded model. Before loading, it calls
  `torch.set_num_threads(1)` and `torch.set_num_interop_threads(1)`, records
  both getters, and runs with `OMP_NUM_THREADS=1`, `MKL_NUM_THREADS=1`,
  `OPENBLAS_NUM_THREADS=1`, and `NUMEXPR_NUM_THREADS=1`. The fresh unmodified
  CLI row records its observed thread policy and remains a startup-inclusive
  characterization; do not form ratios unless the measured CPU budgets and
  work are equivalent. Capture outputs in every path and compare them, rather
  than timing `/dev/null`. Use monotonic wall time and the
  nearest-rank p50/p95/p99 where sample count supports it; otherwise report all
  samples, median, and maximum. Record command order, allowed CPUs, governor,
  and effective PyTorch/ORT thread policies. Do not change system affinity or
  governor; fix PangoPup at its default one worker/one model thread and fix
  upstream CPU-thread environment in the retained command.
- Add a compact comparison table and interpretation at the bottom of
  `README.md`, including compressed/installed sizes, memory, latency,
  operational trade-offs, and evidence-backed remaining optimization ideas.
  Preserve the first-use sections above it. Reconcile the existing top
  `Storage and memory` section so it summarizes and links the one detailed
  bottom section rather than retaining contradictory Ticket 053/v0.2.0-era
  observations. A checked script must derive or verify every README benchmark
  literal from retained `results.json`.
- Add `CITATION.cff` for PangoPup v0.3.0 naming Ian Maurer as author and linking
  the repository. Add direct README prior-art links to the Pangolin paper,
  Pangolin GitHub repository, and Zenodo score dataset, with correct author,
  DOI, version, and license attribution.
- Validate `CITATION.cff` structurally offline in the normal gate with a
  development-only parser/test; it must not add a runtime dependency.

## Success Checklist

- The retained report makes every workload, identity, sample count, cache
  state, and measurement command reproducible and separates stored lookup from
  inference.
- Exact lookup outputs are checked against the retained corpus before timing;
  benchmark loops cannot silently time misses or wrong rows.
- Every model path is untimed-preflighted and each timed output is checked for
  expected row count, order, gene multiplicity, masking, distance 50, and its
  path-specific exact or retained-tolerance oracle. Record skipped variants,
  reference/annotation/checkpoint identities, and strand. The Zenodo scores
  are not asserted identical to current Pangolin because the publisher does
  not identify the generating software/checkpoint revision.
- README numbers are mechanically traceable to retained raw results and do not
  overstate precision, portability, or biological equivalence.
- `CITATION.cff` parses as CFF 1.2.0, names Ian Maurer without an invented
  ORCID, and pins the immutable v0.3.0 release, exact publication commit, and
  2026-08-05 release date without misattributing Pangolin or the Zenodo
  dataset.
- No authoritative dataset, model, index, or public release is rebuilt,
  modified, or republished. Use one ignored
  `/home/ian/workspace/data/pangopup-benchmark-057/` scratch root. Retain a
  pre/post inventory and hashes of the production installation; terminate all
  spawned Pangolin/service processes; remove ZIP/partial downloads, extracted
  members, generated input/output, SQLite/sidecars, and the entire scratch root
  after accepted compact evidence is retained, including on handled failure.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Comparison boundary.** Options were one headline speedup or separate
   workload classes. Decision: separate stored lookup from model inference and
   show layer overhead only among comparable lookup paths. A single ratio would
   be technically misleading.
2. **Zenodo baseline.** Options were scanning all gene files, building another
   index, or giving the reader the known per-gene member. Decision: use a tiny
   Python standard-library reader with the Ensembl member supplied. The
   retained-open case opens the ZIP central directory once, then for each
   sample reopens and decompresses exactly the distinct known `.tsv.gz` members
   required by that workload, scans each once, and returns requested rows. The
   harness records that member count for every 1/10/100/1,000 workload; it does
   not open unrelated members for the one-row case. A fresh-process case
   additionally includes Python startup and ZIP central-directory open. No full-dataset
   dictionary is materialized or silently cached. This favors the source
   organization without inventing a competing database; its gene-input and
   linear member-scan requirements remain visible.
3. **Cache state.** Options were one best-case number or cold and warm views.
   Decision: retain both process/open and admitted steady-state results. OS page
   cache cannot be globally dropped safely on the shared host, so cold-disk
   performance is not claimed.
4. **Optimization scope.** Options were to change production code while
   measuring or first establish evidence. Decision: this ticket measures and
   documents only. Any product optimization requires a later bounded ticket
   with exactness and regression gates.

## Dependencies

Public v0.3.0, `snv-grch38-v1`, `runtime-grch38-v1`, the checked 1,000-SNV
fixture, and the published Zenodo archive at record 15649338 (which declares no
version).

## Notes

- Host: the retained Linux Ryzen 7 5825U system; record kernel, CPU, memory,
  filesystem, Rust, Python, PyTorch, Pangolin, and PangoPup identities anew.
  Run paths in a fixed documented order. Isolate memory checkpoints by process
  and collect peak RSS with GNU time plus `/proc` RSS/PSS/virtual mappings for
  long-lived processes. Report mmap file-backed residency separately from heap
  implications; do not reuse a high-water process across path rows.
- Storage inventory must separately report: Zenodo ZIP
  (12,988,141,317 bytes), its central-directory compressed/uncompressed member
  sums and six sampled members; Python environment; twelve Pangolin checkpoints;
  FASTA and gffutils DB; PangoPup SNV/runtime downloads and installed members;
  executable; and SQLite. Mark shared versus additive bytes.
- Zenodo authority: record/DOI `https://doi.org/10.5281/zenodo.15649338`, file
  `Pangolin_hg38_snvs_masked.zip`, published 2025-06-12 by Nils Wagner and
  Aleksandr Neverov under CC BY 4.0, size 12,988,141,317 bytes, MD5
  `679ef0b50e511b6102b4b88fbf811108`. Zenodo's API declares no version; do not
  call it v1.
- Pangolin authority: `https://github.com/tkzeng/Pangolin`, paper DOI
  `10.1186/s13059-022-02664-4`. Prior art must distinguish Tony Zeng's
  GPL-3.0 Pangolin 1.0.2 software, the Tony Zeng/Yang I Li paper, and the
  Wagner/Neverov CC BY 4.0 dataset.
- The current host retains the active installed PangoPup profile and upstream
  Python Pangolin installation. The raw Zenodo archive was intentionally
  removed during cleanup and may be downloaded once into an ignored benchmark
  scratch root, checksum-verified, sampled, then deleted.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05.

## Independent Ticket Review

Reviewer: Codex subagent `/root/ticket057_design_review`, 2026-08-05.

Initial verdict: REJECT. The reviewer found undefined workload/process/cache/
thread/output boundaries; no direct PangoPup model matrix; insufficient model
correctness gates; ambiguous storage and isolated-memory accounting; an
unsupported Zenodo `v1` label; duplicate README-metric risk; and incomplete
scratch/process cleanup.

Resolution: define separate lookup and inference matrices, exact rows and
sample counts, warmup/statistics/output/HTTP boundaries, path-specific
correctness oracles, fixed model-cache isolation, complete storage and process
memory inventories, Zenodo's actual null-version identity, machine-readable
README traceability, offline CFF validation, and one fully removed scratch
root with production-asset before/after proof.

Second review verdict: REJECT. The reviewer required an isolated empty SQLite
path for every fresh CLI model sample, exact named model cases and batching,
an already-loaded single-threaded PyTorch comparison beside the unmodified
startup-inclusive CLI, and removal of the remaining Zenodo-version/member-count
ambiguities.

Second resolution: name M01 and ordered M01-M10, require one batch per sample
with masking/distance/gene behavior fixed, isolate and inspect every CLI model
cache, add a twelve-checkpoint retained-open PyTorch helper with explicit
one/one intra-op and inter-op threads, retain the unmodified CLI only as a
separate startup row, and make the Zenodo identity and per-workload member scan
exact.

Final verdict: ACCEPT. The revised ticket defines fair and reproducible lookup
and inference matrices, exact workloads and execution boundaries, isolated
cache state, correctness oracles, storage/memory accounting, README
traceability, citation validation, and complete scratch cleanup. No material
defect remains.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
