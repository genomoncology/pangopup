---
base: 2d4ffea
head: 545cb80ce9f948c07ed43d69e0237cc52a0290a1
---

# Say that the model-work refusal depends on cache state

The published HTTP contract now says that the uncached-model limit counts distinct canonical cache keys that remain misses after the live cache lookup. It explains that known cache population can let an identical request succeed later. It does not promise persistence or eventual success because cache writes are best-effort and bounded entries can be evicted.

The contract also says that a batch with no uncached model work can use the full item limit. A `MODEL_BATCH_TOO_LARGE` response remains HTTP 422 without `Retry-After`. Waiting alone does not help. A caller without evidence of cache warming should split the batch or reduce distinct uncached model work to the reported ceiling instead of retrying the same request blindly.

Design review corrected the original claim that waiting could resolve the refusal and pinned the count to distinct canonical live-cache misses. Code review found that the first executable documentation check could read its own assertions after a section boundary was removed. The final check stops at the next section heading or code fence, and independent re-review accepted it. The focused HTTP specification reported 9 passed. `make lint` and `git diff --check` passed before the implementation commit.
