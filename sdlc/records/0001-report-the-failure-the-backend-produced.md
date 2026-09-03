---
base: efdbed4adb201adac6cc871fa2eb18fafb07d27c
head: 437e7a2b01ae5a4cb96ba1d59abe7ba037c88a8e
---

# Report the backend failure family over HTTP

HTTP scoring errors now preserve the backend's stable `MODEL_REJECTED`,
`MODEL_SCORING`, or `MODEL_CACHE_INVALID` code while retaining the generic
message and HTTP 500 response. This lets downstream callers distinguish
request, inference, and cache failures without exposing backend details.
