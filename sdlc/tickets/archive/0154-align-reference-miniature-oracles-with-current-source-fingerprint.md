---
flow: build
priority: 1
deps: []
---
# Align reference miniature oracles with the current source fingerprint

## Outcome

The reference builder's miniature integration tests accept the already checked current builder source fingerprint and continue to prove stable reference and notice bytes.

## Done, observably

- Reproduce the Linux `builder_provenance` failure at the pushed release candidate. Its generated manifest carries `9d19e7cc…` while three integration assertions still expect `09cd4444…`.
- Update only those three assertions to the current checked reference source fingerprint. Keep the historical v1 source identity, fixture manifests, member hashes, production runtime identity, and fingerprint implementation unchanged.
- Run the two focused reference integration tests on Linux and the repository's lint, test, and spec gates. Record the result.

## Boundary

The change updates test expectations for a source fingerprint that changed before this ticket. Do not rebuild or republish a reference bundle.
