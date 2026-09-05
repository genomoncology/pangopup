---
base: f885cb2acf10ec218d27508b35cf459866b87b25
head: 555b1031f63ec9b0f041178e64714ca5cf5f75a7
---

# A valid batch reports every item outcome

`/v1/score` now returns HTTP 200 with one ordered result per submitted variant whenever the scoring envelope and shared options are valid and no operational failure occurs. This rule covers singleton, mixed, and all-rejected batches.

An invalid variant value returns `status: "rejected"`, its exact submitted `input`, empty result collections, the stable `INVALID_VARIANT` error, and the active scoring identity. A normalized variant that the model rejects retains its normalized genomic fields and `MODEL_REJECTED`. Invalid items consume no lookup, cache, admission, or inference work.

Request-envelope failures, shared-option failures, body and item limits, the reported model-work limit, saturation, readiness, cache failure, worker failure, scoring failure, and reference-provider failure remain request-level responses. The design and code reviews verified this boundary. Code review required stronger tests for ordered distinct model rejections and for a provider failure after an earlier invalid item. A later documentation review restored existing content-type and provenance promises after README compression.

`make lint`, `make test`, and `make spec` passed on the final Linux tree. The specification suite reported 282 passed and 7 skipped.
