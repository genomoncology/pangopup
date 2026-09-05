---
base: 49daf8ccf53d4b210ddbe09d631560d2ff9b7847
head: 116872ae9f10ed71497b239e5de0ed97867bf6cf
---

# Require JSON for HTTP scoring

The scoring route now requires exactly one valid `application/json` content type before readiness checks or body reads. Matching is case-insensitive and accepts legal parameters. Missing, repeated, malformed, non-JSON, and structured-suffix values receive the exact 415 `UNSUPPORTED_MEDIA_TYPE` response. Route and method precedence remains unchanged.

Design review first rejected ambiguous duplicate-header handling, precedence, coverage, and an inaccurate browser-risk explanation. Code review then exposed incorrect edge behavior in the first parser. The final no-allocation scanner implements the required HTTP token and quoted-string grammar, and the same reviewers accepted both remediations. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 277 passed and 7 skipped.
