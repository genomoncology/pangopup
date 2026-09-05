---
base: 6d3b52e56c35356e4f17bea8edd1124ba879243a
head: b48aeb1d041c26cdfe252f49e70852ed0f367375
---

# Reject model input before runtime admission

The engine now owns one shared request-only model validator for unsupported shapes, alleles longer than 100 bases, and insufficient fixed left context. Scoring reuses the same validation. The CLI calls it before model-side runtime admission. Lookup-first routing still allows an authoritative SNV to complete without model-context validation. Batches report the first request-only rejection in input order and emit no partial output.

Design review defined the exact request-only set, the deliberate precedence over asset failures, both CLI routes, the authoritative lookup exception, shared ownership, and documentation. Code review accepted the implementation. Root review corrected one misleading architecture phrase. The same reviewer accepted the correction. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 279 passed and 7 skipped.
