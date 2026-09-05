---
flow: build
priority: 5
deps: ["0006"]
---
# A saturated response tells the caller when to retry

The HTTP service returns 429 `MODEL_QUEUE_FULL` when it refuses model work, but the response gives the caller no retry guidance. Generic HTTP clients either retry immediately or invent their own delay. Immediate retries add work at the exact moment the model queue cannot accept it.

Ticket 0006 makes saturation visible before callers reach their own timeout and expresses admission in model-work units. A full-queue response must use that same observed work and the documented slowest-retained-p50 planning estimate to tell the caller when another attempt could be useful. The guidance may be early or late for a particular workload. It does not guarantee that capacity will exist when the caller retries.

The response carries exactly one `Retry-After` field in the positive decimal `delay-seconds` form defined by HTTP when the rejected request could fit on an empty service. The delay is `ceil((running + queued) × 10.241)` seconds with a minimum of one second. The dispatcher captures `running + queued` under the same lock that refuses the request. The calculation uses the admitted work before the rejected request and rounds upward to a whole second. The response body keeps the typed `MODEL_QUEUE_FULL` error byte for byte. Other client and server errors do not claim that waiting will fix them.

A request whose own uncached-model weight exceeds the configured capacity still receives the existing 429 status and exact `MODEL_QUEUE_FULL` body, but it receives no `Retry-After` field. No delay can make that request fit without a configuration change. Omitting the field prevents a standards-aware client from retrying a permanent configuration mismatch forever. The request may become admissible after an operator raises the capacity.

The formula does not divide by the configured worker count. The retained measurement does not prove linear scaling across workers. This choice can advise a later retry on a multi-worker service. A worker-aware estimate could advise an unjustifiably early retry. Retained multi-worker measurements are the lever for changing this decision later.

The status response does not change. Clients do not derive retry guidance from status. The service supplies it on `MODEL_QUEUE_FULL` because that response carries the admission snapshot used by the calculation.

This changes a public contract consumed by HTTP clients. A downstream annotation service can retry queue saturation according to the response while leaving rejected variants and operational failures under their separate policies.

Done, observably:

- Every transient 429 `MODEL_QUEUE_FULL` response carries exactly one positive decimal `Retry-After` delay in seconds. A request that exceeds the configured capacity receives no retry guidance even when the service is idle.
- The guidance equals `ceil((running + queued) × 10.241)` at refusal time and has a minimum of one second.
- A more heavily loaded service never advises an earlier retry than the same service with less admitted work under the same configuration.
- The same admitted-unit state produces the same guidance with one worker or multiple workers.
- An idle service configured below a request's weight returns the unchanged 429 body without `Retry-After`.
- A model rejection, invalid request, missing route, and service failure do not carry queue retry guidance.
- The status schema remains unchanged. Operator documentation names the formula, its slowest-retained-p50 source, the upward rounding, the worker-count decision, and the lack of a capacity guarantee.
- The suite pins the header and its relationship to observed admitted work without depending on wall-clock sleeps.

Boundary: do not add an automatic server retry, client library, request deadline, asynchronous job API, or guaranteed completion time. Do not change the 429 status, error code, admission decision, model-work limit, or service scheduling established by ticket 0006.
