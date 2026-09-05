---
base: 2e9d0cf582b331e0c138d0d37cc2a7a47c6928c1
head: df6e1858746c7eeaa69b876ff62d6b1798f135a0
---

# Guide retries after model saturation

Temporary model-capacity refusals now carry one decimal-seconds `Retry-After` field. The dispatcher calculates the delay from the admitted variant units captured by the refusal. Requests that exceed the configured capacity receive the unchanged 429 response without retry guidance because waiting cannot make them fit. The status response remains unchanged.

Design review defined the exact planning formula, rounding, snapshot, worker-count behavior, and permanent capacity-mismatch exception. Code review accepted the implementation. The first full specification run caught README growth beyond its enforced word limit. The documentation was condensed without removing the public contract, and the same reviewer accepted the correction. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 275 passed and 7 skipped.
