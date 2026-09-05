---
flow: build
priority: 5
---
# The reported contract cannot promise work the service will refuse

`/v1/status` reports `request_contract.variants.max_uncached_model_items` as the built-in ceiling of ten. `serve --model-queue-capacity` accepts one through 1024 and is never compared against that ceiling. A service configured below ten publishes a request size its own admission control can never accept.

An idle service started with `--model-queue-capacity 5` reports `max_uncached_model_items: 10` and then refuses a six-variant uncached request with HTTP 429 and `MODEL_QUEUE_FULL`. The refusal carries no `Retry-After`, correctly, because no amount of waiting admits that weight. The caller built a request the published contract allowed, received a status reserved for temporary conditions, and has no reported number that would have prevented it.

Ticket 0012 made the reported contract the caller's source of truth. Ticket 0013 made a refusal say whether waiting resolves it. This gap undoes both for any deployment below the ceiling, and it pushes the compensation back into every consumer as a minimum across two reported numbers.

Report `max_uncached_model_items` as the smaller of the built-in ceiling and the configured model queue capacity. Enforce that same reported number where the request currently checks the built-in ceiling, so an over-ceiling request is refused as `MODEL_BATCH_TOO_LARGE` with HTTP 422 before admission rather than as `MODEL_QUEUE_FULL` with HTTP 429 after it. A permanent refusal then arrives with a permanent status and the number the caller should have used.

Leave the admission guard that omits `Retry-After` in place. The request check now keeps admitted weight within capacity, so `/v1/score` no longer reaches that branch, and the guard stays correct for any future admission path.

Done, observably:

- `/v1/status` reports `max_uncached_model_items` equal to the smaller of the built-in ceiling and the configured model queue capacity.
- A service below the ceiling refuses an over-ceiling uncached request with HTTP 422 and `MODEL_BATCH_TOO_LARGE` while idle, and its refusal message states the number that the same executable reports.
- A service at or above the ceiling reports ten and behaves exactly as it does today.
- Temporary saturation still returns HTTP 429 with an integer `Retry-After` computed from admitted work.
- Behavioral tests cover a below-ceiling capacity and the default capacity through the HTTP surface, including the reported number, the refusal status, and the refusal text.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change the default queue capacity, the built-in ten-variant ceiling, the item or body limits, the `Retry-After` formula, error codes, HTTP statuses, response shapes, accepted gene or variant forms, admission accounting, scoring, routing, or caching. Do not change `pangopup-core`. Do not add configuration options or status fields.
