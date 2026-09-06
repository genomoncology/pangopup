---
base: 9af77ff
head: 51dedbdf2a84692bfd0cdf67b95dabb2d03faab6
---

# Model work counts work, not submissions

The service now groups request-local uncached model inputs by canonical cache key before cache lookup, the distinct-work limit, and admission. Equivalent inputs within one request use one model execution and one admission unit. Completed and rejected outcomes expand back to every original item in request order with the exact submitted `input`.

Operational failures remain request-wide. Separate requests do not coalesce and each reserves its own work. Cache hits, precomputed outcomes, and invalid inputs continue to consume no model admission units.

Design review added the `max_items` boundary, canonical-key coverage, success and rejection fan-out, operational-failure behavior, cache-write independence, and a cross-request test. Code review accepted the implementation after public wording was limited to uncached model inputs. The focused service suite reported 116 passed. Focused README and HTTP specifications, `make lint`, formatting, the README size gate, and `git diff --check` passed.
