# Service Boundary

This document records target service design. Pangopup does not yet ship an HTTP
server, service lifecycle integration, container, or metrics. The shipped
runtime interface is `pangopup lookup`; its typed
lookup-first/model route already returns stable JSON Lines or exact
tab-separated output from an activated installed profile or complete explicit
override and persists successful model results in a bounded SQLite cache.

## One lookup-first core

The current CLI and future HTTP adapter call the same typed routing API:

```text
validated GRCh38 variant
  -> covered SNV index hit: exact precomputed gene result(s)
  -> no hit or supported non-SNV: pinned model inference
  -> unsupported or failed route: stable typed error
```

Callers may explicitly request the parallel typed model-only route. That route
bypasses the SNV provider for the whole batch and otherwise reuses the same
model scoring, filtering, cache, result, and provenance contracts. The future
HTTP request should expose the equivalent optional `model_only` boolean; it
does not need a general scoring-mode grammar or a comparison envelope.

Every result identifies its route and the exact lookup bundle or model,
compiled GRCh38 sequence index, mask, and inference parameters involved. A
precomputed SNV hit is authoritative and must not be recomputed merely because
the model is available. Adapters own transport parsing and rendering, not
scoring, masking, index layout, or model-runtime types.

## Measured model partition boundary

ADR 0024 retains one host-qualified partition table for the AMD Ryzen 7 5825U
under exact non-SMT affinities: `1×1`, `1×2`, `1×4`, and `2×4` for physical-CPU
budgets 1, 2, 4, and 8. The portable ordinary policy remains `1×1`; the service
must not extrapolate the table from logical CPU count, cgroup quota, or an
unmatched CPU identity.

The measurement also proves that the separately opened SNV mmap stays fast
while a model batch is in flight. The service must therefore keep lookup
outside model queue admission. Ticket 040 deliberately did not choose queue
capacity, backpressure, dispatch, SQLite connection ownership, concurrent-fill
coalescing, or failure fan-out; the HTTP implementation owns and tests those
policies.

## Foreground lifecycle

The planned executable exposes `pangopup serve` as one foreground process. It
does not fork, daemonize, write PID files, or implement its own
start/stop/restart supervisor. Docker, systemd, Kubernetes, or another external
process manager owns those lifecycle actions and restart policy.

The service exposes:

- liveness that says the process event loop is responsive;
- readiness that becomes successful only after the pinned asset profile opens
  and required providers initialize;
- `pangopup status` plus a status endpoint that report software version,
  installed asset identities, enabled routes, readiness, and non-secret
  configuration; and
- graceful shutdown on ordinary process-manager signals.

Readiness will consume the established canonical four-asset runtime profile
and reject a mixed tuple. Offline installation, activation, CLI discovery, and
held provider opening exist; service lifecycle composition remains future.
Service startup may invoke the same pinned asset-sync operation exposed
explicitly as `pangopup sync`. Offline mode forbids networking and names
missing or incompatible assets. A running process holds one immutable opened
profile; an upgrade is a new process, not an in-place mmap/model swap.

## HTTP contract direction

The first HTTP slice should be small: versioned batch JSON over explicit GRCh38
variants, stable typed errors, health/readiness/status endpoints, request/body
and batch limits, timeouts, backpressure, and deterministic ordering. It should
not add transcript HGVS, projection, clinical interpretation, or remote calls
to other genomic services.

The executable CLI's JSONL contract is already shipped and remains useful for
process-boundary integration and testing. HTTP defines a separate JSON request
and response envelope while reusing the same core result fields and provenance.

## Deployment direction

A future minimal container should:

- run the foreground service as a non-root user;
- use a read-only runtime filesystem and no package manager/toolchain;
- accept a verified asset profile through an immutable image layer or read-only
  mount;
- expose the readiness/liveness endpoints as its health contract;
- preserve GPL and source-dataset notices; and
- have bounded CPU, memory, request, timeout, and shutdown behavior tested.

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

The future HTTP adapter reuses this database and key/value contract. Concurrent
fill coalescing belongs to that service's bounded worker/queue design and is
not hidden in the sequential CLI or scoring engine.

## Operational proof

Before the service is called production-ready, retained evidence must cover
startup, warm and defensible cold behavior, concurrency, throughput,
p50/p95/p99 latency, resident memory, page faults, inference resources,
graceful shutdown, backpressure, and corrupt/missing asset failure. Later
hardening includes structured logs, useful metrics, resource limits, read-only
runtime posture, dependency/license inventory, SBOM, release provenance,
signing where practical, and upgrade/rollback exercises.
