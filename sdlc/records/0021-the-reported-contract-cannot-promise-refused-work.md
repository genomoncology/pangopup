---
base: 73c1aa2bcc06af085c3448040263913ee51b1b34
head: ab14de34b853f7447924e58e0a4b01a0262abe9a
---

# The reported contract cannot promise refused work

PangoPup now reports the largest uncached-model request that the configured service can admit while idle. The effective limit is the smaller of the built-in ceiling of ten and `--model-queue-capacity`. The same effective value drives `/v1/status`, request validation, and refusal text.

A request above the reported limit now receives HTTP 422 with `MODEL_BATCH_TOO_LARGE` before admission. HTTP 429 with `MODEL_QUEUE_FULL` remains reserved for temporary saturation and retains its integer `Retry-After` guidance. Cache hits remain outside uncached-model accounting.

The implementation added HTTP tests for below-ceiling and default capacities. Independent code review also checked capacities 1, 5, 10, 20, and 1024, cache-hit handling, refusal ordering, temporary saturation, documentation, and change boundaries. The reviewer accepted the candidate without findings.

`make lint`, `make test`, and `make spec` passed on Linux. The specification suite reported 282 passed and 7 skipped.
