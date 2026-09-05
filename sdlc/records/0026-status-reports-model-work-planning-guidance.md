---
base: 0351ace
head: 9966aef74000f2dfcc690fb21cc543d1729a518a
---

# Status reports model-work planning guidance

`/v1/status.model` now reports `planning_millis_per_unit` as 10,241 and `full_capacity_planning_seconds` as the upward-rounded product of that factor and configured queue capacity. Both values are JSON integers. The second value uses the same helper as `Retry-After` and does not divide by worker count.

The fields publish retained measurement guidance. They do not guarantee latency or recommend a client timeout. They do not enter scoring identity or change score responses, admission, queue capacity, routing, caching, or scoring.

Design review named and typed the fields and corrected the formula language before implementation. Code review requested boundary tests only. The final tests cover capacities 1, 5, 20, and 1024 plus a two-worker configuration. PangoPup CLI tests, focused HTTP and README specifications, Clippy, formatting, the README size gate, and `git diff --check` passed.
