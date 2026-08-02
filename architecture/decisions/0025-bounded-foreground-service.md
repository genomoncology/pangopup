# ADR 0025: Bounded foreground HTTP service

Status: accepted

## Decision

Pangopup serves ordinary synchronous HTTP responses from one foreground
process. Authoritative mmap hits and completed SQLite hits bypass model
admission. Each request with remaining model work becomes one job in a bounded
FIFO waiting line consumed by fixed, single-owner model workers. A full line is
rejected immediately with HTTP 429.

The portable defaults are one worker, one ONNX intra-op thread, and 16 waiting
requests. Explicit worker and thread counts do not change biological scoring,
but the effective thread policy is part of result provenance and SQLite cache
identity. Pangopup does not infer Ticket 040's host-qualified mappings.

Queued work whose HTTP receiver has closed is discarded. Started inference is
allowed to finish and may populate SQLite. SIGINT and SIGTERM stop score
admission and drain accepted work; a second signal forces exit. Unexpected
worker loss fails readiness, refuses new model admission, and completes waiting
callers with a stable unavailable error.

## Consequences

The async HTTP event loop remains responsive during blocking inference without
introducing durable jobs, polling, priorities, dynamic resizing, distributed
queues, an in-memory LRU, or in-flight coalescing. Clients may use blocking or
async HTTP calls. Deployment supervisors own start, stop, and restart.
