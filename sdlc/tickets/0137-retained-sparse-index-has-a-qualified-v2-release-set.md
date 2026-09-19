---
flow: build
priority: 1
deps: ["0136"]
---
# Retained sparse index has a qualified v2 release set

## Outcome

The retained complete sparse member has one reproducible, exhaustively certified `snv-grch38-v2` bundle, transport, proof, and local publication set produced by the exact pushed ticket 0136 tooling.

## Inputs and fixed decisions

Use clean pushed commit `8eb8917ff9f788c28b29287ed3a06b6b4762c554` for both tooling provenance and release target. Use candidate-producing commit `1b1d95d6b5d32112a7c57faeb3af50d88ed93f08`. The retained candidate must be exactly 2,035,371,437 bytes with SHA-256 `354343dc1a9f6558e46693e2481b5181461be2115cd92e029ffd4d4abef4a01e`. The corpus authority is the installed immutable fixed-v1 bundle `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`. Run the Linux-only preparation in an isolated disposable container from the clean worktree. Keep outputs under a new retained data directory.

## Done, observably

- Preflight authenticates the exact candidate, candidate report, fixed-v1 bundle, clean tooling commit, container platform, available disk, and empty output roots before expensive work.
- Two independent clean runs produce byte-identical bundle manifests, notices, score members, transport manifests and parts, proof receipts, release profiles, checksum lists, and release notes.
- Each bundle and reconstructed transport passes exhaustive certification. An isolated install under explicit cache and data roots resolves to the v2 bundle and leaves the existing installation byte-for-byte and metadata-identical.
- The complete checked cross-format corpus returns identical ordered status, gene, score, position, ambiguity, and provenance results for fixed-v1 and sparse-v2 through the command and service routes. Retained warm unfiltered 1, 10, and 100-query latency and complete-file throughput measurements pass ADR 0027 or stop activation.
- The proof records measured part count, installed bytes, fresh-install bytes, bundle and transport identities, candidate provenance, tooling provenance, and exact member sizes and checksums.
- Commit only small canonical v2 proof, profile, checksums, notes, and a durable qualification record. Large bundle and transport bytes remain in the retained data directory. Existing v1 checked bytes remain unchanged.
- `make lint`, `make test`, and `make spec` pass after the small authorities are recorded.

## Boundary

Do not publish GitHub assets, change compiled production sync or runtime authority, activate v2, delete v1 or candidate data, change score precision, or change public command and HTTP output. A later ticket binds the runtime profile, proves upgrade and rollback, and activates v2 only after all evidence passes.
