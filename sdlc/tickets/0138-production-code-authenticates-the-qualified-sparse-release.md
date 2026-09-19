---
flow: build
priority: 1
deps: ["0137"]
---
# Production code authenticates the qualified sparse release

## Outcome

PangoPup strictly authenticates the checked `snv-grch38-v2` proof and release profile and can bind its bundle identity into a compatible runtime profile without selecting it as the active public authority yet.

## What is true today

The checked v2 proof and profile are exact outputs of two accepted retained runs. The ordinary release parser accepts only the v1 schema and fixed index format, and `production_profile()` still selects v1. The runtime-profile builder can certify sparse bundles after tickets 0134 and 0135, but no checked test proves the resulting runtime and scoring identities change only because the admitted SNV identity changed.

## Done, observably

- Closed canonical v2 proof and profile types validate the exact checked bytes, schemas, repository, tag, target/tooling commits, authority chain, source/reference/count/logical facts, sparse provenance, transport members, part order and sizes, proof identity, URLs, and checked sizing sums.
- Any changed, extended, noncanonical, inconsistent, oversized, unknown-format, wrong-authority, or v1/v2-crossed proof/profile fails with a typed release error. Existing v1 parsing, preparation, byte authority, and error behavior remain unchanged.
- One internal qualified-v2 accessor returns only the exact checked v2 authority. The ordinary production accessor, remote sync, installed discovery, and compiled runtime release continue selecting v1 in this ticket.
- Runtime-profile preparation admits a certified sparse-v2 bundle and the unchanged model, reference, mask, and policy. A miniature proves that only the SNV bundle and derived runtime/scoring/data-set identities change; model-side identities and score values remain exact.
- Architecture, release-profile documentation, compatibility guidance, and executable specifications state the inactive qualified-v2 boundary and the later activation requirements.
- Focused red-green tests, `make lint`, `make test`, and `make spec` pass.

## Boundary

Do not construct or publish `runtime-grch38-v2`, switch sync URLs, activate v2, change public output, delete v1, run retained large data, define an unmeasured performance limit, publish GitHub assets, or tag 0.5. Runtime-v2 preparation, route/performance qualification, upgrade/rollback, authority switch, and publication follow separately.
