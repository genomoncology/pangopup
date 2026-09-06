---
flow: build
priority: 10
---
# Release qualification uses the current CLI contract

The current CLI accepts literal and exact-edit GRCh38 variants and describes their shared placeholder in focused help. Both native container smoke jobs reject that correct output because the release qualifier still expects the older SNV-only placeholder. The production-release fixture and checker repeat the same stale text, so their local test agrees with itself and stays green.

Release qualification must validate the help contract that the current executable actually ships. One authoritative expectation must cover the container and production executable paths, or a normal gate must prove that every independently retained expectation agrees. The fix must preserve strict first-line help checking and must not weaken the no-assets, non-root, read-only qualification boundary.

Done, observably:

- The current container qualification accepts the shipped focused lookup help on native AMD64 and ARM64.
- The production executable qualification checks the same current focused lookup help.
- A normal local gate fails when either release path drifts from the shipped help contract.
- The remaining focused help routes and their exact first lines stay protected.
- `make lint`, `make test`, and `make spec` pass without reducing coverage.

Boundary: do not change CLI help, variant grammar, scoring behavior, container contents, asset behavior, or release credentials. Do not publish a tag, release, or image in this ticket.
