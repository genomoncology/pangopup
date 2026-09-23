---
flow: build
priority: 1
deps: ["0157"]
---
# Align transport test with current SNV builder fingerprint

## Outcome

The Linux transport test accepts the checked current SNV builder fingerprint while still proving that the generated manifest carries it and causal source changes alter it.

## Done, observably

- Reproduce `builder_identity_covers_assets_manifest_notice_and_certification_source` at pushed main `762ab07`. Its pinned `c40e9b…` fingerprint is historical; the current builder and source fingerprint unit oracle report `7e3c2305…`.
- Update only the test's pinned current fingerprint. Keep generated manifest equality and all three causal mutation checks.
- Pass the focused Linux test, full Linux builder suite, and repository lint, test, and spec gates. Record any unrelated failure separately.

## Boundary

Change no production code, source fingerprint inputs, published assets, or historical fixture bytes.
