# ADR 0025: Bounded foreground HTTP service

Status: accepted

## Decision

Pangopup serves ordinary synchronous HTTP responses from one foreground process. Authoritative mmap hits and completed SQLite hits bypass model admission. Each request with remaining model work becomes one whole job in FIFO order. Fixed, single-owner model workers consume those jobs. Admission bounds the total uncached model variants across running and queued jobs. Work above the bound is rejected immediately with HTTP 429.

The portable defaults are one worker, one ONNX intra-op thread, and 20 admitted uncached-model-variant units. `--model-queue-capacity` retains its name and its 1 through 1024 range. It configures these units. The status fields `running`, `queued`, and `queue_capacity` use the same unit and report `work_unit: "uncached_model_variant"`. Explicit worker and thread counts do not change biological scoring, but the effective thread policy is part of result provenance and SQLite cache identity. Pangopup does not infer Ticket 040's host-qualified mappings.

The default uses the slowest retained p50 for the portable `sequential:1/1` policy. That measurement is 10.241 seconds per variant in `planning/artifacts/022-reference-alternate-batching.md`. The 20-unit bound gives one worker a planning estimate of about 205 seconds. This estimate does not guarantee latency. A cheaper workload can shed work sooner than necessary. This is the accepted cost of choosing the slowest retained p50 instead of the median. Operators can change the capacity for a different workload.

A rejected request that could fit on an empty service receives one decimal `Retry-After` field. The dispatcher derives the delay from the admitted work captured under the same lock as the refusal. It calculates `ceil((running + queued) × 10.241)` seconds with a one-second minimum. The calculation does not divide by worker count because the retained evidence does not prove linear scaling. The delay may be early or late and does not reserve future capacity. A request whose own weight exceeds capacity receives no retry guidance. Other errors do not receive queue retry guidance. This behavior does not change the status schema.

Queued work whose HTTP receiver has closed is discarded. Started inference is
allowed to finish and may populate SQLite. SIGINT and SIGTERM stop score
admission and drain accepted work; a second signal forces exit. Unexpected
worker loss fails readiness, refuses new model admission, and completes waiting
callers with a stable unavailable error.

## Consequences

The async HTTP event loop remains responsive during blocking inference without introducing durable jobs, polling, priorities, dynamic resizing, distributed queues, an in-memory LRU, or in-flight coalescing. Admission reserves every remaining miss in a request or refuses the whole request. Grouping the same model work across requests does not change the total capacity it consumes. A configured capacity below ten can refuse one valid maximum-size model request. Clients may use blocking or async HTTP calls. Deployment supervisors own start, stop, and restart.
