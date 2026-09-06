---
base: 291d7a9
head: 2ffaefd
---

# A reviewed descendant can finalize a staged release

Container finalization now separates the current trusted workflow source from the exact staged release commit. Current event and workflow bytes must equal current `origin/main`. The checked-out release commit must be an ancestor of that main commit.

Finalization also requires the published immutable non-prerelease release and its direct tag to identify the staged release commit. Stage-run metadata, receipt contents, leaf digests, native qualification, leaf revision labels, and final index revision remain bound to that commit. New staging still requires one exact current-main commit.

The live immutable v0.4.0 release and tag satisfied the new checks. Negative release, tag, ancestry, and authentication predicates failed as required. Independent design and code reviews accepted the result. `make lint`, `make test`, and `make spec` passed with 283 specifications passing and 7 retained-asset specifications skipped by design.
