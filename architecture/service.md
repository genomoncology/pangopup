# Service Boundary

This document records target service design. Pangopup does not yet ship an HTTP
server, model routing, service lifecycle integration, container, metrics, or
model-result cache. The shipped runtime interface is `pangopup lookup`, which
already returns stable JSON Lines or exact tab-separated output.

## One lookup-first core

The future CLI and HTTP adapters call the same typed routing API:

```text
validated GRCh38 variant
  -> covered SNV index hit: exact precomputed gene result(s)
  -> no hit or supported non-SNV: pinned model inference
  -> unsupported or failed route: stable typed error
```

Every result identifies its route and the exact lookup bundle or model,
reference, mask, and inference parameters involved. A precomputed SNV hit is
authoritative and must not be recomputed merely because the model is available.
Adapters own transport parsing and rendering, not scoring, masking, index
layout, or model-runtime types.

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

Service startup may invoke the same future pinned asset-sync operation exposed
explicitly as `pangopup assets sync`. Offline mode forbids networking and names
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

## Cache decision gate

The mmap lookup path relies on the operating-system page cache. A persistent
application cache is considered only for model inference after representative
end-to-end measurements show meaningful repeated work. If adopted, its key
must include the normalized variant and every scoring identity: gene/masking
context, checkpoint, reference, mask, window, and inference parameters.

Any cache slice must bound size, define eviction, serialize concurrent fills,
recover from corruption, prove identity-based invalidation, and show latency or
compute benefit net of lookup and serialization overhead. SQLite or another
store is an implementation candidate, not an architectural requirement.

## Operational proof

Before the service is called production-ready, retained evidence must cover
startup, warm and defensible cold behavior, concurrency, throughput,
p50/p95/p99 latency, resident memory, page faults, inference resources,
graceful shutdown, backpressure, and corrupt/missing asset failure. Later
hardening includes structured logs, useful metrics, resource limits, read-only
runtime posture, dependency/license inventory, SBOM, release provenance,
signing where practical, and upgrade/rollback exercises.
