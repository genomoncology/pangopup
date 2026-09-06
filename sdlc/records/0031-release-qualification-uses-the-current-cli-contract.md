---
base: e4b3071
head: e828c50
---

# Release qualification uses the current CLI contract

The native container and production executable release qualifiers now require the focused lookup help that the current executable ships. The shared expectation accepts the documented literal and exact-edit placeholder. It preserves exact first-line checks, both help flags, and the container's no-network, read-only, no-assets boundary.

A new normal test runs the current executable, compares its focused lookup help with the executable specification, and requires exactly one matching expectation in each release qualification path. This prevents a stale fixture and checker from agreeing with each other while rejecting the shipped executable.

The remote failure at run `34033554891` supplied the red evidence on native AMD64 and ARM64. Independent design and code reviews accepted the correction. The new release-help contract check, production-release qualification fixture, focused CLI specification, shell syntax checks, Python compilation, `make lint`, and `git diff --check` passed before the implementation commit.
