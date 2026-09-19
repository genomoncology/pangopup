---
flow: build
priority: 1
deps: ["0136"]
---
# Retained sparse index has a reproducible v2 publication set

## Outcome

The retained complete sparse member has one reproducible, exhaustively certified `snv-grch38-v2` bundle, transport, proof, and local publication set produced by the exact pushed ticket 0136 tooling.

## Inputs and fixed decisions

Use clean pushed commit `8eb8917ff9f788c28b29287ed3a06b6b4762c554` for both tooling provenance and data-release target. Use candidate-producing commit `1b1d95d6b5d32112a7c57faeb3af50d88ed93f08` as the record 0132 association. The retained candidate must be 2,035,371,437 bytes with SHA-256 `354343dc1a9f6558e46693e2481b5181461be2115cd92e029ffd4d4abef4a01e`; its 1,177-byte report must hash to `e1c0a684aa70eaffe6aae26133a89b3cf38fd6baa40437d70d15e2a3fd72ae5a`. The corpus authority is the installed immutable fixed-v1 bundle `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`.

## Done, observably

- Preflight authenticates the exact candidate, report, fixed-v1 manifest, notice and member, clean tooling commit, Linux container identity and platform, Rust version, executable hash, host and container storage, absent output roots, and current installed-state snapshot.
- Build `--locked --release` from a standalone clean detached clone of the exact tooling commit with readable Git metadata and a fresh external Cargo target. Mount source and all retained inputs read-only. Record the image digest and exact commands.
- Two independent runs use separate absent roots and perform assembly, transport packing, release preparation, and isolated installation. They produce identical member sets, bytes, sizes, and hashes for bundle, transport, proof, profile, checksum list, and notes.
- Each assembled bundle and reconstructed transport passes exhaustive certification. The isolated install reports SNV ready and runtime missing. Explicit and discovered command lookup agree on the checked regression requests.
- Snapshot comparison proves the existing installation and fixed-v1 authority retain the same paths, modes, sizes, content hashes, link identities, and modification metadata. Access timestamps are excluded. The task may activate v2 only inside its new isolated test root.
- Retain one complete publication set plus both logs and comparison evidence under a new retained data directory. Remove only the explicitly recorded duplicate outputs after byte comparison. Do not alter the candidate or installed v1 files.
- Commit only small canonical v2 proof, profile, checksums, notes, exact command record, and qualification record. Record measured part count, installed bytes, fresh-install bytes, bundle and transport identities, candidate and tooling provenance, member sizes, and checksums. State that the generated authorities land after their target commit.
- Existing v1 checked bytes remain unchanged. `make lint`, `make test`, and `make spec` pass after small authorities are recorded.

## Boundary

This ticket does not claim command/service parity, unfiltered production latency, or complete-file throughput. Those lack current harnesses and defined acceptance limits. They remain activation blockers. Do not publish GitHub assets, change compiled sync or runtime authority, activate v2 outside the isolated root, delete v1 or candidate data, change score precision, start a public service, or change public output. A later ticket adds strict v2 authority consumption, executable cross-format route evidence, measured activation gates, runtime binding, upgrade, and rollback.
