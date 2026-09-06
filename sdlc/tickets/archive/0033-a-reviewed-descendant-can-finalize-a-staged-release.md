---
flow: build
priority: 10
---
# A reviewed descendant can finalize a staged release

The immutable v0.4.0 executable release and held native container leaves identify commit `ea4438e50762e32f09052b364060c89201ed78bc`. A release-qualification checker defect was then fixed and recorded on `main`. The container finalization workflow now refuses before reading the authenticated stage receipt because it requires the release commit, workflow commit, event commit, checkout, and current `origin/main` to remain identical. The release commit is a verified ancestor of current `main`; no product, image, staged leaf, executable asset, tag, or scoring asset changed.

Container finalization must accept one exact successful stage receipt for an immutable release commit when the workflow runs from a clean current `main` that descends from that release commit. Finalization must read the existing public release and tag, require the expected version, require a published immutable non-prerelease state, and require both the release target and tag reference to resolve to the receipt's exact release commit. It must keep the held leaf digests and every public image revision label bound to that release commit. It must reject an unrelated, future, rewritten, unreleased, differently tagged, or otherwise unauthenticated commit. New stage runs must continue to require one exact current-main commit.

Done, observably:

- Finalization accepts the v0.4.0 receipt from run `34039157332` while current `main` descends from its exact release commit.
- Finalization still authenticates the current workflow and event source as exact current `origin/main`.
- Finalization requires the published immutable v0.4.0 release target and tag reference to resolve to the receipt's exact release commit, and a normal gate rejects a missing or disagreeing release binding.
- The receipt, held leaf digests, checkout used for qualification, and published revision remain bound to the exact release commit.
- A normal gate proves that stage mode still rejects a commit that differs from current `main` and that finalize mode rejects a release commit outside current `main` history.
- The normal lint, test, and executable specification gates pass.

Boundary: Do not change the v0.4.0 release, executable assets, release tag, staged leaf digests, image contents, image version, scoring assets, or predecessor alias check. Do not weaken receipt identity, tag-absence, anonymous native qualification, or final manifest checks.
