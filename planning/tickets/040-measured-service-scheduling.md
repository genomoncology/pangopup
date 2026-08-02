# 040 — Measured service model partition

Status: ready

## Why

Pangopup's next product outcome is a foreground HTTP service, but its execution
shape is still unresolved. A model request takes seconds while a warm SNV mmap
lookup takes microseconds. A server that protects one complete scorer with a
single mutex could therefore make an ordinary SNV wait behind unrelated model
inference. Guessing a session count could instead waste memory, reduce
single-request speed, or oversubscribe CPUs.

This ticket resolves one prerequisite with one bounded production experiment.
It selects how measured CPU budgets are partitioned between independent model
sessions and ONNX intra-op threads, while recording whether concurrent model
load materially interferes with the separately opened SNV provider. It does not
design the service queue or open an HTTP port.

## Scope

- Add one explicitly ignored, coordinator-only Linux x86_64 measurement harness
  under `pangopup-engine` integration tests. This location owns only the
  existing model/reference/mask composition being measured; the harness is not
  a service runtime. It accepts the four qualified production asset paths
  through environment variables, authenticates their exact identities, and
  must not contain developer-specific absolute paths.
- Compare every integer partition below in a fresh process under the stated
  physical-core affinity. `workers × threads` is the complete model CPU budget;
  every worker owns one reference/mask/model composition and one ONNX session.

  | CPU budget | candidates |
  |---|---|
  | 1 | `1×1` |
  | 2 | `1×2`, `2×1` |
  | 4 | `1×4`, `2×2`, `4×1` |
  | 8 | `1×8`, `2×4`, `4×2`, `8×1` |

- Use a measurement-only fixed dispatcher: one OS thread owns each scorer,
  measured cases are assigned round-robin in stable case order, all workers
  start each measured concurrent batch at one barrier, and the harness joins
  every result before beginning the next model operation. This dispatcher is
  not selected or reused as service code.
- In every fresh candidate process, open worker sessions sequentially, warm
  every worker once with `M09`, then perform exactly:
  - three serial measured `M09` requests through worker zero;
  - the idle warmed SNV series;
  - one concurrent batch containing `M07` through `M14` once each;
  - while that concurrent model batch remains in flight, the coordinator runs
    the loaded warmed SNV series, then joins the model results.
- Each SNV series contains batches of 1, 10, and 100 requests, each repeated 25
  times. An atomic active-worker count is incremented before common barrier
  release and decremented only after a worker's final assigned result. Every
  loaded SNV sample must begin and end while that count is nonzero; otherwise
  the round is invalid rather than silently relabeled as loaded.
- Derive the SNV corpus deterministically by pairing
  `tests/fixtures/snv-regression/requests.tsv` with `expected.jsonl`, selecting
  the first 100 source-order rows whose status is `found` and whose records are
  nonempty, and using its 1/10/100 prefixes with their existing gene filters.
  Every lookup must match its frozen expected result exactly and remain on the
  authoritative precomputed path.
- Record startup time, p50/p95/p99 wall latency, saturated model throughput,
  peak RSS, minor/major faults, inference counts, asset identities, affinity,
  CPU/kernel/runtime versions, candidate, round, and exactness status as
  canonical JSON Lines. Run three fresh-process rounds per candidate.
- Use nearest-rank percentiles, `ceil(p × n) - 1` after ascending sort. Within a
  round, the three lone-request samples produce p50 while p95/p99 are the
  maximum and explicitly descriptive; the eight concurrent request latencies
  produce p50 while p95/p99 are likewise descriptive maxima; each 25-sample
  SNV series produces ordinary nearest-rank p50/p95/p99. Do not pool samples
  across fresh processes.
- Aggregate a candidate's three rounds as: median of round lone-request p50,
  median of round complete concurrent-batch elapsed nanoseconds, maximum of
  round concurrent p95, maximum peak RSS, and maximum fault counts. A candidate
  is exact only if all three rounds are exact. Report throughput as eight
  requests divided by the aggregate batch elapsed value. Every concurrent
  request latency begins at common barrier release and ends when that request's
  result arrives, so it includes time waiting behind an earlier case assigned
  to the same worker.
- Select one partition for each CPU budget mechanically:
  1. reject any candidate with incomplete rounds, exactness failure, or an
     operational failure;
  2. reject peak RSS above 1 GiB;
  3. reject a candidate whose lone `M09` p50 exceeds 125% of that budget's
     fastest eligible lone-request p50;
  4. identify the throughput leader as the candidate with the lowest median
     concurrent-batch elapsed value;
  5. a candidate is within 5% of that leader when checked `u128` arithmetic
     satisfies `100 × leader_elapsed >= 95 × candidate_elapsed`; among that set
     prefer lower maximum concurrent p95, then lower maximum RSS, then fewer
     sessions.
- Rerun the selected mapping through all 14 scored compatibility cases and
  every ordered expected record. If a CPU budget has no eligible candidate,
  try `1×1` as a conservative fallback under that budget's affinity. If it
  fails exactness, operation, or the RSS bound, the ticket is blocked for that
  budget and must not invent a supported mapping.
- Treat every selected partition as qualified only for the retained host's
  exact CPU identity and non-SMT affinity. Do not derive a portable runtime
  mapping from `available_parallelism`, logical CPU count, or cgroup quota.
  ADR 0017's portable ordinary `1×1` policy remains unchanged for unmatched
  hosts; Ticket 040 does not supersede it.
- Retain `planning/artifacts/040-service-scheduling-raw.jsonl` and a concise
  `planning/artifacts/040-service-scheduling.md` report. The report must explain
  in plain English what was compared, the selected mapping, lookup behavior
  under model load, memory cost, and limitations of one-host measurements.
- Add ADR `architecture/decisions/0024-measured-service-scheduling.md` and
  update `architecture/service.md`, `planning/frontier.md`, `README.md`, and
  `AGENTS.md` so the next ticket can distinguish the portable `1×1` default
  from the retained host-qualified model-partition evidence while it designs
  the service-owned scheduler.
- Exclude service queues, backpressure capacity, admission granularity,
  dispatch policy, concurrent-fill coalescing, SQLite connection ownership,
  HTTP protocol/framework, `pangopup serve`, public CLI changes, Docker/systemd,
  accelerator work, model conversion, asset rebuilding, publication,
  production resource-limit claims, and any public external effect.

## Success Checklist

- Harness unit tests prove nearest-rank indexing, three-round aggregation,
  exact 5% comparison arithmetic, candidate tie breaking, stable round-robin
  assignment, overlap-count rejection, and rejection of incomplete measurement
  sets.
- Every candidate measurement validates the exact production SNV, model,
  reference, mask, compatibility-corpus, ORT, and CPU-policy identities before
  emitting a result. Invalid paths, identities, affinity, worker counts, thread
  counts fail before measurement.
- The raw artifact contains three complete fresh-process rounds for all ten
  candidates. The report applies the ticket's selection rules without manual
  exceptions and identifies a selected or explicit conservative fallback
  partition for budgets 1, 2, 4, and 8.
- The selected mapping reproduces all 14 scored compatibility cases and every
  ordered record exactly. No score tolerance or rounded spot check substitutes
  for the typed oracle.
- The 1/10/100-SNV measurements are reported both idle and during concurrent
  model work from the exact named fixture prefixes, and every loaded sample has
  a nonzero active-worker count at both boundaries. They characterize CPU
  interference only; the durable service design continues to require lookup
  outside model admission, while production queue isolation is proved in the
  later service ticket.
- The production experiment is `#[ignore]`, is absent from `make lint`, `make
  test`, and `make spec` execution, and cannot run without explicit asset-path,
  candidate, and affinity inputs. It is not exposed as `verify`, a CLI command,
  or a routine maintenance task.
- Normal bounded tests, `make lint`, `make test`, and `make spec` pass without
  production assets, network access, Python, or a long-running scan.

## Decisions

### Measure scheduling before HTTP

**Consideration:** HTTP needs a concurrency policy, but the seconds-versus-
microseconds cost split makes a guessed global lock particularly harmful.

**Options:** ship HTTP around one mutex; choose a worker count by intuition; or
measure model-session partitions first.

**Trade-offs:** the experiment delays the port-opening outcome by one ticket,
but prevents the transport from freezing an unmeasured execution policy.

**Decision and why:** measure first. Ian selected a balanced latency/throughput
policy, with memory secondary to eligible model performance. The later service
ticket must keep lookup outside its model scheduler.

### Compare equal total CPU budgets

**Consideration:** one multithreaded session and several narrower sessions use
CPU differently. Comparing `1×8` against `4×2`, for example, is fair only when
both receive the same total core budget.

**Options:** compare worker counts at the ordinary `1/1` policy; compare only
the prior host winner `8/1`; or compare all integer worker/thread partitions of
1, 2, 4, and 8 cores.

**Trade-offs:** the full partition matrix opens more production sessions, but
it distinguishes single-request latency, concurrency, and memory while reusing
the already qualified fixed thread policies.

**Decision and why:** compare all ten listed partitions in fresh processes and
select independently for each retained host affinity. These are host-qualified
tuning results, not portable defaults. ADR 0017's ordinary `1×1` remains the
portable behavior for other CPU topologies and quotas.

### Keep this ticket out of service ownership

**Consideration:** `pangopup-engine` owns scoring composition but explicitly
does not own cache or concurrency policy. The future service layer owns queues
and concurrent-fill behavior.

**Options:** implement and retain a scheduler in the engine; create a production
service-runtime crate now; or limit this ticket to a disposable measurement
dispatcher and leave service behavior to the HTTP ticket.

**Trade-offs:** a reusable scheduler would settle more behavior but would make
this measurement ticket a second product implementation. A disposable
dispatcher measures the model partition without violating crate ownership.

**Decision and why:** this ticket selects only the worker/thread partition.
The harness dispatcher is test-only. The next service ticket owns and reviews
bounded admission, dispatch, SQLite connections, failure fan-out, and exact
in-flight coalescing.

### Retain evidence, not another validator

**Consideration:** the user explicitly rejected long-running verification that
is easy to rerun accidentally.

**Options:** add a normal verification command; run an undocumented one-off;
or retain an ignored authenticated harness plus immutable measurements.

**Trade-offs:** an ignored harness remains reproducible but requires explicit
maintainer inputs. It cannot slow ordinary development or user operation.

**Decision and why:** retain the ignored harness and report. It is never part
of normal gates and creates no runtime command.

## Dependencies

- Ticket 021: qualified fixed ONNX CPU policies and the host-specific `8/1`
  frontier observation.
- Ticket 039: explicit model-only routing and the settled future HTTP boolean.

## Notes

- Work in `/home/ian/workspace/repos/pangopup` and preserve unrelated changes.
- The retained production installation is currently available under a
  coordinator-controlled data root, but the harness must receive paths through
  environment variables and authenticate identities. Do not commit local
  paths, caches, copied assets, or production bytes.
- Use the existing `M07`–`M14` and full 14-case compatibility oracle. Do not
  invoke upstream Python/PyTorch or recapture the corpus.
- Use fresh process execution and `taskset` affinities matching the retained
  Ticket 021 non-SMT host convention: `0`; `0,2`; `0,2,4,6`; and
  `0,2,4,6,8,10,12,14`. Fail rather than silently measure another affinity.
- Measure warmed SNV pages honestly and label them warm. Do not claim cold I/O
  without a controlled page-residency procedure.
- This ticket selects a model partition but does not add a permanent service
  runtime, cache composition, or HTTP dependency. Measurement dispatch code
  stays confined to the ignored integration test; only its small pure
  aggregation helpers receive normal bounded tests.
- The repository gate is exactly `make lint`, `make test`, and `make spec`.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 039 result, the current service frontier, the
retained production assets, and Ian's choices to measure before HTTP and rank
balanced latency/throughput. Revised after independent review to define exact
statistics and restrict the outcome to model-partition evidence rather than
placing service queues or caching in `pangopup-engine`.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket040_design_review`

Initial verdict: **REJECT**.

The reviewer found under-specified warmups, repetitions, percentile and
cross-round aggregation; service queue/cache/coalescing policy assigned to the
engine despite its explicit boundary; unresolved production scheduler choices;
no mapping for 3/5/6/7 visible CPUs; an unnamed SNV workload; and an impossible
requirement to select a fallback even if it failed its own gates.

Coordinator disposition: accepted. The revised ticket defines every sample,
nearest-rank calculation, aggregation and tie rule; removes queue, cache and
coalescing behavior; names an exact fixture-derived SNV workload; defines the
host-qualification boundary; and makes an ineligible fallback block that budget
rather than manufacture support. A second revision makes the idle/loaded
overlap schedule executable, measures request latency from common release,
defines the exact elapsed-time comparison arithmetic, and retains ADR 0017's
portable `1×1` default rather than extrapolating physical-core results through
logical CPU counts.

Second re-review verdict: **ACCEPT**. The reviewer confirmed that the exact
overlap schedule, latency origin, statistics, elapsed-time comparison, fixed
SNV corpus, host qualification, fallback behavior, and architectural
exclusions resolve every material finding. The ticket is implementation-ready.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
