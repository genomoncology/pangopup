---
flow: build
priority: 4
---
# Say that the model-work refusal depends on cache state

`MODEL_BATCH_TOO_LARGE` reads as a permanent contract violation. It is not. The refusal counts variants the cache cannot already answer, so the same bytes can be refused once and accepted a moment later.

Eleven uncached variants were refused with HTTP 422. One of the eleven was then scored by a separate single-variant request. The identical eleven-variant request was resubmitted and returned HTTP 200 with eleven results, because one of them had become a cache hit.

Current guidance does not say this. A consumer reading the published contract reasonably concludes that a 422 means its request was malformed against a fixed limit, treats it as an integration defect, and fails visibly on a condition that waiting or a warmer cache resolves. The same consumer will also size every batch to the uncached-model ceiling forever, because it has no reason to believe a larger batch is ever admissible. A request of one hundred variants already in the precomputed index is accepted and answered immediately.

State in the published contract that the uncached-model count is measured against live cache state at request time, not against the submitted item count, and that a refused request can succeed unchanged once its variants are cached. Say that a batch within the item limit whose variants are already answerable is accepted regardless of the uncached-model ceiling. Keep the status code, the error code, and the message exactly as they are, because the refusal itself is correct.

Done, observably:

- The HTTP specification states that the uncached-model count is measured against live cache state at request time.
- It states that an identical request refused once can be accepted later without changing.
- It states that a batch within the item limit is accepted when its variants need no uncached model work.
- The status code, error code, and message for the refusal are unchanged.
- No behavior, field, value, limit, or response shape changes.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: this is a documentation change. Do not change any limit, status, code, message, response, admission behavior, or executable behavior. Do not add a new field, endpoint, or configuration. Do not rewrite historical tickets, records, or publication evidence.
