# Service Boundary

Pangopup ships a foreground HTTP server, signal lifecycle, and thin non-root
Docker image through one native AMD64/ARM64 GHCR index. The public index
currently identifies application v0.2.0; the repository prepares v0.3.0 as one
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

## Measured model partition boundary

ADR 0024 retains one host-qualified partition table for the AMD Ryzen 7 5825U
under exact non-SMT affinities: `1×1`, `1×2`, `1×4`, and `2×4` for physical-CPU
budgets 1, 2, 4, and 8. The portable ordinary policy remains `1×1`; the service
must not extrapolate the table from logical CPU count, cgroup quota, or an
unmatched CPU identity.

The measurement also proves that the separately opened SNV mmap stays fast
while a model batch is in flight. The service keeps lookup outside model queue
admission. It uses fixed workers, a bounded FIFO waiting line, immediate 429
backpressure, one handler SQLite connection, and one SQLite connection per
worker. It deliberately does not coalesce in-flight work.

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
  installed asset identities, enabled routes, readiness, and non-secret
  configuration; and
- graceful shutdown on ordinary process-manager signals.

Readiness consumes the established canonical four-asset runtime profile
and reject a mixed tuple. Offline installation, activation, CLI discovery, and
held provider opening are established. Service startup performs no sync or
network operation; operators run `pangopup sync` explicitly. A running process
holds one immutable opened profile; an upgrade is a new process, not an
in-place mmap/model swap.

## HTTP contract

The first HTTP slice is versioned batch JSON over explicit GRCh38 variants, stable typed errors, health/readiness/status endpoints, 64-KiB bodies, 100-variant batches, a 10-uncached-model-variant limit, backpressure, and deterministic ordering. A model rejection belongs to its input item when the request also produces at least one normal outcome and no operational failure occurs. The response keeps that rejection in input order and returns HTTP 200. HTTP 422 applies only when every input outcome is model-rejected. An authoritative or cached normal outcome therefore keeps a mixed response at HTTP 200. Scoring, cache, worker, and readiness failures invalidate the complete request. They never become item absences or rejections. The HTTP slice adds no transcript HGVS, projection, clinical interpretation, remote calls, job IDs, polling, or server-side model timeout.

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
