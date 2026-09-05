---
base: e723a917b4d10372a1fe15bd1cf4c7b273e26704
head: c53f8cb7ca5a62a577e14ab595c4131905eb25de
---

# Preserve valid results when one model variant is rejected

The HTTP scoring route now returns an ordered item-level rejection beside normal authoritative, cached, or modeled outcomes. A mixed response uses HTTP 200 when it has at least one normal outcome and no operational failure. A request where every input is model-rejected retains HTTP 422. Scoring, cache, worker, and readiness failures still invalidate the complete request.

Independent code review found imprecise documentation and missing mixed cache coverage. The remediation corrected the status rules and proved that a cache hit remains byte-identical, stays in order, and does not run through the model. The same reviewer accepted both corrections. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 277 passed and 7 skipped.
