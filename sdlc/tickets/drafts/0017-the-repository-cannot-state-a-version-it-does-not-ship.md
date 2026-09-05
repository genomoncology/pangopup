---
flow: build
priority: 6
---
# The repository prepares one coherent 0.4.0 application candidate

PangoPup has completed the application work intended for its next release, but the source application, executable expectations, qualification checks, and container staging workflow still identify 0.3.0. `architecture/service.md` is older still. It says the public container is 0.2.0 and the repository prepares 0.3.0 even though 0.3.0 is already public.

Prepare the source application as version 0.4.0 without publishing it. The workspace version, all eight PangoPup package entries in the lockfile, CLI and HTTP version outputs, current qualification expectations, and the container workflow's candidate version must agree on 0.4.0. The service architecture must state that 0.3.0 remains public while the repository prepares 0.4.0.

The repository also needs a gate that distinguishes three kinds of version statement. Application-candidate claims must agree with the workspace version. Current-public-release metadata must continue to identify 0.3.0 until publication. Historical release records and deliberate fixture inputs must keep the versions they recorded. A disagreement must fail a normal repository gate and name the file and expected category.

The current-public category includes citation metadata and its validation, public README download and image examples, public container and first-use specifications, delivery architecture, and the planning FAQ. Append-only publication evidence remains history. The explicit 0.3.0 active-identity input and the container-tag-absence sample remain fixed fixtures.

The active scoring identity includes the application version. Preparing 0.4.0 must therefore change the current service identity even when every asset and CPU policy stays the same. The algorithm and its pinned explicit-version fixtures remain unchanged.

Done, observably:

- The built CLI and `/v1/status` report application version 0.4.0.
- The lockfile, production qualification, and container staging workflow agree with the declared application version.
- Public installation and citation material still identifies the immutable 0.3.0 release until publication.
- Service architecture states the real transition from public 0.3.0 to candidate 0.4.0.
- A normal gate rejects drift in an application-candidate or current-public claim and names the offending file without rewriting history or fixed fixtures.
- The current scoring identity changes for 0.4.0, and a test proves that the version remains part of its preimage.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not create a Git tag, GitHub release, container tag, or moving `latest` tag. Do not finalize or publish an image. Do not claim that 0.4.0 is public. Do not rewrite publication records, release notes, prior-release documentation, or fixed-version test inputs. Do not change scoring, assets, the scoring-identity algorithm, or any public request or response shape beyond the expected application version and resulting identity value.
