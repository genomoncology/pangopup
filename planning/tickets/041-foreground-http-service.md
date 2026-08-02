# 041 — Foreground HTTP scoring service

Status: ready

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
- Every HTTP body is canonical compact JSON followed by one newline and uses
  `Content-Type: application/json`. Errors use the closed envelope
  `{"error":{"code":"<STABLE_CODE>","message":"<BOUNDED_MESSAGE>"}}`.
  The status/code contract is: malformed/unknown/duplicate/invalid input is
  `400 INVALID_REQUEST`; body overflow is `413 REQUEST_TOO_LARGE`; too many
  uncached model variants is `422 MODEL_BATCH_TOO_LARGE`; saturated model
  admission is `429 MODEL_QUEUE_FULL`; shutdown admission is
  `503 SHUTTING_DOWN`; model/operational scoring failure is
  `500 SCORING_FAILED`; an unknown route is `404 NOT_FOUND`; and a known route
  with the wrong method is `405 METHOD_NOT_ALLOWED`. Health/readiness/status
  responses use the same JSON and newline rules.
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
- Count capacity as running jobs plus waiting jobs. A worker removes one job
  from the waiting count when it starts. When every worker is occupied and the
  waiting line is full, a new request requiring uncached model work receives a
  stable `429 Too Many Requests` JSON error immediately. SNV hits and cache hits
  still succeed while that line is full. No request priority or dynamic worker
  resizing exists.
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

- `pangopup serve --listen 127.0.0.1:0` starts against injected miniature
  installed assets for tests, reports its chosen address without secrets, and
  serves the four exact routes. Production startup uses only the activated
  installed profile.
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
- Lifecycle tests prove startup fails before bind on missing/incompatible
  assets, readiness transitions, new admission stops during shutdown, accepted
  work drains, and SIGINT/SIGTERM use the same graceful path.
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

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
