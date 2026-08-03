# 047 — Upgrade ONNX Runtime without changing scores

Status: ready

## Why

The qualified Apple M5 Max Docker retest proved native ARM64 lookup, model,
cache, and HTTP behavior, but ONNX Runtime 1.24.2 prints
`Unknown CPU vendor. cpuinfo_vendor value: 0` on every process start. That
pollutes stderr for `--help`, `status`, quiet sync, precomputed lookup, and the
service even when no model session is opened.

`ort` 2.0.0-rc.13 wraps ONNX Runtime 1.28.0 and a newer cpuinfo dependency, but
ONNX Runtime 1.28.0 still contains the unknown-vendor warning. Therefore this
upgrade is a hypothesis, not an established fix: it becomes quiet only if the
new cpuinfo recognizes Docker Desktop's virtual Apple CPU. The first outcome
must be a cheap Mac probe. Full numerical and release qualification is
justified only if that probe succeeds.

## Scope

- Prepare a minimal candidate replacing exact `ort` 2.0.0-rc.12 / ONNX Runtime
  1.24.2 with exact `ort` and `ort-sys` 2.0.0-rc.13, ONNX Runtime 1.28.0, and
  explicit `api-27`. Record the resolved native archive URLs and SHA-256
  digests for Linux x86-64 and ARM64, plus dependency/license gate results.
- After interim review, the coordinator pushes the minimal dependency-only
  probe commit to a named temporary qualification branch, not `main`. Build its
  native ARM64 image and run `--version` on the Apple M5 Max. If the warning
  remains, delete the temporary branch, leave `main` on rc.12, commit only a
  concise rejection artifact and ticket closeout to `main`, and draft the next
  bounded fix decision. Do not continue the expensive matrix.
- Continue the remaining scope only if the fail-fast Mac probe is quiet.
- Update locked dependencies, runtime-version constants, package/container
  build inputs, and current architecture documentation that declares the live
  runtime. Preserve historical measurement artifacts as historical evidence.
- Run `pangopup-build model qualify` against the unchanged retained production
  bundle and unchanged checked `pangolin-model-v1` evidence. It must reproduce
  14 cases, 18 strands, 36 sequence evaluations, 432 arrays, and 45,756 raw
  scalars with maximum error no greater than the existing `1e-5`; neither
  evidence nor tolerance may be regenerated or loosened.
- Separately prove all 14 model cases / 21 public records, signed loss values,
  positions, ordering, warnings, and provenance remain accepted by the
  existing independent Pangolin public oracle.
- Prove that the model bundle, reference bundle, mask, installed runtime asset
  profile, scoring semantics, and SQLite cache key/value schemas do not change.
  Existing valid model-cache rows must remain hits only because their public
  results are unchanged.
- Compare baseline commit `c4850e4` and the candidate on the same host, affinity,
  production assets, `sequential:1/1` policy, harness, warmups, and sample
  counts. Reject an increase above 10% in either model-open or inference p50,
  above 20% in inference p95, or above 10% in maximum RSS, stripped executable
  size, or final image size. Any noisy or borderline result gets a fresh paired
  rerun and an explicit independent decision; do not tune threads here.
- Qualify the exact reviewed candidate on native AMD64 and ARM64 Linux, then on
  Apple Silicon through Docker Desktop. The Mac run must exercise informational
  commands, lookup-only routing, uncached model inference, cached inference,
  and foreground HTTP startup while retaining stdout/JSON behavior.
- Add a compact retained artifact and, only after the candidate qualifies, a
  new architecture decision explaining the runtime replacement, unchanged
  scoring/cache/asset identities, and observed platform results. Historical
  measurements remain unchanged historical evidence.

## Success Checklist

- Cargo resolves exactly `ort` and `ort-sys` 2.0.0-rc.13 with ONNX Runtime
  1.28.0 and explicit `api-27`; normal builds remain locked and static. Exact
  archive URLs/digests for both native Linux targets and dependency/license
  gate results are retained.
- Existing miniature kernel, model-bundle, variant-scoring, routing, cache,
  executable-delivery, container, and spec tests pass without weakened
  tolerances or regenerated biological expectations.
- The unchanged authenticated raw-kernel evidence qualifies all 45,756 scalars
  within the existing `1e-5` maximum-error contract. The retained production
  qualification separately reproduces all 14 frozen model cases
  and 21 public records. Gain/loss scores, signed loss convention, relative
  positions, record order, warnings, and provenance are identical to the
  accepted public oracle. Any biological/public-output difference rejects the
  upgrade.
- Model/reference/mask/runtime-profile identities, cache application/schema
  IDs, and canonical cache key/value bytes are unchanged. Whole SQLite files
  need not be byte-identical. The executable from baseline commit `c4850e4`
  writes a successful production row; the candidate returns byte-identical
  public JSON from that row while inference is unavailable and uninitialized.
  Candidate `sync --offline` reports the existing runtime profile ready and
  downloads zero bytes.
- `make lint`, `make test`, and `make spec` pass.
- Exact-commit GitHub `ci` and both native miniature `container` jobs pass.
  A `container` workflow dispatch with `production_model=true` passes both
  native production-model jobs for the exact commit. The read-only exact-commit
  `package-linux` workflow also passes and produces its private seven-day
  Actions artifact; nothing is released or published.
- On the Apple M5 Max Docker qualification, the literal
  `Unknown CPU vendor` / `cpuinfo_vendor value: 0` warning is absent from
  stderr for `--version`, top-level and focused help, read-only `status`, quiet
  and offline sync, an SNV lookup, an uncached model request, a persistent
  cached request, and foreground service startup. Scoring and HTTP regression
  checks remain green.
- No stderr filtering, warning-text suppression, process redirection, fake CPU
  identity, or lazy-loading rewrite is used to hide the warning.

## Decisions

1. **Test the candidate before adopting it; never mask the warning.** ONNX
   Runtime 1.28.0 retains the warning branch. rc.13 may help only through newer
   CPU detection. Filtering stderr would conceal future diagnostics and would
   not fix the platform behavior.
2. **Public results are the hard compatibility boundary.** Tiny internal float
   implementation differences are acceptable only within the already accepted
   independent kernel tolerance and only when every public Pangolin result is
   unchanged. No oracle or threshold may be loosened to admit the upgrade.
3. **Do not churn immutable biological assets.** Architecture decision 0014
   already keeps compiler/runtime/CPU identity outside the model artifact. The
   ONNX graph, reference, mask, and transport remain the same bytes.
4. **Preserve persistent cache compatibility conditionally.** Runtime version
   is intentionally absent from the cache key because a qualified runtime is
   an implementation detail. If even one accepted public result changes, the
   upgrade is rejected rather than silently reusing old cached answers.
5. **Use two distinct pushed-candidate stages.** After minimal candidate
   implementation and interim code review, the coordinator pushes the exact
   commit only to temporary branch `qualification/ticket-047-ort-probe`. The
   Mac clones that commit and runs only the ARM64 build and `--version` stderr
   probe; `main` remains on rc.12. If the probe is quiet, the developer
   completes raw/public/cache/performance/local qualification and the same code
   reviewer accepts the final diff. Only then does the coordinator push the
   qualification-ready commit to `main`, which makes the exact commit eligible
   for automatic CI/container and the production-model/package dispatches plus
   the full supplied Mac matrix. Remove the temporary branch after either
   outcome. The developer records whether raw Mac evidence was directly
   accessible or only supplied as a report. Linux ARM64 alone is insufficient.

## Non-goals

- MPS, Metal, CUDA, CoreML, another execution provider, or GPU inference.
- Quantization, graph conversion, model/checkpoint changes, or new assets.
- Thread-policy changes, model pooling, request scheduling, or performance
  promises.
- README restructuring, version bump, executable publication, or GHCR
  publication.
- Suppressing all ONNX Runtime logs; only the upstream Apple CPU defect is in
  scope.

## Dependencies

- Ticket 046: complete; exact-commit CI and native AMD64/ARM64 container
  qualification are green.
- Supplied Ticket 045 Apple Silicon retest establishes the 1.24.2 warning and
  the functional baseline.

## Coordinator Authorship

Coordinator: Codex

Drafted from the pinned dependency/build contracts, accepted runtime and cache
decisions, retained production compatibility test, upstream runtime source and
candidate evidence, and the supplied Apple Silicon evidence. The coordinator
does not implement or approve its own ticket.

## Independent Ticket Review

Reviewer: Pauli the 3rd

Verdict: ACCEPT after remediation. The review corrected the central assumption:
ONNX Runtime 1.28.0 retains the warning branch, so rc.13 is a fail-fast CPU
detection hypothesis rather than a known fix. It also required the unchanged
45,756-scalar oracle, exact public results, dispatched native production jobs,
package qualification, paired performance bounds, precise cache-row
interoperability, archive/API evidence, and a two-stage temporary-branch/main
qualification lifecycle. The final design is bounded and implementable without
runtime redesign.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Qualification

Coordinator: pending

## Coordinator Final Check

Coordinator: pending
