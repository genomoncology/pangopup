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

## Implementation evidence, pending public verification

The isolated `ticket/0151` implementation pins the retained v2 SNV proof,
SNV release profile, runtime release profile, and runtime transport by exact
bytes. Combined sync stages the v2 SNV without changing the selected tuple.
Runtime activation selects the complete v2 profile only after all four assets
validate. Discovery follows the selected runtime profile. The exact v1 profile
remains admitted for rollback; crossed tuples fail closed.

Focused evidence on 2026-09-23: the asset library passes 140 tests with the
runtime-v2 qualification feature, the service fixture lifecycle passes 23
tests with two intentionally ignored, and the exact v1-verifier test passes.
The runtime fault regression proves a distinct staged SNV, consumed
pre-activation fault, prior-pair preservation, successful retry, and explicit
rollback. A retained real-data clone also completed v1→v2→v1 through ordinary
install commands and was removed after verification. Independent implementation
review accepted the remediation. `make lint`, `make test`, `make spec`, anonymous
public asset verification, and the final real public upgrade remain required.
Do not merge this selector until ticket 0150 verifies the public v2 assets.
