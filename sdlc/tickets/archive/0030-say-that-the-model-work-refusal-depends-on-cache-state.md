---
flow: build
priority: 4
---
# Say that the model-work refusal depends on cache state

`MODEL_BATCH_TOO_LARGE` reads as a permanent contract violation. It is not. The refusal counts variants the cache cannot already answer, so the same bytes can be refused once and accepted a moment later.

Eleven uncached variants were refused with HTTP 422. One of the eleven was then scored by a separate single-variant request. The identical eleven-variant request was resubmitted and returned HTTP 200 with eleven results, because one of them had become a cache hit.

Current guidance does not say this. A consumer reading the published contract reasonably concludes that a 422 means its request was malformed against a fixed limit and fails visibly on a condition that known cache population can change. Waiting alone does not help because a refused request performs no inference or cache write. Another successful request or explicit prewarming can populate the cache, but cache writes are best-effort and bounded entries can be evicted. The same consumer will also size every batch to the uncached-model ceiling forever, because it has no reason to believe a larger batch is ever admissible. A request of one hundred variants already in the precomputed index is accepted and answered immediately.

State in the published contract that the limit counts distinct canonical cache keys that remain misses after the live cache lookup. It does not count submitted items or time. Say that an unchanged refused request can succeed later when another successful operation has populated enough of those keys, but do not promise that outcome. Say that a batch within the item limit whose variants need no uncached model work is accepted regardless of the uncached-model ceiling. Document that this 422 has no `Retry-After`, and that a caller without evidence of cache warming should split or reduce distinct work to the conservative reported ceiling rather than retrying the same request because time passed. Keep the status code, the error code, and the message exactly as they are, because the refusal itself is correct.

Done, observably:

- The HTTP specification states that the limit counts distinct canonical cache keys that remain misses after the live cache lookup.
- It states that an identical request refused once can be accepted later after known cache population, without promising persistence or eventual success.
- It states that a batch within the item limit is accepted when its variants need no uncached model work.
- It states that this 422 has no `Retry-After`, that time alone does not help, and that blind unchanged retries are not advised.
- It tells a caller without evidence of cache warming to split or reduce distinct work to the conservative reported ceiling.
- A normal specification check protects these statements from removal.
- The status code, error code, and message for the refusal are unchanged.
- No behavior, field, value, limit, or response shape changes.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: this is a documentation change. Do not change any limit, status, code, message, response, admission behavior, or executable behavior. Do not add a new field, endpoint, or configuration. Do not rewrite historical tickets, records, or publication evidence.
