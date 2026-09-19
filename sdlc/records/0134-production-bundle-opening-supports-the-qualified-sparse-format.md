# Production bundle opening supports the qualified sparse format

`BundleOpen` now dispatches a closed canonical bundle manifest to either the unchanged fixed-v1 reader or the qualified sparse-direct-v1 reader. Both formats preserve ordered filtered and unfiltered score-provider results, source-reference ambiguity, provenance, and format-derived bundle identity. Unknown formats, mismatched media types, mismatched payloads, and closed-schema extensions fail before lookup. Sparse bounded open still avoids ordinary payload reads, and touched corruption reaches the provider corruption boundary.

Fixed-v1 exhaustive certification and measured lookup remain fixed-only. Sparse input returns the distinct typed `BUNDLE_INCOMPATIBLE` asset error rather than corruption, conflict, panic, or misleading fixed-v1 evidence. The active release profile remains fixed-v1. No score precision, command-line, HTTP, or installed identity changed. Unfiltered complete-corpus performance remains unmeasured.

Independent design review accepted the corrected ticket. Independent code review rejected an initially erased incompatibility type and then an incorrect installation-conflict classification; the final dedicated error kind passed re-review. `make lint`, `make test`, `make spec`, and `git diff --check` passed on macOS. The specification gate ran 193 blocks.
