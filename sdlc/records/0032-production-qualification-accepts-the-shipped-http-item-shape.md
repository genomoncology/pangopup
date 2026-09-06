---
base: ea4438e
head: 22963d9
---

# Production qualification accepts the shipped HTTP item shape

The production-output validator now separates HTTP transport fields from the unchanged direct scoring oracle. It requires every submitted `input` exactly, requires one valid lowercase SHA-256 scoring identity across status and all three HTTP score paths, rejects extra properties, and compares the remaining JSON recursively with exact types and ordered arrays.

The portable qualification fixture now emits the complete v0.4.0 item shape for precomputed SNV, automatic modeled indel, and forced model SNV results. Mutation checks reject missing, malformed, inconsistent, extra, wrong-type, and wrong-value fields. They also reject Python's equal-valued integer/float and Boolean/integer cases.

The retained v0.4.0 production output passed after the correction. The original `6801301` to `6801301.0` bypass failed after remediation. Independent design and code reviews accepted the result. `make lint`, `make test`, and `make spec` passed with 283 specifications passing and 7 retained-asset specifications skipped by design.
