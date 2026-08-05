# 053 — Measure current runtime resource use

Status: complete

## Why

The v0.3.0 README must state realistic download, installed-disk, and memory
requirements. Historical measurements cover individual lookup and model
experiments, but no one retained run measures the current complete CLI and
foreground service with the qualified installed assets. This is the next
coherent slice because release documentation must not turn old laboratory
numbers into current product claims.

## Scope

- Add `maintainers/ticket-053/measure.py`, a bounded `uv` maintainer harness for
  Linux that observes a locked release build of the reviewed Ticket 053 commit
  as a separate process. Add its parser/aggregation tests in
  `maintainers/ticket-053/test_measure.py`. It must not add a runtime dependency
  or change scoring behavior.
- Reuse the retained qualified active data and cache profiles. Do not download,
  rebuild, repack, verify exhaustively, or mutate public assets.
- Reject a dirty worktree or non-release build. Record the binary's reported
  version and SHA-256 and bind them to the clean full Git commit that built it;
  reject any mismatch with the run contract.
- Measure five rounds. Each round uses one fresh foreground service for the
  first four defined checkpoints:
  - ready with the model already loaded and no requests served;
  - after ordered 1-, 10-, and 100-SNV precomputed requests;
  - after one first uncached supported model request; and
  - a second fresh ready service serving that pre-populated SQLite result.
  Also measure one fresh CLI process for the one-SNV request in each round.
- For short CLI children, record GNU `time` elapsed time, peak RSS, and minor
  and major faults. At every service checkpoint, record elapsed request time,
  `/proc/<pid>/smaps_rollup` RSS/PSS, `/proc/<pid>/status` virtual size and high
  water RSS, and `/proc/<pid>/stat` process-relative minor/major faults. Compare
  monotonic fault counters only within the same service process.
- Record exact disk/member sizes, host, kernel, CPU, compiler, full application
  commit, binary SHA-256, asset identities, and the portable `1×1` model
  policy.
- Report medians and maxima without claiming cold-cache performance. State
  plainly that file-backed mmap pages in RSS/PSS are reclaimable and that this
  host's page cache is warm.
- Retain the human-readable result in
  `planning/artifacts/053-current-runtime-resources.md` and the complete compact
  samples as JSON Lines schema `pangopup.runtime-resources.v1` in
  `planning/artifacts/053-current-runtime-resources.jsonl`.
- Update `planning/frontier.md` to distinguish the resulting current evidence
  from future multi-host capacity work.
- Do not edit `README.md`, version numbers, release workflows, assets, formats,
  model code, routing, or service policy in this ticket.

## Success Checklist

- The harness fails closed when the executable, installed profile, cache root,
  expected asset identities, Linux `/proc` fields, request outputs, or sample
  counts do not match its declared contract.
- Focused tests cover parsing and aggregation of representative GNU `time` and
  `/proc` samples, including missing/malformed fields and a same-process counter
  decrease. A lower counter in a different fresh process is not an error.
- All measured SNVs retain `precomputed` provenance. The uncached and cached
  non-SNV results are byte-identical and retain `model` provenance. Before the
  fresh-service hit, the disposable SQLite database contains exactly the
  expected entry; its file bytes remain unchanged by the hit, and the cached
  request completes in less than one tenth of that round's uncached request.
- The retained report gives exact download and installed byte counts and
  measured lookup/service/model/cache memory with method and limitations.
- The measurement does not alter the retained installed data; any disposable
  measurement cache is isolated and explicitly removed or retained by path.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Measure the whole product boundary, not library allocations.** Rust heap
   counters omit mmap residency, ONNX native memory, SQLite, and process
   overhead. A separate-process Linux observer is less surgical but answers
   the user's deployment question.
2. **Use retained qualified assets, not a rebuild.** Rebuilding would consume
   substantial time and could change the object under measurement. Exact
   installed identities make reuse both faster and more trustworthy.
3. **Report RSS, PSS, and mapping size together.** RSS alone can make a 15 GB
   mmap look like heap consumption; virtual size alone is also misleading.
   The three values explain owned/shared resident pages and address-space
   mapping without pretending mapped bytes equal required RAM.
4. **Keep the result host-qualified.** Five rounds reduce incidental noise,
   but one warm Linux host is not a universal capacity guarantee. The README
   may use the result as an observed baseline with explicit headroom, not as a
   hard minimum for every platform.

## Dependencies

Ticket 052.

## Notes

- Retained qualified data is under
  `/home/ian/workspace/data/pangopup-release-050-c50dd13/data/pangopup`; the
  matching retained cache root is under
  `/home/ian/workspace/data/pangopup-release-050-c50dd13/cache/pangopup`.
- The SNV lookup member is 15,033,158,255 bytes. The model, reference, and mask
  members must be derived from the admitted active profile rather than guessed
  from filenames.
- Authenticate SNV bundle
  `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`,
  runtime profile
  `sha256:0efc5b7d9e966935775f9b19ef33eae75cb304cc5d5ba3f1d700ccddc6ddbd8c`,
  model bundle
  `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`,
  reference bundle
  `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`,
  and mask member SHA-256
  `714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.
- Use a separate temporary `XDG_CACHE_HOME` or explicit model-cache path for
  every round's uncached/cache-hit pair so retained evidence is not
  contaminated.
- Reuse `planning/artifacts/004-query-manifest.tsv` for the exact 1/10/100 SNV
  prefixes. Use compatibility case `M09-insertion-short-plus`, exactly
  `GRCh38:chr12:6801303:G:GA`, as the model/cache request.
- Existing `planning/artifacts/004-snv-lookup-performance.md` and
  `planning/artifacts/040-service-scheduling.md` are historical context, not
  substitutes for this complete-product run.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05. Drafted from the accepted v0.3.0
release plan and current clean `main` at `0aad10c`.

## Independent Ticket Review

Reviewer: `/root/ticket053_design_review` (independent, read-only).

Initial verdict: REJECT. The reviewer required service checkpoints that match
eager model loading, an exact clean-build identity, metric-specific lifecycle
rules, concrete files/workloads/profile identities, and stronger proof that the
fresh service used SQLite rather than inference. The coordinator revised each
point in the ticket. Re-review verdict: ACCEPT. The reviewer found the revised
scope self-contained, bounded, correctly sequenced, and sufficient to support
host-qualified README claims.

## Implementation Evidence

Developer: `/root/ticket053_implementation_2`.

- Added the bounded Linux `uv` observer and seven parser/aggregation contract
  tests under `maintainers/ticket-053/` without changing a runtime crate.
- A one-round complete smoke and the final five-round retained run passed
  against the authenticated installed profile. The final run used a clean
  detached worktree at commit `d4af71ac91e7d65ae6c7546fc3fc01aa481d3d3f`,
  built `--release --locked`, and bound binary SHA-256
  `db9dcd3d7346af1132290edc6b5ebd3ced26c0a685b0b8fd6b3e247ee6534e60`.
- All 35 service checkpoints, five short CLI samples, metadata, and aggregate
  summary are retained in the 42-line JSONL artifact. Every SNV retained
  `precomputed` provenance. Each round created exactly one isolated SQLite
  entry; uncached and cached model bytes matched, cached requests were more
  than ten times faster, and database bytes did not change during each hit.
- The retained data tree's path/size/mtime/mode metadata was identical before
  and after collection. Per-round disposable caches were removed by the
  harness.
- The first code review rejected unsafe exceptional child lifecycles, copied
  download-size constants, and an under-specified model workload. Remediation
  added a bounded startup read, unconditional session cleanup, safe temporary-
  file stderr capture, SIGTERM-to-SIGKILL wait escalation, and four explicit
  fake-child lifecycle tests. It also authenticates both retained transport
  manifests and every member before deriving download totals, and pins M09's
  gene filter, exact record, provenance, and stable result digest.
- Final authenticated transport identities were SNV manifest
  `f9b7501087226fb35cbfa66fa9b903cc21eb8bbbacb067363b9eeef487ee9e9a`
  and runtime manifest
  `415860610ccc060ff3ed5678b450650265330d43f7e73bc533c4ff0125e300a3`.
  All ten uncached/cached M09 samples retained result SHA-256
  `8b92aa263001ac961541128e90aaee23ab36e3f7882e316860a1787934e8eeac`.
- Focused gate: `uv run maintainers/ticket-053/test_measure.py` — 12 passed.
- Reviewer cleanup gate: `uv run ruff check maintainers/ticket-053` and
  `uv run ruff format --check maintainers/ticket-053` — passed after removing
  one unused import and applying the formatter; retained artifacts unchanged.
- Retained outputs:
  `planning/artifacts/053-current-runtime-resources.md` and
  `planning/artifacts/053-current-runtime-resources.jsonl`.

## Adversarial Code Review

Reviewer: `/root/ticket053_code_review` (independent, read-only).

Initial verdict: REJECT. The reviewer found an unbounded startup read and
exception paths that could leave a service alive, download totals copied from
constants rather than authenticated retained transport, and an insufficiently
pinned model workload. The developer added bounded and unconditional process
cleanup with four lifecycle tests, authenticated both transport inventories
and every compressed member, and pinned M09's exact gene/result/provenance
identity before regenerating the evidence. Re-review verdict: ACCEPT. The
reviewer independently reproduced all 12 focused tests and checked every raw
count, aggregate, fault sequence, cache ratio, result digest, and report value.
A final unused-import cleanup remained accepted after the same reviewer
verified Ruff and focused tests again.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex (`/root`), 2026-08-05.

- `uv run ruff check maintainers/ticket-053` — passed.
- `uv run ruff format --check maintainers/ticket-053` — passed.
- `uv run maintainers/ticket-053/test_measure.py` — 12 passed.
- `make lint` — passed (format, Clippy, advisories, bans, licenses, sources).
- `make test` — passed, including executable-delivery and production-release
  qualification scripts; only declared retained-production measurements were
  skipped.
- `make spec` — 275 passed, 7 skipped.
- `git diff --check` — passed.
- Stale-claim scan confirmed the frontier now calls these results a current
  single-host warm-cache baseline and keeps universal/multi-host capacity work
  explicitly future. README/version/release claims remain intentionally
  unchanged for Ticket 054.
