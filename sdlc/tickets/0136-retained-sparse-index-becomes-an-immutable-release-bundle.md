---
flow: build
priority: 1
deps: ["0135"]
---
# Retained sparse index becomes an immutable release bundle

## Outcome

The retained complete sparse SNV member becomes one deterministic, exhaustively certified three-file bundle and one locally prepared immutable `snv-grch38-v2` publication set.

## What is true today

The exact 2,035,371,437-byte sparse member at `data/pangopup/sparse-candidate-2026-09-16/scores.pgi` passed complete logical parity and retained latency gates. Runtime lookup and exhaustive certification accept its format. It has no bundle manifest, transport, proof receipt, or release profile. The active and public SNV release remains fixed-v1.

## Done, observably

- A deterministic maintainer command binds the retained sparse member to the exact certified v1 corpus facts and attribution without rebuilding score data or trusting an editable report as authority.
- The resulting canonical three-file bundle certifies completely as sparse-direct-v1 and has a new immutable bundle identity while retaining the exact v1 source and decoded logical identities and counts.
- Existing deterministic SNV transport pack, verify, unpack, install, and release preparation accept the certified sparse bundle without weakening fixed-v1 admission or hard-coding candidate paths.
- The prepared `snv-grch38-v2` set records exact member names, sizes, checksums, URLs, proof, target commit, installed bytes, and fresh-install bytes. It is byte-reproducible from the retained member and admitted v1 authority.
- A reconstructed and installed bundle certifies to the same v2 identity and answers a checked cross-format corpus exactly like v1.
- The checked v1 release authority remains readable and unchanged. No active compiled profile moves, no network publication occurs, and no runtime-side release is prepared in this ticket.
- Focused red-green tests, `make lint`, `make test`, and `make spec` pass.

## Boundary

Do not activate v2, modify score values or precision, delete fixed-v1 data, change public command or HTTP output, publish GitHub assets, or prepare the runtime profile that binds the new SNV identity. Command/service parity, upgrade/rollback evidence, and activation follow separately.
