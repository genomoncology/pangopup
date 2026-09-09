---
flow: build
priority: 10
---
# The repository records the public v0.4.1 delivery

The v0.4.1 publication record remains `PREPARED`, and current-state architecture and planning still describe an executable-only v0.4.0 release beside a v0.3.0 container. Public reads on 2026-09-06 now show immutable GitHub Latest release ID `383676522`, tag `v0.4.1`, and target commit `ba8b62180ecd5750a575944d2070f83ca585f4ed`. Anonymous registry reads show `0.4.1`, `v0.4.1`, and `latest` resolving to OCI index `sha256:2177c02fc045136a2ef066dbbfa669f59d56dc15e44765e7b7bfbbc9969a6eb8`. The repository must not keep claiming that publication is incomplete after the required executable, installer, uninstall, native-container, and anonymous verification checks have passed.

The repository must record the completed v0.4.1 publication from observed evidence. Current-state architecture, planning, and version consistency checks must then describe one coherent public v0.4.1 executable and container release. Historical publication records, release bodies, and fixed-version fixtures must remain unchanged.

Done, observably:

- The v0.4.1 publication record says `COMPLETE` only after every qualification and verification required by its prepared runbook has passed. It records the exact authorization binding, UTC publication date, publication commit, successful commit-bound workflow runs, admitted executable and container artifacts, public release identity, native container leaves, public OCI index, anonymous verification, installer result, and isolated uninstall results from retained evidence.
- The completed record contains no token, request header, authenticated download URL, credential path, unsupported inference, or placeholder.
- Current-state architecture and planning describe the immutable v0.4.1 executable and the native AMD64/ARM64 v0.4.1 container as public and qualified. They no longer describe the former split v0.4.0 executable and v0.3.0 container state as current.
- Version consistency checks enforce the coherent public v0.4.1 state and reject stale candidate, prepared, or split-predecessor claims in current-state documents.
- The immutable v0.4.0 executable-only history, the v0.3.0 publication record, every earlier release body, archived SDLC history, and fixed-version test fixtures remain byte-identical.
- The normal lint, test, and executable specification gates pass.

Dependency: Both public v0.4.1 delivery forms and every qualification named by the prepared publication record must be verified before this ticket can claim completion.

Boundary: Do not change product behavior, HTTP or command-line contracts, software or dependency versions, scoring identity, scoring assets, release-note bodies, fixed-version fixtures, or historical publication evidence. Do not create, edit, delete, publish, retag, or replace any release, executable asset, container alias, staged leaf, or scoring asset.
