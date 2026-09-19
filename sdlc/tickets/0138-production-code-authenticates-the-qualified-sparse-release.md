---
flow: build
priority: 1
deps: ["0137"]
---
# Production code authenticates the qualified sparse release

## Outcome

PangoPup strictly authenticates the checked `snv-grch38-v2` proof and release profile and can bind its bundle identity into a compatible runtime profile without selecting it as the active public authority yet.

## What is true today

The checked v2 proof and profile are exact outputs of two accepted retained runs. The ordinary release parser accepts only the v1 schema and fixed index format, and `production_profile()` still selects v1. Sparse parsing and exhaustive certification exist, but runtime-profile preparation rejects every non-v1 SNV and then requires the existing production tuple. Its SNV inspection reads bounded metadata and file size rather than payload bytes, so exhaustive certification remains a prerequisite.

## Done, observably

- Closed canonical v2 proof and profile types validate the exact checked bytes, schemas, repository, tag, target/tooling commits, authority chain, source/reference/count/logical facts, sparse provenance, transport members, part order and sizes, proof identity, URLs, and checked sizing sums.
- Any changed, extended, noncanonical, inconsistent, oversized, unknown-format, wrong-authority, or v1/v2-crossed proof/profile fails with a typed release error. Existing v1 parsing, preparation, byte authority, and error behavior remain unchanged.
- One internal qualified-v2 accessor returns only the exact checked v2 authority. The ordinary production accessor, remote sync, installed discovery, and compiled runtime release continue selecting v1 in this ticket.
- Inner canonical runtime-profile preparation admits the qualified sparse-v2 bundle only after its independent certification prerequisite and keeps the unchanged model, reference, mask, and policy. Hold software version and CPU policy fixed. A miniature asserts equality of every non-SNV profile field, replaces only the SNV descriptor, derives runtime-profile, scoring, and data-set identities through existing production functions, and verifies changed identities plus exact fixed/sparse lookup results. This proves derivation and fixture parity, not active HTTP or full route qualification.
- The existing trusted-production tuple check, production runtime admission, installed discovery, remote sync selector, and compiled runtime release remain v1-only. A prepared sparse inner profile still fails production admission.
- Architecture, release-profile documentation, compatibility guidance, and executable specifications state the inactive qualified-v2 boundary and the later activation requirements.
- Focused red-green tests, `make lint`, `make test`, and `make spec` pass.

## Boundary

Do not construct or publish the outer `runtime-grch38-v2` release or transport, install or activate a v2 runtime, switch sync URLs, change public output, delete v1, run retained large data, define an unmeasured performance limit, publish GitHub assets, or tag 0.5. Outer runtime-v2 preparation, active route/performance qualification, upgrade/rollback, authority switch, and publication follow separately.
