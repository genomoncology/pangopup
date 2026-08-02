# 041 — Foreground HTTP scoring service

Status: complete

## Why

Pangopup already provides lookup-first scoring, persistent SQLite reuse, synced
XDG assets, and measured model-worker shapes, but callers can only invoke that
behavior through a new CLI process. The next product slice is one long-lived
foreground HTTP process that keeps the mmap and model sessions open.

The service must remain simple. An SNV hit or completed SQLite value should not
wait behind model inference. Only uncached model work needs admission to a
fixed number of workers and a bounded waiting line. The caller receives one
ordinary HTTP response when work finishes; Pangopup does not create durable
jobs or require polling.

## Scope

- Add `pangopup serve` to the existing executable. It runs in the foreground,
  never forks or writes a PID file, opens one coherent installed four-asset
  profile before listening, and performs no network asset synchronization.
  Missing or incompatible assets fail startup with the existing redacted error
  classes and direct the user to `pangopup sync`.
- Add one testable service module owned by the CLI/transport layer. It may use a
  small asynchronous HTTP framework, but scoring, masking, lookup, index, and
  cache formats remain in their existing crates. Do not add HTTP or concurrency
  policy to `pangopup-engine`.
- Add these exact first-version routes:
  - `GET /livez` returns HTTP 200 and `{"status":"live"}` while the event loop
    is serving, including during graceful drain.
  - `GET /readyz` returns readiness only after the immutable providers and
    workers are open. Ready is HTTP 200 and `{"status":"ready"}`. During drain
    or after worker failure it is HTTP 503 and `{"status":"not_ready"}`.
  - `GET /v1/status` returns software version, non-secret installed asset
    identities, enabled lookup/model routes, and the configured/current model
    worker, thread, running, queued, and queue-capacity counts.
  - `POST /v1/score` accepts a closed JSON object containing `variants`, an
    optional stable Ensembl `gene`, and optional `model_only` boolean. Variants
    use the existing literal `GRCh38:<CONTIG>:<POS>:<REF>:<ALT>` grammar. The
    response is one closed JSON object containing ordered results with the same
    result/provenance fields and score strings as the shipped CLI JSONL
    contract. Success is the canonical compact JSON object
    `{"results":[<existing CLI JSON result objects>]}` followed by one newline.
- Every non-HEAD HTTP body is canonical compact JSON followed by one newline
  and uses `Content-Type: application/json`. Errors use the closed envelope
  `{"error":{"code":"<STABLE_CODE>","message":"<BOUNDED_MESSAGE>"}}`.
  The status/code contract is: malformed/unknown/duplicate/invalid input is
  `400 INVALID_REQUEST`; body overflow is `413 REQUEST_TOO_LARGE`; too many
  uncached model variants is `422 MODEL_BATCH_TOO_LARGE`; saturated model
  admission is `429 MODEL_QUEUE_FULL`; shutdown admission is
  `503 SHUTTING_DOWN`; model/operational scoring failure is
  `500 SCORING_FAILED`; an unknown route is `404 NOT_FOUND`; and a known route
  with the wrong method is `405 METHOD_NOT_ALLOWED`. Health/readiness/status
  responses use the same JSON and newline rules.
- HTTP `HEAD` on a known route is the sole body exception required by HTTP: it
  returns `405 METHOD_NOT_ALLOWED`, `Content-Type: application/json`, and the
  `Content-Length` of the canonical method-not-allowed error representation,
  but transmits no body bytes. Every 405 includes the route's exact `Allow`
  header: `GET` for `/livez`, `/readyz`, and `/v1/status`; `POST` for
  `/v1/score`. This is not an implicit successful GET/HEAD route. Pin status,
  content type, content length, `Allow`, and zero HEAD body bytes in a
  network-level negative test. Unknown-route HEAD remains 404 and has no
  `Allow` requirement.
- `/v1/status` is the closed canonical object
  `{"version":"<VERSION>","readiness":"ready|draining|failed","assets":{"snv_bundle_id":"<ID>","model_bundle_id":"<ID>","reference_bundle_id":"<ID>","mask_sha256":"<ID>"},"routes":{"lookup":true,"model":true,"model_only":true},"model":{"effective_cpu_policy":"sequential:<THREADS>/1","workers":<N>,"threads_per_worker":<N>,"running":<N>,"queued":<N>,"queue_capacity":<N>}}`.
  Asset paths, cache paths, environment, request contents, and host details are
  never included.
- Stable error codes are the compatibility contract. Messages are printable,
  newline-free, at most 256 bytes, and never contain paths or request bodies.
  Fixed service states use these exact messages: `request body is too large`,
  `request requires more than 10 uncached model variants`, `model queue is
  full`, `service is shutting down`, `model worker failed`, `route not found`,
  and `method not allowed`. Safe validation/scoring detail may vary behind its
  stable code; executable fixtures pin representative complete bytes.
- After binding, stdout emits exactly one machine-readable line
  `{"event":"listening","address":"<BOUND_ADDRESS>"}`. Operational logs go to
  stderr and must not contain request bodies or asset paths.
- Bound request bodies at 64 KiB and batches at 100 variants. Reject duplicate
  JSON fields, unknown fields, empty batches, and invalid variants without
  scoring anything. Preserve request order and all-or-error response behavior.
  After lookup and the first cache check, reject a request with more than 10
  uncached model variants as `422 MODEL_BATCH_TOO_LARGE` before queue admission
  or inference. The larger 100-variant limit remains useful for cheap SNV and
  cached batches without allowing one request to monopolize a worker for an
  unbounded number of model calls.
- Open the SNV provider once and keep lookup outside model admission. Resolve
  authoritative SNV hits immediately. Check the existing SQLite cache before
  queue admission. A request with remaining model misses becomes one queue job;
  one worker processes that request's misses in input order and the response is
  assembled only after every result is available.
- Open SQLite connections sequentially before bind: one handler-side connection
  plus one connection owned exclusively by each model worker. Handler cache
  reads run in a blocking task behind a one-permit nonwaiting gate; if that gate
  is occupied or SQLite reports busy, treat the value as a miss without taking
  model admission or blocking the async event loop. Each worker rechecks and
  writes through only its own connection. No connection mutex is shared with a
  model worker, and no connection is held across inference. A configured
  incompatible explicit cache fails startup; the disposable default retains
  its existing safe recovery behavior.
- Use a fixed worker count and ONNX intra-op thread count for the process. The
  portable defaults are one worker and one thread. Expose explicit positive
  integer `--model-workers`, `--model-threads`, and
  `--model-queue-capacity` options; default queue capacity is 16 waiting HTTP
  requests. Workers are limited to 1–8, threads per worker to 1–8, and waiting
  capacity to 1–1024; reject out-of-range values before opening assets or
  binding. Do not infer a host-qualified
  Ticket 040 mapping from logical CPU count. Document `2×4` as the measured
  explicit choice for the retained Ryzen eight-physical-core host only.
- Each worker opens its model with the actual sequential fixed intra-op
  `CpuPolicy` selected by `--model-threads`; worker count is service capacity,
  not scoring identity. The installed profile's `sequential:1/1` remains the
  qualified portable default and asset compatibility authority. An explicit
  thread override changes only the operational session policy, must be exposed
  as `effective_cpu_policy` in model result provenance and `/v1/status`, and
  must be used in every SQLite cache key. Never read or write an entry under a
  false `1/1` key. Add the effective policy to the shared modeled provenance so
  ordinary CLI output honestly records its unchanged `sequential:1/1` policy.
- Count capacity as occupied worker slots plus waiting jobs. Public `running`
  means an accepted request owns an immediate worker slot, including the short
  handoff before its OS worker begins executing; it is bounded by `workers`.
  Public `queued` means only accepted requests consuming waiting capacity; it
  is bounded by `queue_capacity`. Admission atomically reserves an available
  worker slot first, then a waiting slot, otherwise returns a stable `429 Too
  Many Requests`. Completion/cancellation atomically promotes the next FIFO
  waiting request into the released worker slot or releases that running slot,
  so status and shutdown never under-report accepted work. SNV hits and cache
  hits still succeed while the waiting line is full. No request priority or
  dynamic worker resizing exists.
- Recheck SQLite inside the worker before each inference so an earlier request
  may satisfy a queued duplicate. Do not add a separate in-memory LRU or an
  in-flight coalescing registry. Cache locks must never be held during model
  inference. Cache failures retain the shipped policy: a busy disposable cache
  degrades to model work, and a cache write failure cannot invalidate a valid
  model result.
- The asynchronous handler waits on a one-shot completion from the blocking
  model worker. If the client disconnects while its job is still waiting, the
  worker detects the closed response receiver and discards the job before any
  inference. If inference has already started, it finishes and may populate
  SQLite even after disconnection. Pangopup adds no server-side inference
  timeout in this slice. Clients and deployment proxies may choose their own
  request timeout.
- Handle SIGINT and SIGTERM by stopping admission, making readiness false,
  allowing accepted jobs to finish, and then exiting. During drain, `/livez`
  and `/v1/status` remain available, `/readyz` returns not-ready, and new
  `/v1/score` calls receive `503 SHUTTING_DOWN`; close the listener after the
  accepted queue drains. A second SIGINT or SIGTERM forces process exit so one
  wedged inference cannot prevent an operator from stopping the process. Both
  signals share this exact state machine. Do not add start, stop,
  restart, daemon, systemd, container, metrics, authentication, TLS, job IDs,
  polling, priorities, automatic sync, or distributed queue behavior.
- Update `README.md`, `architecture/service.md`, `architecture/README.md`,
  `planning/frontier.md`, and `AGENTS.md`. Add executable behavior to
  `spec/http-service.md`. If a durable concurrency choice needs an ADR, add one
  under `architecture/decisions/` and link it from the service document.

## Success Checklist

- Normal production builds must not contain a miniature-profile trust bypass.
  An explicit, nondefault test feature may compose the actual child
  `pangopup serve` executable with a grammar-valid miniature installed profile;
  that child must bind `127.0.0.1:0`, emit the exact listening line, serve all
  four routes, and receive real OS signals. The feature and its bounded delay
  control must be absent from normal builds. Separate subprocess tests invoke
  the normal actual executable and prove option parsing plus missing and
  incompatible production profiles fail before bind. Production startup uses
  only the exact activated production profile.
- Add one explicitly ignored retained-production qualification that invokes
  the actual `pangopup serve --listen 127.0.0.1:0` against caller-supplied
  production XDG data, observes the listening line, and probes all four routes.
  Its single score call is one ordered two-variant batch: a known authoritative
  SNV hit from the checked regression fixture followed by the supported M09
  non-SNV from the compatibility corpus. Require ordered `precomputed` then
  `model` provenance so this proves the actual installed HTTP-to-worker-to-
  reference/mask/model composition, not merely listener and mmap startup. Run
  it once for Ticket 041 evidence on the retained host; normal gates and CI do
  not require or download the multi-gigabyte production assets.
- HTTP response fixtures prove byte-stable success and error envelopes,
  CLI-equivalent result/provenance fields, input order, `model_only`, body and
  batch limits, duplicate/unknown-field rejection, and malformed variant
  handling.
- A deterministic blocking fake-model integration test proves that an SNV hit
  and an existing SQLite hit complete while all model workers and the waiting
  line are occupied, while the next uncached model request receives immediate
  `429` without inference.
- Worker tests prove the configured worker limit, exact running/queued status,
  FIFO admission, queued cache recheck, no cache lock during inference, and
  both disconnect cases: a queued job is discarded without inference, while a
  running job finishes and writes a successful result.
- Model-job failure stops that request at the first failed variant and returns
  `500 SCORING_FAILED` without a partial response. Successfully completed
  earlier variants may remain in the disposable SQLite cache; the cache is a
  reusable performance side effect rather than a transactional result store.
- Catch unexpected worker panic/exit at the service boundary. Worker loss sets
  readiness to `failed`, permanently closes model admission, completes the
  current and every queued caller with HTTP 503
  `MODEL_WORKER_UNAVAILABLE`/`model worker failed`, and begins the same graceful
  service shutdown. Do not respawn workers in this ticket.
- A deterministic worker-loss test proves panic/exit cannot strand the queue:
  readiness becomes failed, all accepted responses complete with
  `MODEL_WORKER_UNAVAILABLE`, new admission is refused, and shutdown begins.
- Child-process lifecycle tests prove readiness transitions, new admission
  stops during shutdown, accepted work drains, persistent SIGINT/SIGTERM
  listeners share the same graceful path, and a real second signal forces
  exit. Actual-executable tests prove startup fails before bind on
  missing/incompatible production assets; the ignored retained qualification
  proves the successful production composition without weakening trust.
- Normal tests use only miniature/fake providers and complete quickly; they do
  not download assets or run the production model. One ignored maintainer test
  may exercise the retained production assets, but it is not required for the
  normal gate.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Ordinary HTTP response, not jobs.** Model inference is slow, but it is
   bounded and already cacheable. The first service keeps the connection open
   and returns the final answer. Job IDs, polling, durable job state, and
   callback delivery would add lifecycle and persistence problems without a
   demonstrated need.
2. **Asynchronous transport, blocking workers.** The HTTP event loop remains
   responsive while fixed OS workers own mutable ONNX sessions. This matches
   the model's single-owner contract and lets ordinary synchronous or
   asynchronous clients use the same endpoint.
3. **Bounded request queue.** An unbounded channel could turn a traffic spike
   into unbounded memory and waiting time. One fixed FIFO waiting line with an
   immediate `429` is observable and sufficient; there is no general-purpose
   scheduler.
4. **Lookup and cache before admission.** Microsecond mmap hits and completed
   persistent results must not consume scarce model capacity. This preserves
   lookup-first behavior and the Ticket 040 evidence that lookup can remain
   responsive during inference.
5. **No automatic host tuning.** ADR 0024 is qualified for one exact Ryzen
   topology. Portable service defaults remain `1×1`; users may explicitly
   select worker/thread counts, and later retained measurements may justify a
   safer host-policy layer.
6. **No cancellation of active inference.** The current ONNX boundary has no
   proven safe interruption contract. Finishing and caching useful work is
   simpler and deterministic when a client disconnects. Work that has not yet
   started is discarded when its receiver closes, preventing abandoned queued
   requests from consuming later model capacity.
7. **Bound expensive work separately from lookup batch size.** A request may
   carry 100 cheap or cached variants but at most 10 uncached model variants.
   This keeps the queue's worst-case work understandable without splitting one
   caller into individually scheduled jobs.

## Dependencies

- Ticket 023 persistent model-result cache.
- Tickets 034–035 installed runtime synchronization and composition.
- Ticket 039 explicit model-only routing.
- Ticket 040 measured worker/thread partitions.

All are complete.

## Notes

- Work in `/home/ian/workspace/repos/pangopup` and preserve unrelated changes.
- The repository is public. Tests and retained output must not contain local
  asset paths, environment dumps, credentials, or request bodies in logs.
- The developer may refactor private CLI composition into a reusable internal
  library boundary when necessary, but must not change shipped `lookup`,
  `sync`, or `status` routing, scores, errors, ordering, or precomputed output.
  This ticket explicitly authorizes one additive modeled-output contract
  change: `effective_cpu_policy` is added to model provenance in both CLI JSONL
  and HTTP results, and existing CLI unit/spec fixtures must be updated to prove
  the ordinary command still reports `sequential:1/1`.
- `--listen` defaults to `127.0.0.1:8080`. Non-loopback binding is explicit;
  this ticket adds no authentication or TLS and documentation must warn users
  not to expose the endpoint directly to untrusted networks.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted from the shipped Ticket 040 result and the rolling frontier. The ticket
deliberately chooses one foreground request/response server and one bounded
model waiting line rather than a general scheduler.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket041_design_review`

Accepted after three read-only passes. Review findings established queued-job
cancellation, the ten-model-miss request cap, actual CPU-policy identity,
second-signal forced exit, exact HTTP envelopes, bounded worker configuration,
explicit partial-cache effects, pre-opened per-owner SQLite connections,
fail-closed worker loss, exact health/status bodies, and the authorized additive
modeled-provenance field. The reviewer confirmed that the resulting ticket is
bounded and does not introduce a general scheduler or durable job system.

## Implementation Evidence

Developer: Codex sub-agent `/root/ticket041_implementation`

Implemented one foreground `pangopup serve` adapter in `pangopup-cli`. It opens
the coherent installed profile and every SQLite/model worker before bind,
keeps authoritative lookup and completed cache hits outside one fixed bounded
FIFO, returns ordinary ordered HTTP responses, and implements stable health,
status, backpressure, worker-loss, disconnect, and two-signal drain behavior.
No scheduler framework, durable jobs, polling, automatic sync, in-memory LRU,
or in-flight registry was added.

The model kernel now retains its actual CPU policy. Installed held models can
open under the service-selected sequential thread count; modeled provenance
and every SQLite key record that effective policy. Ordinary CLI fixtures prove
their unchanged `sequential:1/1` policy. ADR 0025 records the bounded service
choice.

Inside-out service tests use fake lookup/cache/model providers and finish
without production assets. They cover exact HTTP bytes and status order,
closed input and size limits, model-only ordering, lookup and cache bypass
under saturation, worker/FIFO/queue counts, immediate 429, both disconnect
states, all-or-error model failure, worker panic fan-out/readiness, and drain
admission. The test-only implementation is split from the production service
module for readability.

Developer verification:

- `cargo test --locked -p pangopup-cli service::tests`: 11 passed.
- `cargo test --locked -p pangopup-cli`: 38 passed.
- `cargo test --locked -p pangopup-cache service_sequential_thread_policies_are_exactly_bounded`: 1 passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `make test`: passed, including executable-delivery and production-release
  qualification scripts.
- `make spec`: 246 passed, 6 skipped.
- `git diff --check`: passed.

One assumption was refined during implementation: serializing routed results
through a generic JSON value would reorder object keys. HTTP therefore embeds
the already-validated raw CLI JSON object inside its response envelope, keeping
the promised CLI field order without a second result serializer.

## Adversarial Code Review

Reviewer: Codex sub-agent `/root/ticket041_code_review`

Final verdict: accepted after remediation and two independent re-reviews.

The first review rejected the implementation on nine concrete grounds:
admission counted only channel slots rather than workers plus waiting slots;
production SQLite/worker behavior lacked direct tests; listener and signal
lifecycle coverage was too synthetic; status counters could be sampled across
separate transitions; `HEAD` inherited Axum's implicit `GET`; worker-loss
fan-out was under-tested; worker-owned sender clones prevented explicit pool
closure; full-byte and boundary fixtures were incomplete; and one superseded
renderer remained.

All findings were remediated without adding a scheduler. One mutex now owns
readiness, running, queued, and the sole optional sender. Admission accepts
exactly `workers + queue_capacity` jobs, queued-to-running is one coherent
transition, status reads one snapshot, and workers retain no sender. Shutdown
closes the sender and joins every worker. The persistent signal collector makes
a second signal observable throughout drain.

Production-path tests now exercise a real `ProductionWorker` and SQLite for
queued recheck, CPU-policy key separation, saturation bypass, absence of a
cache transaction during inference, running-disconnect write-through, and
write-failure result validity. Panic coverage includes one running and two
queued callers, permanent new-admission refusal, shutdown observation, and
ordinary worker join. A real TCP listener pins health bytes and method
behavior. Known-route `HEAD` is 405 with exact `Allow` (`GET` or `POST`), JSON
representation headers, and the empty wire body required by HTTP; unknown
`HEAD` remains 404. The original design reviewer approved that clarified HEAD
contract.

Post-remediation verification:

- focused service tests: 21 passed;
- executable pre-bind lifecycle test: 1 passed;
- `make test`: passed, including executable delivery and production release
  qualification;
- `make spec`: 246 passed, 6 skipped;
- workspace Clippy with `-D warnings`: passed;
- `git diff --check`: passed.

The full gate caught and prevented an early test-only asset feature from
changing the pinned production source identity. The final test support is an
explicit nondefault feature, absent from normal production builds, and uses
the real atomic installer and actual child executable rather than inventing a
second HTTP runner. The model-oracle checksum was updated only for the
authorized addition of `effective_cpu_policy` to the exact M09 provenance
fixture.

### Final re-review remediation

Each accepted model job now carries its admission class (`running` reservation
or `queued` waiting slot). One locked transition reserves an available worker
slot before waiting capacity, so a burst admits exactly `workers +
queue_capacity`, `running <= workers`, and `queued <= queue_capacity` even
before a worker receives from the channel. Cancellation, completion, worker
loss, and shutdown release the same per-job reservation rather than inferring
it from timing.

Unix SIGINT and SIGTERM streams are registered once and remain live throughout
drain. A process-level test sends two real, different signals while work is
running and observes the forced-exit path. Feature-gated child-process tests
run the actual executable over real TCP with atomically installed miniature
assets, probe all four routes, prove accepted work drains on SIGTERM, prove a
second signal forces exit, and prove an incompatible installed profile fails
before the listening line.

Final verification:

- focused service unit tests: 20 passed;
- feature-gated actual-executable lifecycle tests: 4 passed, 1 retained-
  production qualification ignored;
- retained-production qualification: passed once against the existing
  qualified installed profile; the actual executable served all four routes
  and returned one ordered batch as known regression SNV `precomputed` then
  M09 non-SNV `model`;
- workspace Clippy with all targets, all features, and `-D warnings`: passed.
- `make lint`: passed (dependency-policy duplicate notices remain warnings);
- `make test`: passed, including executable-delivery and production-release
  qualification;
- `make spec`: 246 passed, 6 skipped;
- `git diff --check`: passed.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: Codex `/root`

- Inspected the final diff and confirmed the default production binary contains
  no miniature-profile or artificial-delay trust path; those controls require
  the explicit nondefault test-fixture feature.
- Confirmed no local asset paths, request bodies, environment dumps, or secrets
  entered product code, retained user documentation, or executable specs.
- `make lint` passed. Existing non-failing dependency duplicate and
  `zstd-sys` semver-metadata notices remain warnings.
- `make test` passed, including the service unit tests, executable delivery,
  and production-release qualification. Retained multi-gigabyte tests remain
  explicitly ignored in the normal gate.
- `make spec` passed: 246 scenarios passed and 6 were skipped.
- `git diff --check` passed. The only model oracle change is the reviewed
  additive `effective_cpu_policy` provenance field, with ordinary CLI output
  fixed at `sequential:1/1`.
