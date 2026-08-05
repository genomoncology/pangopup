# 056 — Independently qualify the public v0.3.0 release

Status: ready

## Why

Ticket 055 published and coordinator-qualified v0.3.0. A separate read-only
qualification should now prove that a user who knows only the public release
identities can install and run it. This closes the release plan without
rebuilding assets, republishing anything, or trusting Ticket 055's local
summary as the oracle.

## Scope

- Add `planning/artifacts/056-independent-public-v0.3.0.md` with concise,
  credential-free evidence and an explicit pass/fail verdict.
- Clone the public repository anonymously into a new directory, check out tag
  `v0.3.0`, prove it resolves to exact commit
  `3a857f7def2c11ad9d9e38ed62b7204bf7d6b691`, and use only that checkout's
  qualification scripts, fixtures, and release-notes bytes as the oracle.
- Authenticate public GitHub release `v0.3.0` through anonymous API/downloads:
  exact target commit
  `3a857f7def2c11ad9d9e38ed62b7204bf7d6b691`, immutable/non-draft/non-
  prerelease/Latest state, reviewed release body, and exactly six members whose
  names, sizes, and SHA-256 values independently match the manifest/checksum.
  Run the tagged checkout's Linux release qualifier and require the installed
  executable SHA-256 to equal the admitted public executable.
- Run the pinned curl installer in a fresh Ubuntu 24.04 container as a non-root
  user. Require `pangopup --version` and `pangopup -V` to return 0.3.0 and
  focused help to succeed.
- Authenticate GHCR anonymously without existing Docker credentials. Require
  `0.3.0`, `v0.3.0`, and `latest` to resolve to index
  `sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`,
  with exactly native Linux AMD64/ARM64 children, the published source/version/
  license annotations, and exact publication-commit revision. Run the full
  container qualifier on the host-native public leaf.
- Fingerprint the preserved Ticket 055 data and cache before testing, then copy
  them into new Ticket 056 disposable directories (reflink only if the
  filesystem actually supports it; otherwise make a real copy). Run no
  qualification command against the preserved roots. Fingerprint them again
  afterward and require exact equality. Never download, rebuild, certify, or
  modify authoritative biological asset releases. With the public executable
  and only the copies, repeat offline ready status, the retained
  1,000-SNV oracle, automatic non-SNV and forced-model scoring, persistent
  SQLite reuse, and HTTP health/status/SNV/model behavior.
- Prove persistent SQLite reuse separately: start with a new empty model-result
  cache, run one supported uncached model request, authenticate a resulting
  SQLite entry, then issue the byte-identical request from a separate process.
  Require identical output, unchanged database bytes, and a bounded cache-hit
  observation clearly distinct from the uncached inference observation.
- Repeat code-only and `uninstall --full --yes` behavior only in new disposable
  paths. Prove displayed paths and preservation/removal boundaries; never use
  the preserved qualification data as an uninstall target. Put sentinels
  immediately outside each managed executable/data/cache path and require them
  unchanged after both scopes.
- Report product failures separately from environmental/tooling failures. Do
  not change releases, tags, images, aliases, repository settings, or assets.

## Success Checklist

- Anonymous public executable and OCI identities match the fixed v0.3.0
  contract exactly.
- A clean non-root pinned install and host-native public container both run
  version/help successfully.
- Offline lookup/model/cache/HTTP behavior reproduces the retained exact
  oracles without network synchronization.
- Both uninstall scopes pass in isolated paths and preserved assets remain
  byte-for-byte fingerprint-identical.
- The retained report states limitations honestly, including that one Linux
  host cannot independently execute the non-native leaf and that GitHub's
  native ARM64 qualification is separate evidence.
- `make lint`, `make test`, and `make spec` remain green; no product code is
  changed unless the independent audit finds a separately reviewed defect.

## Exclusions

- No new release, retag, upload, deletion, visibility change, or asset sync.
- No model/index/reference/mask rebuild or full corpus scan.
- No performance claim beyond incidental smoke observations.
- No Apple MPS/Metal or cross-platform expansion.

## Dependencies

Ticket 055 and public v0.3.0.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05.

## Independent Ticket Review

Reviewer: Codex subagent `/root/ticket056_design_review`, 2026-08-05.

Initial verdict: REJECT. The reviewer found that qualification could mutate
preserved assets, oracle independence was underspecified, SQLite reuse was not
distinguished from repeat inference, and uninstall lacked outside sentinels.

Resolution: require an anonymous exact-tag checkout as the sole oracle, copied
disposable assets with before/after source fingerprints, explicit fresh-cache
and separate-process reuse evidence, public binary digest equality, and
outside sentinels for every uninstall root.

Final verdict: ACCEPT.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## Coordinator Final Check

Coordinator: pending
