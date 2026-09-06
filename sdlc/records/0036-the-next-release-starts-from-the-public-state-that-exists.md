---
base: 2f38878
head: 5d689b3
---

# The next release starts from the public state that exists

The repository now prepares v0.4.1 while representing the split predecessor accurately. GitHub's public executable remains immutable v0.4.0. GHCR remains v0.3.0 because v0.4.0 container publication stopped before aliases were created. The version gate tracks those facts independently.

The immutable v0.4.0 release-note bytes remain protected. Its publication record now pins the public release, direct tag, six-member inventory, abandoned staged leaves, successful qualification, failed full uninstall, absent v0.4.0 container aliases, and unchanged v0.3.0 container predecessor. New v0.4.1 notes and a prepared runbook cover the uninstall fix and release-gate portability work without claiming a scoring or API contract change.

The v0.4.1 runbook uses an out-of-commit user message or receipt for the exact-commit authorization binding. Its checker requires 22 safety-critical steps in order. The final executable-publish and index-creation gates must remain immediately adjacent to their public mutations. Mutation tests cover removed, reordered, and intervening steps.

Independent design review required a complete authorization binding, full v0.4.0 historical evidence, phase-specific predecessor states, and complete executable-release verification. Independent code review found and resolved the self-referential commit placeholder, phase contradiction, incomplete ordering enforcement, and incomplete partial-record checks. `make lint`, `make test`, and `make spec` passed with 283 specifications passing and 7 retained-asset specifications skipped by design.
