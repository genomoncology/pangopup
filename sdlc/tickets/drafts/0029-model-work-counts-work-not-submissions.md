---
flow: build
priority: 6
---
# Model work counts work, not submissions

Admission and the uncached-model request limit both count submitted items. `ModelJob::weight` returns `items.len()`, and the request check compares the length of the miss list against the reported limit. Neither collapses items that share one cache key.

Eleven copies of one uncached variant are refused with `MODEL_BATCH_TOO_LARGE` for requiring more than ten uncached model variants. Ten copies of that same variant are accepted and normally complete after one inference because a successfully written result lets later worker items hit the cache. That sharing depends on the cache write and does not cover model rejections. The service therefore refuses a request whose successful path normally costs one inference, and it reserves ten of twenty queue units for it.

The consequence reaches every consumer. A batch client must deduplicate identical strings before it calls, purely to avoid spending capacity it does not use. That is service accounting pushed into the caller.

Count distinct model work. Group request-local miss items that share one canonical cache key before the request limit compares against the reported ceiling, and reserve one admission unit for each distinct key rather than one for each submitted item. Execute each request-local key once without depending on a cache write, then expand its completed result to every occurrence. Every submitted item still receives its own ordered result carrying its own `input`, because the response contract is unchanged.

Done, observably:

- A request holding many copies of one uncached variant counts one unit against the reported uncached-model limit.
- That request is accepted when its distinct uncached work fits the limit, however many copies it carries within the reported `max_items` envelope.
- Different submitted strings that normalize to one cache key count as one admission unit, execute once, and return separate ordered items preserving each original `input`.
- Admission reserves one unit per distinct uncached cache key, and `running` plus `queued` reflect distinct work.
- A request-local successful result and a request-local model rejection each expand to every matching submitted item with the existing ordered item shape and exact `input`.
- An operational worker failure remains one request-level failure with no partial item array.
- A request of distinct uncached variants above the reported limit is still refused exactly as it is today.
- `Retry-After` still derives from admitted units through the unchanged formula.
- Two concurrent requests for the same uncached key each reserve their own unit and do not coalesce work across requests.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change the reported limits, the default queue capacity, the `Retry-After` formula, HTTP statuses, error codes, error messages, response order, item shape, scoring, routing, or cache contents. Do not deduplicate across requests, coalesce in-flight work between requests, or add configuration.
