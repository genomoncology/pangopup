---
flow: build
priority: 8
---
# A valid batch reports every item outcome

`/v1/score` already returns HTTP 200 with one ordered result per input when a model-rejected variant has at least one normal neighbor. It changes behavior when every variant is model-rejected: the service discards the item results and returns one request-level HTTP 422 error. It also returns one request-level HTTP 400 when an individual variant string is invalid, so one bad item can still discard valid neighbors before scoring.

A caller that supplied a valid scoring envelope asked the service to classify a batch. The service completed that request when it can report a final outcome for every item. An item that cannot be parsed, normalized, or scored is an item outcome. It is not a failed HTTP operation. This rule applies to a batch of one, a mixed batch, and a batch where every item is rejected.

The scoring route must return HTTP 200 and one ordered outcome for every submitted variant when the JSON envelope, shared request options, body size, item count, and published work limits are valid. Each invalid or model-rejected item must carry a stable rejected status and error that lets a batch client handle that item without discarding its neighbors. The response must retain enough submitted input for the caller to correlate a rejected item when normalized genomic fields do not exist.

Request-wide failures remain request-wide. Invalid JSON, an invalid shared gene filter, a missing or invalid content type, an invalid item count, a body above the limit, or work above the reported model limit keeps its existing non-200 response. Service readiness, queue saturation, cache failure, worker failure, and other operational failures also keep their existing responses.

This corrects the all-rejected boundary established by tickets 0004 and 0011. A downstream batch client must consume the ordered item results for every HTTP 200 response and treat each rejected item as a completed no-score outcome. Update the current migration guidance and public HTTP documentation with the uniform batch rule. Keep historical tickets, records, and publication evidence unchanged.

Done, observably:

- One model-rejected variant returns HTTP 200 with one rejected item result.
- A batch where every model-routed variant is rejected returns HTTP 200 with one rejected result per input in the original order.
- A mixed batch with an invalid variant value and a valid variant returns HTTP 200, preserves the valid result, and reports the invalid item in its original position.
- A batch where every variant value is invalid returns HTTP 200 with one rejected result per input in the original order.
- A rejected item has a stable machine-readable error and remains correlatable to the submitted value even when the service cannot produce normalized genomic fields.
- Request-wide validation, capacity, saturation, readiness, cache, worker, and scoring failures retain their existing HTTP statuses and response contracts.
- The current user documentation explains that HTTP success describes batch processing, while each item status describes annotation success.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change accepted variant forms, normalization, scoring, routing, caching, admission accounting, limits, retry guidance, gene filtering, score values, result order, or operational failure handling. Do not expose detailed backend error text. Do not change the command-line interface. Do not add asynchronous jobs, caller-supplied identifiers, or a new endpoint. Do not rewrite historical tickets, records, or publication evidence.
