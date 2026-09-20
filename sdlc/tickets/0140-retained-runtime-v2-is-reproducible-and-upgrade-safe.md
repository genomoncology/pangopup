---
flow: build
priority: 1
deps: ["0139"]
---
# Retained runtime v2 is reproducible and upgrade-safe

## Outcome

PangoPup has one retained, reproducible `runtime-grch38-v2` publication set bound to the qualified sparse SNV release, and release evidence proves the real 0.4.1-to-0.5.0 SQLite cache transition plus fixed-v1 rollback before any asset publication or production authority switch.

## What is true today

Commit `c0c7054dbf5be948f44c2e9b871ed626c57050cc` contains the reviewed clean runtime-v2 preparation tool and executable semantic gate. The retained sparse publication set lives at `/Users/ian/workspace/data/pangopup/sparse-release-v2-2026-09-19/publication-runs/run-a`. The exact v1 runtime transport lives in `/Users/ian/workspace/data/pangopup/runtime-v0.5.0-cache/profiles/d1caf6346bb24378f720056416fa6286f1153ccaf0c6a0778494f557035ef59e/transport`. Production selection, sync, installed discovery, and runtime admission still select v1. No retained runtime-v2 output exists, and no v2 asset is public.

## Done, observably

- Commit this reviewed ticket on `main` and use that clean pushed commit as the independently supplied release target. Build the preparation executable from exact clean pushed tooling commit `c0c7054dbf5be948f44c2e9b871ed626c57050cc`. Record the compiler, locked dependencies, host, commands, input paths, file identities, tooling commit, and distinct release-target commit before opening large inputs.
- Copy no retained source into the repository. Create one dated retained root under `/Users/ian/workspace/data/pangopup/`. Snapshot the existing v1 installed data root before and after. Every retained command uses explicit data, cache, and output paths. A failed or interrupted run leaves no active product state and no unexplained staging output.
- Prepare runtime v2 twice from independently empty output directories. Both runs use the exact retained v1 runtime transport, the exact qualified sparse-v2 release profile and bundle, the clean tooling commit, and the ticket commit as target. Compare every output byte. The runtime profile, runtime transport manifest, release profile, checksums, release notes, and source supplement match exactly between runs.
- Prove the eight model, reference, mask, manifest, and notice transport members match v1 byte for byte. Prove only `runtime-profile.json` and `runtime-transport.json` differ. Prove the new inner profile names the checked sparse SNV bundle and preserves every non-SNV source, policy, model, reference, and mask field.
- Verify, pack, unpack, and install the retained runtime-v2 transport in isolated roots through production-equivalent code with an explicit checked qualification authority. Do not widen ordinary production admission. The installed components and inner profile authenticate to the prepared release. Offline reuse performs no network work.
- Run the ticket 0139 semantic gate plus retained cross-format qualification. The retained qualification uses the same 1,000-request corpus and seven groups, real fixed-v1 and sparse-v2 providers, actual model/reference/mask files, command output, service output, every position-one miss, overlap and filtered overlap, `REF=N`, found/miss/rejected ordering, and exact allowed identity differences. It reports counts and identities. It adds no new latency or throughput threshold.
- Exercise one actual v0.4.1 SQLite model cache in an isolated root. Populate it with a retained model-routed non-SNV using the public v0.4.1 executable and v1 runtime. Open the same cache with the candidate 0.5 executable and retained v2 runtime. Prove the old cache is discarded exactly once because its recorded setup is incompatible, the first 0.5 request recomputes and stores one hundredth-precision result, the next identical request is a cache hit without another model evaluation, and every returned and stored identity belongs to 0.5/v2. Scores remain exact hundredths on both SNV and model routes; this ticket does not introduce a third decimal.
- Exercise rollback with isolated copies. Fixed-v1 remains readable by 0.5. A v2 cache row never answers under the v1 scoring identity, and a v1 cache row never answers under v2. Rollback does not delete the retained v1 assets or mutate the retained v2 publication set.
- Record exact output names, byte counts, SHA-256 values, runtime profile identity, data-set version, scoring identity, SQLite observations, semantic counts, and before/after retained-root fingerprints in the repository. Check in only the small authority metadata and evidence summary needed for later publication and activation.
- Independent code and evidence review finds no unexplained byte change, authority widening, cache alias, partial output, retained-root mutation, or claim unsupported by a command. `make lint`, `make test`, and `make spec` pass from the exact resulting commit.

## Boundary

Do not publish GitHub assets, change production selectors or URLs, make v2 discoverable by ordinary sync or installation, tag 0.5, publish executable or container artifacts, delete v1, change public score precision, introduce indel precomputation or threshold triage, or modify a downstream consumer. Publication, activation, release text, container predecessor correction, current user documentation, and downstream reminders follow after this retained evidence is accepted.
