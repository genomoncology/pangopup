---
flow: build
priority: 5
---
# The reported contract cannot promise work the service will refuse

`/v1/status` reports `request_contract.variants.max_uncached_model_items` as the built-in ceiling of ten. `serve --model-queue-capacity` accepts one through 1024 and is never compared against that ceiling. A service configured below ten therefore publishes a request size its own admission control can never accept.

An idle service started with `--model-queue-capacity 5` reports `max_uncached_model_items: 10` and then refuses a six-variant uncached request with HTTP 429 and `MODEL_QUEUE_FULL`. The refusal carries no `Retry-After` because no amount of waiting admits that weight. Status also reports `model.queue_capacity: 5`, so a caller can compensate by combining two fields. Ticket 0012 established `request_contract` as the machine-readable source for valid request limits. The contract alone therefore promises a request that the idle service refuses.

Ticket 0013 deliberately used HTTP 429 without `Retry-After` when a request exceeded configured capacity. That behavior predates ticket 0012's reported request contract. The combined behavior now leaves temporary saturation and a permanent request-limit failure under the same 429 code even though the contract promised the request was valid. This ticket deliberately replaces that permanent-capacity response.

The request contract must report the largest uncached-model request that the configured service can admit while idle, without exceeding the built-in ceiling. A request above that reported limit must receive HTTP 422 with `MODEL_BATCH_TOO_LARGE`. HTTP 429 with `MODEL_QUEUE_FULL` remains a temporary saturation response for a request that fits on an idle service.

Update the current user and service documentation to describe this distinction. Keep historical decision records and publication evidence unchanged.

Done, observably:

- `/v1/status` reports `max_uncached_model_items` equal to the smaller of the built-in ceiling and the configured model queue capacity.
- A service below the built-in ceiling refuses an uncached request above the reported limit with HTTP 422 and `MODEL_BATCH_TOO_LARGE` while idle, and its refusal message states the number that the same executable reports.
- A service at or above the ceiling reports ten and behaves exactly as it does today.
- Temporary saturation still returns HTTP 429 with an integer `Retry-After` computed from admitted work.
- Behavioral tests cover a below-ceiling capacity and the default capacity through the HTTP surface, including the reported number, the refusal status, and the refusal text.
- The README and service architecture describe the effective request limit, permanent over-limit rejection, and temporary queue saturation accurately.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: the named correction changes an idle over-limit request from HTTP 429 `MODEL_QUEUE_FULL` to HTTP 422 `MODEL_BATCH_TOO_LARGE`. Do not change any other error code or HTTP status. Do not change the default queue capacity, its accepted configuration range, the built-in ten-variant ceiling, the item or body limits, the `Retry-After` formula, response shapes, accepted gene or variant forms, admission accounting, scoring, routing, or caching. Do not change `pangopup-core`, historical decision records, or publication evidence. Do not add configuration options or status fields.
