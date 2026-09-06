---
flow: build
priority: 10
---
# Production qualification accepts the shipped HTTP item shape

The v0.4.0 executable passed its production run against the retained qualified assets, but the checked production-output validator rejected the valid HTTP SNV result with `HTTP SNV response mismatch`. The returned item carried the published `input` and `scoring_identity` properties. The comparison oracle represents the same scored result without transport-only service properties. Normal gates stayed green because their generated qualification fixture did not expose this difference.

Production qualification must compare the score-bearing content that the HTTP and direct lookup paths share while still checking every service-only property against its own published contract. A correct v0.4.0 result must pass for the precomputed SNV, automatic modeled indel, and forced model SNV. Every HTTP item's `input` must equal the submitted request value. Every `scoring_identity` must use the published lowercase SHA-256 form and must equal the status identity and every other returned item identity. A removed field, wrong type or value, malformed identity, cross-item or status mismatch, or extra item property must fail.

Done, observably:

- The complete checked production qualification accepts the retained v0.4.0 precomputed SNV, automatic modeled indel, and forced model SNV HTTP responses while preserving their direct-oracle score and provenance checks.
- A portable test reproduces the former false rejection with all three current published item shapes before the fix and passes afterward.
- Mutation coverage proves that every missing, wrong-type, wrong-value, malformed, cross-item, status-inconsistent, or extra transport property fails.
- The normal lint, test, and executable specification gates pass.

Boundary: Keep the HTTP response contract and the scored-result oracle unchanged. Do not weaken record, provenance, score, ordering, or response-envelope checks. Do not change the published v0.4.0 release, tag, executable assets, staged container leaves, or scoring assets.
