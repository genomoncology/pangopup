# 021 — Select the CPU policy from complete variant measurements

Status: complete

## Why

Pangopup's complete lookup-first CLI and Pangolin-compatible model fallback now
work, but model inference still uses the deliberately conservative
single-thread ONNX Runtime baseline. On this host, that raw Rust baseline took
about 2.2–2.3 seconds per context, while upstream PyTorch using its default
threads took about 0.66–0.68 seconds. Those old numbers are not complete
variant latency: one variant requires reference and alternate inference and can
require both strands.

The next useful result is therefore not a new runtime or cache. It is a small,
repeatable comparison of CPU session policies on the real complete scorer,
followed by one evidence-backed default. Lookup-hit behavior must remain
byte-for-byte unchanged.

## Scope

### One bounded CPU-policy seam

- Add one small public immutable `CpuPolicy` value in `pangopup-model`
  containing sequential/parallel execution mode, an intra-op choice of
  affinity-aware ONNX Runtime `Auto` or `Fixed(NonZeroUsize)`, and a nonzero
  inter-op thread count. Add
  `ModelKernel::open_with_cpu_policy(&Path, CpuPolicy)` as the explicitly named
  low-level/maintainer seam used by cross-crate qualification.
- Keep ordinary callers on one compiled-in production default. Add an explicit
  policy constructor for tests and maintainer measurement; document it as a
  real low-level library API rather than hiding it behind `cfg(test)` or a
  feature. Do not add CLI flags, environment-variable configuration, global
  runtime state, a session pool, or a general backend abstraction.
- The initial candidate set is exactly:
  - sequential `auto/1`;
  - sequential `1/1`, `2/1`, `4/1`, and `8/1`; and
  - parallel `1/2`, `1/4`, and `1/8`.
  Inter-op threads are fixed to one for sequential mode because ONNX Runtime
  documents that they have no effect there. The two bounded families test
  within-node parallelism separately from the combined graph's independent
  branches without an oversubscribed intra-times-inter matrix.
- Preserve graph optimization `All`, the default CPU execution provider,
  authenticated descriptor-held model loading, initialization probe, mutable
  single-owner kernel, model artifact identity, and all public score semantics.
- The only possible changed ordinary default in this ticket is sequential
  `auto/1`, which lets ONNX Runtime respect the runtime CPU affinity. Fixed
  candidates characterize this host and inform later service concurrency; they
  are not promoted as a universal thread count. If `auto/1` does not qualify,
  ordinary callers retain sequential `1/1`.

### Complete-request measurement and selection

- Add one ignored, maintainer-only release measurement in
  `pangopup-engine`. It opens the retained authenticated production model,
  reference, and mask assets once per process and accepts exactly one candidate
  policy per invocation.
- Measure component-open/initialization separately from warmed
  `VariantScorer::score` calls. Time the frozen compatibility cases
  `M09-insertion-short-plus` and `M10-insertion-short-both`; they represent
  complete one-strand and two-strand non-SNV work. Use two warmups and seven
  consumed samples per case, and report p50, p95, process high-water RSS, CPU
  affinity, policy, runtime versions, asset identities, and exact case/record
  qualification as one machine-readable JSON object.
- The coordinator runs every candidate in a fresh process with exact affinity
  `0,2,4,6,8,10,12,14`, one logical CPU from each of this host's eight physical
  cores, and without concurrent benchmark work. The harness must report and
  reject any different allow-list and reject an unknown or malformed policy
  rather than silently using the production default. Retain
  `lscpu -e=CPU,CORE,SOCKET,ONLINE` with the commands and results.
- A candidate is eligible only if both cases reproduce the existing exact
  public compatibility records and its high-water RSS is no more than twice
  the sequential `1/1` baseline. It must improve p50 by at least 20% on both
  cases to displace the baseline. Report p95 descriptively; with seven samples
  it is the maximum observation and is deliberately not a selection gate.
- If multiple candidates qualify, name the **measured frontier winner** as the
  one with the lowest worse of its two p50 ratios versus baseline. Break an
  exact tie by lower RSS, then prefer `auto` over a fixed intra-op count, then
  fewer fixed threads, then sequential before parallel. This winner
  characterizes this host; it need not be the shipped default.
- Separately name the **selected ordinary default** as sequential `auto/1` if
  that affinity-aware candidate itself qualifies, otherwise sequential `1/1`,
  even if a host-specific fixed candidate is the measured frontier winner.
- Re-run the selected ordinary default once in a fresh process, then run the
  existing retained 14-case production qualification with that ordinary
  default. It must reproduce all 14 cases and accepted asset identities.
- Retain commands, raw candidate JSON lines, host/runtime facts, the mechanical
  selection calculation, the selected-policy rerun, and the 14-case result in
  `planning/artifacts/021-measured-cpu-policy.md`. Measurements are one-host
  evidence, not a portable latency promise or a wall-clock test gate.

### Fast tests and documentation

- Normal model tests use only the checked miniature ONNX bundle. They prove
  policy validation, candidate parsing, the ordinary default, real miniature
  inference under both execution families, and unchanged output/shape
  validation without production assets, Python, PyTorch, a network request, or
  timing assertions. Code review verifies the direct mapping to the three ONNX
  Runtime session-builder calls; ONNX Runtime does not expose applied session
  options for a useful runtime assertion.
- Existing engine, CLI, 1,000-SNV regression, model-routing specs, and
  compatibility fixtures remain unchanged unless a narrow assertion is needed
  to prove the selected default. Do not add a routine long-running verifier.
- Add ADR `architecture/decisions/0017-measured-cpu-policy.md`. Update
  `README.md`, `architecture/README.md`, `architecture/design.md`,
  `architecture/runtime-data.md`, `planning/faq.md`, and
  `planning/frontier.md` so they name the selected policy and measured complete
  request evidence without presenting host-specific timings as guarantees.
- The updated frontier chooses the next single outcome from the evidence:
  reference/alternate scheduling or graph batching if CPU remains materially
  inadequate; otherwise proceed to the existing cache/delivery outcome order.
  Do not draft that next ticket here.

Explicitly excluded: changing ONNX graph batch shape, concurrent requests,
session pooling, model-result caching, SQLite, MPS/CUDA, quantization,
alternative runtimes, coherent asset download/activation, HTTP, Docker,
systemd, asset publication, HGVS, normalization, and any external effect.

## Success Checklist

- Every bounded candidate produces exact M09 and M10 public records and one
  complete retained JSON measurement under the same host conditions.
- The deterministic rule names one measured frontier winner across all
  qualifying candidates. Separately, the selected ordinary default is
  qualifying affinity-aware `auto/1` or retained sequential `1/1`; ordinary
  `ModelKernel::open` never adopts a host-specific fixed count.
- The selected ordinary default passes the retained 14-case production
  qualification and accepted model/reference/mask identities.
- Fast synthetic tests cover policy validation, candidate parsing, both modes,
  selected default, and unchanged inference errors; no normal test needs
  production assets or measures time.
- Existing lookup JSON/table bytes and the 1/10/100/1,000 SNV performance
  regression remain unchanged.
- ADR, architecture, README, FAQ, retained evidence, and frontier agree about
  what was measured, what shipped, and what remains.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Measure complete scoring, not extrapolated kernel calls.** Raw context
   timing is useful diagnosis but hides reference/alternate and strand
   multiplicity. The selected default is based on one- and two-strand
   `VariantScorer` requests.
2. **Test two small policy families, not a Cartesian tuning search.**
   Sequential intra-op and parallel inter-op candidates exercise the two useful
   ONNX Runtime controls while avoiding oversubscription and benchmark
   infrastructure.
3. **Require a clear, portable win before changing the baseline.** Correctness,
   bounded RSS, and p50 improvement on both representative requests matter more
   than a noisy maximum mislabeled as a stable p95. Only ONNX Runtime's
   affinity-aware automatic intra-op policy can replace fixed `1/1`; fixed
   results remain host characterization.
4. **Expose the necessary library seam but keep it out of the CLI.** A public
   typed low-level constructor is honest and usable by cross-crate
   qualification. Ordinary users still receive one reviewed default;
   operational concurrency belongs to the later service outcome.

## Dependencies

- Ticket 018 authenticated raw CPU model kernel.
- Ticket 019 variant-level model scoring and compatibility qualification.
- Ticket 020 lookup-first CLI model routing.

## Notes

- Retained production assets already exist under
  `/home/ian/workspace/data/`; do not rebuild, convert, download, publish, or
  mutate them.
- The existing production qualification is
  `crates/pangopup-engine/tests/production_qualification.rs`; extend it only
  where needed to select the ordinary default without weakening its exact
  identity checks.
- ONNX Runtime 1.24.2 documents that inter-op threads affect parallel graph
  execution while intra-op threads affect work within nodes. The shipped
  native runtime and `ort` crate remain pinned. Retained evidence identifies
  that exact static archive as the measured threading implementation and notes
  the `ort` warning that explicit intra-op counts may be ineffective for
  OpenMP builds; the current archive has no unresolved GOMP/OMP symbols.
- Production measurements are coordinator-owned small outputs and must be
  generated after focused implementation tests and before adversarial code
  review.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 020 outcome, retained Ticket 018/020 evidence,
the current model/scorer code, local `ort` 2.0.0-rc.12 API documentation, and
the rolling frontier.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket021_design_review`

Initial review rejected four details while accepting the bounded outcome:

- seven samples made p95 a noisy single-maximum gate;
- a host-specific fixed thread winner was unsafe as a universal default and
  omitted ONNX Runtime's affinity-aware automatic policy;
- the cross-crate policy constructor and its test proof were ambiguous; and
- “eight physical cores” did not identify this SMT host's actual CPU affinity.

The coordinator made p95 descriptive, added `auto/1` as the only possible
changed ordinary default, named the public typed low-level constructor and
implementable tests, and fixed affinity to
`0,2,4,6,8,10,12,14` with topology/runtime evidence. Re-review then found one
terminology ambiguity between a host's measured frontier winner and the
portable selected ordinary default. Those are now separately named with
complete selection, tie, rerun, and qualification rules.

Final re-review: **ACCEPT**. The reviewer found the ticket to be the smallest
correct next outcome, deterministic, portable at the shipped boundary,
implementable, reproducible, and consistent with the one-ticket process.

## Implementation Evidence

Developer: Codex sub-agent `/root/ticket021_implementation`

Implemented the typed `CpuPolicy`/`ModelKernel::open_with_cpu_policy` seam,
closed eight-candidate parser, direct ONNX Runtime execution/intra/inter mapping,
fast miniature inference and validation controls, ADR 0017, and an ignored
complete-`VariantScorer` production measurement. The harness authenticates and
opens the three production components once, rejects a noncanonical policy or
affinity, checks every M09/M10 warmup and sample against the frozen public
records, and emits one JSON object with component-open timing, p50/descriptive
p95, RSS, runtime/asset identities, and qualification.

Focused non-production evidence:

- `cargo test --locked -p pangopup-model --test bundle`: 11 passed;
- `cargo test --locked -p pangopup-engine --lib`: 10 passed;
- `cargo test --locked -p pangopup-engine --test cpu_policy_measurement
  --no-run`: compiled;
- locked clippy for `pangopup-model` and `pangopup-engine`, all targets, with
  warnings denied: passed; and
- `cargo fmt --all` plus `git diff --check`: passed.

The developer also ran the complete non-production gate after the focused
checks: `make lint` passed, `make test` passed with only the declared
maintainer/production tests ignored, and `make spec` reported 167 passed and 2
skipped.

No production asset was opened, downloaded, rebuilt, changed, or published.
The retained artifact contains the exact coordinator commands and results.
The coordinator-owned production run found sequential fixed `8/1` to be this
host's measured frontier winner. Affinity-aware `auto/1` failed the M10
improvement gate, so ordinary `ModelKernel::open` correctly remains sequential
`1/1`. A fresh selected-default rerun was exact, and the ordinary-default
production qualification passed all 14 cases and 21 records with the accepted
model/reference/mask/post-ensemble identities. ADR 0017, README, architecture,
runtime-data, FAQ, and frontier now distinguish that portable default from the
host-specific winner and select reference/alternate graph batching as the next
bounded outcome.

Code-review remediation closes the public constructor's numeric boundary
before `ort` can cast `usize` to its signed 32-bit C thread-count parameter.
Fast tests cover the exact accepted maximum and the first rejected intra-op
and inter-op values while rechecking all eight constants/parser spellings.
`AGENTS.md` now also reports CPU-policy selection as established and graph
batching as remaining work.

## Adversarial Code Review

Reviewer: Codex sub-agent `/root/ticket021_code_review`

Initial review found three issues:

- the public constructor accepted thread counts above ONNX Runtime's signed
  32-bit C parameter domain before `ort` narrowed `usize`;
- the retained artifact summarized CPU topology instead of preserving the
  required exact `lscpu` command and output; and
- `AGENTS.md` still described CPU tuning as future work.

The same developer added typed numeric-domain validation and independent
intra/inter boundary tests, and corrected `AGENTS.md`. The coordinator appended
the exact previously captured 16-row topology output. Focused remediation
checks passed.

Final re-review: **ACCEPT**. The reviewer independently confirmed all three
findings closed, model tests 12/12, the ignored harness compiled without
running, locked all-target model/engine clippy passed with warnings denied, and
the diff was clean. The reviewer found no remaining correctness, security,
performance-evidence, lookup-regression, architecture, or documentation issue
and did not open production assets.

## External Effect Evidence

Coordinator: not applicable

This ticket performs no download, upload, release, or other external mutation.

## Coordinator Final Check

Coordinator: Codex `/root`

After independent code-review acceptance:

- `make lint`: passed;
- `make test`: passed, with only declared maintainer/production tests ignored;
- `make spec`: 167 passed, 2 skipped;
- `git diff --check`: passed; and
- stale-claim scan: CPU-policy selection is consistently established,
  host-specific `8/1` is not presented as the portable default, ordinary
  `1/1` remains explicit, and reference/alternate graph batching is the one
  next outcome.

The coordinator also confirmed the retained raw candidate lines, arithmetic,
fresh selected-default rerun, 14-case/21-record production qualification,
accepted asset identities, topology output, and OpenMP-symbol check. No asset
was rebuilt, changed, downloaded, uploaded, or published.
