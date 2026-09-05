---
flow: build
priority: 10
---
# The version gate runs on the supported Mac

Ticket 0017 replaced the version consistency gate with a checker that imports Python's `tomllib`. The supported Apple Silicon maintainer machine provides Python 3.9.6, where `tomllib` does not exist. `make lint` now stops before any Rust check with `ModuleNotFoundError: No module named 'tomllib'`.

Keep the structured, claim-specific version checks from ticket 0017 and make them run with the repository's existing `python3` on this Mac. Parse only the workspace version and the eight PangoPup package versions needed by the checker. Use no network access and add no Python package dependency.

Start with a test that fails on the ticket 0017 implementation and proves the checker can load and validate the real repository inputs without `tomllib`. Then make the narrowest implementation change that passes that test.

Done, observably:

- The version checker validates the candidate, current-public, historical, and fixed-fixture categories under Python 3.9.
- The checker still rejects a changed workspace version, a changed PangoPup lockfile package version, and each existing claim-specific mutation.
- `make lint`, `make test`, and `make spec` pass on the supported Apple Silicon Mac without reducing specification coverage.
- Linux behavior remains unchanged.

Boundary: do not change any application, package, public-release, historical, or fixture version. Do not weaken or remove a ticket 0017 version check. Do not add a package manager, virtual environment, network step, vendored parser, or third-party Python dependency. Do not change scoring, assets, requests, responses, release tags, images, or publication state.
