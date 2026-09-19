---
flow: build
priority: 1
deps: ["0135"]
---
# Sparse release preparation has a trusted authority chain

## Outcome

Maintainers can deterministically assemble and prepare a sparse SNV release from authenticated fixed-v1 corpus authority without changing the active or published v1 contract.

## What is true today

The exact retained sparse member passed complete logical parity and latency gates, but it has no trusted bundle manifest or release proof. The existing preparer compiles one fixed-v1 receipt and profile and rejects other index formats. Its fixed-builder fingerprint does not cover the sparse writer, converter, or assembler.

## Authority and provenance decision

The checked immutable `snv-grch38-v1` identity is the sole corpus authority. Assembly must authenticate that full bundle before it inherits attribution, source, reference, counts, and logical identities. It must hash and exhaustively certify the supplied sparse member against those facts. The retained candidate report supplies evidence only and grants no authority. The sparse manifest records the candidate-producing commit separately and carries a new assembler fingerprint over every byte-producing sparse writer, converter, assembler, manifest schema, and dependency input. This decision can be overturned before v2 publication.

## Done, observably

- A maintainer construction command admits the exact v1 production authority plus a supplied sparse member and candidate-producing commit, then atomically publishes a canonical three-file sparse bundle without rebuilding scores.
- Changed v1 authority bytes, source-only counts, reference facts, sparse bytes, logical identities, candidate commit shapes, symlinks, replacements, and existing output fail with typed errors and leave no partial publication.
- The resulting miniature bundle certifies completely as sparse-direct-v1. Its inherited corpus facts match v1, its bundle identity differs, and its builder provenance covers the actual sparse byte path.
- Transport pack, verify, unpack, and isolated install accept certified sparse bundles while preserving encoder settings, 1,000,000,000-byte parts, ordinal names, the final-part rule, and the 1,000-part ceiling. No two-part assumption remains outside the immutable v1 contract.
- A maintainer v2 proof/profile construction path derives closed canonical data only after complete bundle and transport verification. The ordinary production profile, remote sync, runtime authority, fixed-v1 receipt/profile bytes, and v1 release preparation remain unchanged.
- Miniature tests prepare twice and compare bundle metadata, parts, proof, profile, checksum list, and notes byte for byte. They state SNV-only installed bytes and fresh-install bytes as explicit member sums.
- Architecture, release-profile documentation, and executable specifications describe the authority, provenance, deterministic sizing, and two-step release sequence.
- Focused red-green tests, `make lint`, `make test`, and `make spec` pass.

## Boundary and sequence

This ticket changes tooling and miniature evidence only. Commit and push it before any retained-data run. A later ticket runs the exact clean tooling SHA in Linux against retained data, prepares `snv-grch38-v2`, records measured part counts and identities, and commits the small authorities separately. Run retained installation under dedicated cache and data roots and prove the existing installation did not change. Do not activate v2, prepare a runtime profile, publish network assets, change score values or public output, or delete fixed-v1 data.
