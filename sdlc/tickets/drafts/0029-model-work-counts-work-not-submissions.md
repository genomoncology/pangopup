---
flow: build
priority: 6
---
# Model work counts work, not submissions

Admission and the uncached-model request limit both count submitted items. `ModelJob::weight` returns `items.len()`, and the request check compares the length of the miss list against the reported limit. Neither collapses items that share one cache key.

Eleven copies of one uncached variant are refused with `MODEL_BATCH_TOO_LARGE` for requiring more than ten uncached model variants. Ten copies of that same variant are accepted and complete in about the time of a single inference, because the work is shared once admitted. The service therefore refuses a request whose real cost is one inference, and it reserves ten of twenty queue units to perform one.

The consequence reaches every consumer. A batch client must deduplicate identical strings before it calls, purely to avoid spending capacity it does not use. That is service accounting pushed into the caller.

Count distinct model work. Collapse miss items that share one cache key before the request limit compares against the reported ceiling, and reserve one admission unit for each distinct key rather than one for each submitted item. Every submitted item still receives its own ordered result carrying its own `input`, because the response contract is unchanged.

Done, observably:

- A request holding many copies of one uncached variant counts one unit against the reported uncached-model limit.
- That request is accepted when its distinct uncached work fits the limit, however many copies it carries.
- Admission reserves one unit per distinct uncached cache key, and `running` plus `queued` reflect distinct work.
- Every submitted item still returns its own ordered result with its own `input`, including duplicates.
- A request of distinct uncached variants above the reported limit is still refused exactly as it is today.
- `Retry-After` still derives from admitted units through the unchanged formula.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change the reported limits, the default queue capacity, the `Retry-After` formula, HTTP statuses, error codes, error messages, response order, item shape, scoring, routing, or cache contents. Do not deduplicate across requests, coalesce in-flight work between requests, or add configuration.
