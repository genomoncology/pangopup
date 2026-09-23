---
flow: build
priority: 1
deps: ["0155"]
---
# Rebind historical SNV regression fixture provenance

## Outcome

The Linux SNV regression test regenerates the checked score fixture and still verifies the current builder's source fingerprint.

## Done, observably

- Reproduce `checked_regression_fixture_regenerates_byte_exactly` at pushed commit `6379272`. Its generated manifest reports `sha256:7e3c2305…`; the checked historical manifest reports `sha256:c40e9b93…`.
- Assert the generated manifest carries the current fingerprint already pinned by the source fingerprint unit oracle. Rebind that field and the existing builder version solely for comparison with the historical manifest.
- Keep the checked fixture, production builder, generated bundle identity, member bytes, score records, and all other byte comparisons unchanged.
- Pass the focused regression test, the Linux builder tests, and repository lint, test, and spec gates. Record any unrelated failure.

## Boundary

Change only the regression test's historical comparison. Do not regenerate or republish a score bundle.
