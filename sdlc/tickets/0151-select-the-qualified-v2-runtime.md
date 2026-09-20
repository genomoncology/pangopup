---
flow: build
priority: 1
deps: ["0150"]
---
# Select the qualified v2 runtime

## Outcome

Ordinary sync, installation, discovery, and scoring select the published sparse v2 authorities while the immutable v1 formats remain readable for explicit rollback and retained installations.

## Done, observably

- Production authority, remote URLs, sync, installation, discovery, runtime admission, and status select the exact published `snv-grch38-v2` and `runtime-grch38-v2` bytes. Crossed, substituted, or incomplete authorities fail closed.
- A clean offline miniature and a fresh anonymous public download prove install, reuse, status, lookup, model routing, and service routing through the ordinary shipping paths. The 1,000-request and seven-group semantic gate remains exact.
- A real 0.4.1 installation upgrades atomically to v2. The old active profile remains usable until the new complete profile commits. Fixed-v1 stays readable through an explicit path and rollback deletes neither asset set.
- Model-cache layout transition, recomputation, later cache hit, and reuse across the fixed-to-sparse index switch remain observable. Both lookup and model routes emit exact hundredths; no third decimal appears.
- Current documentation reports the measured sparse installed and transport sizes. The changelog states that v2 asset digests move and sync is required. Compatibility text tells consumers to store `data_set_version` rather than infer equality from route or score text.
- Container publication checks pin the actual public v0.4.1 predecessor index `sha256:2177c02fc045136a2ef066dbbfa669f59d56dc15e44765e7b7bfbbc9969a6eb8`.
- Red-green tests cover every selector and stale claim. Update architecture, release documentation, the durable record, and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Do not publish v0.5.0 executable or container artifacts, delete or mutate v1 releases, change score precision or semantics, add indel precomputation or threshold triage, or modify a downstream consumer.
