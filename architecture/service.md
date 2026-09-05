# Service Boundary

Pangopup ships a foreground HTTP server, signal lifecycle, and thin non-root
Docker image through one native AMD64/ARM64 GHCR index. The public index
currently identifies application v0.3.0; the repository prepares v0.4.0 as one
coherent executable/container candidate without changing service behavior or
assets. It does not yet ship a systemd example or metrics. The CLI interface remains
`pangopup lookup`; its typed
lookup-first/model route already returns stable JSON Lines or exact
tab-separated output from an activated installed profile or complete explicit
override and persists successful model results in a bounded SQLite cache.

## One lookup-first core

The CLI and HTTP adapter call the same typed routing API:

```text
validated GRCh38 variant
  -> covered SNV index hit: exact precomputed gene result(s)
  -> no hit or supported non-SNV: pinned model inference
  -> unsupported or failed route: stable typed error
```

Callers may explicitly request the parallel typed model-only route. That route
bypasses the SNV provider for the whole batch and otherwise reuses the same
model scoring, filtering, cache, result, and provenance contracts. The HTTP
request exposes the equivalent optional `model_only` boolean; it
does not need a general scoring-mode grammar or a comparison envelope.

Every result identifies its route and the exact lookup bundle or model,
compiled GRCh38 sequence index, mask, and inference parameters involved. A
precomputed SNV hit is authoritative and must not be recomputed merely because
the model is available. Adapters own transport parsing and rendering, not
scoring, masking, index layout, or model-runtime types.

## Active scoring identity

The HTTP service computes one `pangopup.active-scoring-identity.v1` value during startup. The RFC 8785 canonical JSON preimage contains the running software version, admitted runtime-profile identity, and effective CPU policy. These inputs cover every installed component and active policy that can change an answer. The status route and every returned score item expose the same full SHA-256 value.

Worker count, queue capacity and state, cache configuration and contents, listener address, paths, process and host facts, and request fields do not enter the preimage. Existing route provenance remains authoritative for the component-level audit trail. The concise identity does not enter `RoutedResult`, route provenance, model cache identity, cache keys, or the cache schema. Standalone CLI output remains unchanged because that command can run without a complete service profile.

## Measured model partition boundary

ADR 0024 retains one host-qualified partition table for the AMD Ryzen 7 5825U
under exact non-SMT affinities: `1×1`, `1×2`, `1×4`, and `2×4` for physical-CPU
budgets 1, 2, 4, and 8. The portable ordinary policy remains `1×1`; the service
must not extrapolate the table from logical CPU count, cgroup quota, or an
unmatched CPU identity.

The measurement also proves that the separately opened SNV mmap stays fast while a model batch is in flight. The service keeps lookup and completed SQLite hits outside model admission. It uses fixed workers, whole-request FIFO dispatch, immediate 429 backpressure, one handler SQLite connection, and one SQLite connection per worker. It deliberately does not coalesce in-flight work.

`--model-queue-capacity` bounds running plus queued uncached model variants. The default is 20 units. The request contract reports the smaller of this capacity and the built-in ceiling of ten as `max_uncached_model_items`. A request above the reported limit receives HTTP 422 with `MODEL_BATCH_TOO_LARGE` before admission. Admission reserves every remaining miss in a request that fits the reported limit or refuses the whole request temporarily. Invalid item values consume no model admission units. The public status fields `running`, `queued`, and `queue_capacity` use this unit and identify it as `uncached_model_variant`. The dispatcher separately counts running jobs to assign fixed workers. That internal count does not change the public work totals.

The slowest retained p50 for the portable `sequential:1/1` policy is 10.241 seconds per variant in `planning/artifacts/022-reference-alternate-batching.md`. Twenty units therefore give one sequential worker a planning estimate of about 205 seconds. Variant costs differ by more than a factor of two, so this estimate does not guarantee retirement time. Operators adjust the capacity for their workload.

A 429 refusal captures `running + queued` under the admission lock. Every request that reaches admission can fit on an empty service, so the response carries one decimal `Retry-After` delay of `ceil((running + queued) × 10.241)` seconds with a one-second minimum. The calculation rounds upward and does not divide by worker count because retained measurements do not prove linear worker scaling. The delay does not guarantee that capacity will exist. Other errors receive no queue retry guidance. The status schema does not expose this snapshot.

## Foreground lifecycle

ADR 0025 records the fixed-worker, bounded-FIFO, and drain choices below.

The executable exposes `pangopup serve` as one foreground process. It
does not fork, daemonize, write PID files, or implement its own
start/stop/restart supervisor. Docker, systemd, Kubernetes, or another external
process manager owns those lifecycle actions and restart policy.

The service exposes:

- liveness that says the process event loop is responsive;
- readiness that becomes successful only after the pinned asset profile opens
  and required providers initialize;
- `pangopup status` plus a status endpoint that report software version,
  installed asset identities, the active scoring identity, enabled routes, readiness, non-secret
  configuration, and the active public request contract; and
- graceful shutdown on ordinary process-manager signals.

Readiness consumes the established canonical four-asset runtime profile
and reject a mixed tuple. Offline installation, activation, CLI discovery, and
held provider opening are established. Service startup performs no sync or
network operation; operators run `pangopup sync` explicitly. A running process
holds one immutable opened profile; an upgrade is a new process, not an
in-place mmap/model swap.

## HTTP contract

The first HTTP slice is versioned batch JSON over explicit GRCh38 variants, stable typed errors, health/readiness/status endpoints, 64-KiB bodies, 100-variant batches, a 10-uncached-model-variant limit, backpressure, and deterministic ordering. Each `variants[]` value accepts the literal grammar or the strict exact-edit insertion and deletion grammar documented by the CLI. The service converts exact edits before routing and admission. A valid request envelope returns HTTP 200 with one ordered outcome per submitted string. Every item carries that exact string under `input` before its existing fields. Invalid syntax, unsupported genomic values, exact-edit geometry failures, out-of-bounds edit windows, and unusable anchors become `INVALID_VARIANT` item outcomes. A deleted-sequence mismatch with a valid anchor and a model rejection keep their normalized fields and become `MODEL_REJECTED` item outcomes. This rule also covers singleton and all-rejected batches. Reference corruption remains a server failure. Invalid JSON, non-string array members, invalid shared options, and limit failures remain request-level client errors. Scoring, cache, worker, readiness, and saturation failures remain request-level operational errors. The HTTP slice adds no transcript HGVS, projection, clinical interpretation, remote calls, job IDs, polling, or server-side model timeout.

`/v1/status.request_contract` serializes this enforced request boundary for clients. One service definition owns the body, variant-count, and uncached-model-work limits. The scoring handler and status output consume it. Engine-owned constants likewise bind model allele and exact-edit sequence reporting to validation. One adapter contig-spelling descriptor serves both parsing and status generation. The fixed contract does not vary with readiness, queue occupancy, assets, paths, listener, host, or request data. It does not enter the active scoring identity because it does not change a score.

The executable CLI's JSONL contract is already shipped and remains useful for
process-boundary integration and testing. HTTP defines a separate JSON request
and response envelope while reusing the same core result fields and provenance.

## Container deployment

The shipped minimal container:

- runs the foreground service as a non-root user;
- use a read-only runtime filesystem and no package manager/toolchain;
- keeps installed assets in explicit named volumes and runs `sync` only when
  the operator requests it;
- exposes the readiness/liveness endpoints as its health contract;
- preserve GPL and source-dataset notices; and
- retains the existing bounded request and graceful shutdown behavior.

The Dockerfile deliberately adds no Compose, restart policy, shell-based
healthcheck, TLS, or orchestration policy. The public registry workflow changes
delivery, not runtime behavior: its index contains exactly the two native thin
leaves and no scoring assets. Each leaf is checked natively with miniature
assets. A separate manual read-only workflow checks all 14 retained production
cases through each architecture's final stripped image; ordinary hosted
publication runners do not download the 15 GB production SNV installation, so
the retained service specs and full-volume Apple run remain the HTTP evidence.

A native systemd example may invoke the same foreground command and point at
the same installed profile. Pangopup-specific `start`, `stop`, and `restart`
commands are deliberately unnecessary.

## Persistent model-result cache

The mmap SNV path continues to rely only on the operating-system page cache.
Successful complete model results use the persistent SQLite cache selected in
ADR 0019. Stable-gene filtering happens after cache retrieval, maximizing reuse
without changing masking. The default is 10,000 entries with deterministic
insertion/update-order eviction; valid hits are read-only and `unlimited` is
explicit.

The HTTP adapter reuses this database and key/value contract. Workers recheck
SQLite before each inference; there is no separate in-memory cache or
in-flight-fill registry.

## Operational proof

Before the service is called production-ready, retained evidence must cover
startup, warm and defensible cold behavior, concurrency, throughput,
p50/p95/p99 latency, resident memory, page faults, inference resources,
graceful shutdown, backpressure, and corrupt/missing asset failure. Later
hardening includes structured logs, useful metrics, resource limits, read-only
runtime posture, dependency/license inventory, SBOM, release provenance,
signing where practical, and upgrade/rollback exercises.
