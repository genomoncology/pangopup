---
flow: build
priority: 5
deps: ["0006"]
---
# A saturated response tells the caller when to retry

The HTTP service returns 429 `MODEL_QUEUE_FULL` when it refuses model work, but the response gives the caller no retry guidance. Generic HTTP clients either retry immediately or invent their own delay. Immediate retries add work at the exact moment the model queue cannot accept it.

Ticket 0006 makes saturation visible before callers reach their own timeout and expresses admission in model-work units. A full-queue response must use that same observed work and the documented conservative retirement estimate to tell the caller when another attempt could be useful. The guidance is an estimate because model variants have different costs. It must prefer a late retry over an immediate retry storm when the exact retirement time is unknown.

The retry guidance must use the standard HTTP response mechanism so ordinary clients can honor it without a PangoPup-specific library. The response body keeps the typed `MODEL_QUEUE_FULL` error. Other client and server errors do not claim that waiting will fix them.

This changes a public contract consumed by HTTP clients. A downstream annotation service can retry queue saturation according to the response while leaving rejected variants and operational failures under their separate policies.

Done, observably:

- Every 429 `MODEL_QUEUE_FULL` response carries valid positive retry guidance through the standard HTTP mechanism.
- The guidance derives from the admitted work and conservative service estimate rather than a bare unrelated constant.
- A more heavily loaded service never advises an earlier retry than the same service with less admitted work under the same configuration.
- A model rejection, invalid request, missing route, and service failure do not carry queue retry guidance.
- The status route and operator documentation use the same unit and assumptions as the saturated response.
- The suite pins the header and its relationship to observed admitted work without depending on wall-clock sleeps.

Boundary: do not add an automatic server retry, client library, request deadline, asynchronous job API, or guaranteed completion time. Do not change the 429 status, error code, admission decision, model-work limit, or service scheduling established by ticket 0006.
