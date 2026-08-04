# 050 — Publish the current Linux executable as v0.2.0

Status: ready

## Why

The public curl installer still resolves to immutable `v0.1.0`, which predates
explicit model-only scoring, the foreground HTTP service, the non-root Docker
delivery contract, read-only status, focused help, and resilient observable
synchronization. Current `main` is qualified but ordinary Linux users cannot
receive those outcomes through the documented installer.

## Scope

- Advance the workspace and user-visible executable version from `0.1.0` to
  `0.2.0`, updating only version-bound code, fixtures, specs, packaging tests,
  and current-state delivery documentation. This minor version communicates
  additive user-facing CLI/service capabilities without changing asset formats
  or scoring semantics.
- Add compact exact release notes at `planning/artifacts/050-release-notes.md`
  covering the changes since v0.1.0, Linux x86-64/GLIBC prerequisites, install,
  sync, CLI, HTTP, Apple/Docker boundary, assets, and licenses.
- Add a credential-free publication/qualification record and syntax-checked
  coordinator runbook at `planning/artifacts/050-public-linux-release.md`,
  adapted from the proven Ticket 038 lifecycle.
- Prepare exactly the existing six-file Linux x86-64 inventory using the
  read-only `.github/workflows/package-linux.yml`; do not add formats or assets.
- Before publication, require the exact publication-ready commit's successful
  `ci/gate`, successful package workflow, local six-file admission, and fresh
  pinned-container production qualification: online sync, offline reuse,
  combined status, the 1,000-SNV oracle, exact M09 automatic inference,
  explicit model-only scoring, focused help, sync progress/quiet behavior, and
  the foreground HTTP scoring/health surface.
- Recheck repository publication-security controls and the absence of `v0.2.0`,
  create one private draft targeting the exact commit, upload every held member
  once, compare complete remote names/sizes/SHA-256 digests and release body,
  then publish once as immutable Latest only after every private check passes.
- After successful publication, qualify the public unauthenticated release and
  tagged curl installer in a fresh pinned container and record non-sensitive
  evidence. Public verification occurs after the irreversible publication; a
  failure is recorded and stops work without modifying the immutable release.
- Make the publication-ready README and `architecture/delivery.md` accurate for
  the eventual immutable `v0.2.0` tag before publication: v0.2.0 is the ordinary
  curl-installed feature set, while v0.1.0 remains an older immutable release.
  Post-publication documentation changes only record evidence/state that could
  not exist earlier; the tagged source must never permanently describe v0.2.0
  as planned or require current-main source for its shipped features.
- Update `planning/frontier.md` after publication, mark the ticket complete,
  and clean the live ticket.
- If any prepublication check fails, stop without publication. A failed private
  draft may be deleted only when its exact release ID is authenticated and the
  tag is still absent. Never modify or delete an existing published release or
  tag, and never retry an upload over an existing member.
- Do not publish a container, ARM64 executable, raw upstream data, rebuilt
  runtime data, changed model, changed SNV index, or custom ONNX Runtime.

## Success Checklist

- `pangopup --version` and version-bound tests report `pangopup 0.2.0`; Cargo
  lock/workspace package versions are coherent.
- The exact publication-ready commit passes `make lint`, `make test`, and
  `make spec`, and its remote `ci` workflow has one successful `gate` job.
- The package workflow produces exactly `LICENSE`, `NOTICE`,
  `pangopup-linux-x86_64`, its checksum, CycloneDX SBOM, and canonical release
  manifest for version 0.2.0 and the exact target commit.
- Local admission proves the six files, checksum, manifest, imported libraries,
  maximum GLIBC 2.39, exact version, exact commit, and clean-container smoke.
- Fresh prepublication production qualification reproduces the retained 1,000
  SNVs and exact M09 result after online sync and offline reuse. The exact
  packaged executable also proves all focused help leaves, visible monotonic
  `sync --progress` with unchanged final JSON, silent `sync --quiet`, one exact
  `--model-only` result, foreground service startup, `/livez`, `/readyz`,
  `/v1/status`, a precomputed `/v1/score`, and a modeled or SQLite-reused
  `/v1/score`.
- GitHub release `v0.2.0` targets the reviewed commit, has the exact reviewed
  body and six-member inventory, is public, immutable, non-prerelease, and
  Latest; its tag resolves directly to the target commit.
- Immediately after immutable publication, a network-unauthenticated verifier
  confirms public metadata and downloads; the tagged
  `install.sh --version 0.2.0` path installs, reports the version, reuses
  qualified assets offline, and repeats the focused help, sync, SNV, automatic
  model, model-only, and HTTP checks in a fresh pinned container.
- The tagged compact README presents curl-installed v0.2.0 capabilities as the
  ordinary path without claiming a registry image or Apple MPS support.
- No credential, authorization header, signed URL, environment dump, or private
  path enters Git history.

## Decisions

1. **Version:** patch or minor. Use `0.2.0`: lookup scoring remains compatible,
   but model-only routing, HTTP service, Docker delivery, help, status, and sync
   behavior are a meaningful additive public surface.
2. **Inventory:** add convenience archives or retain the proven direct binary.
   Retain the exact six-file inventory so the existing checksum installer,
   qualification, SBOM, and immutable-release boundary remain unchanged.
3. **Assets:** republish data with the executable or keep immutable asset
   families independent. Keep `snv-grch38-v1` and `runtime-grch38-v1` unchanged;
   v0.2.0 pins and synchronizes them but does not mirror or rebuild them.
4. **Publication:** direct public creation or private draft. Use one checked
   private draft and fail closed before the irreversible immutable publication.
5. **Latest:** preserve v0.1.0 as Latest or advance it. Make v0.2.0 Latest only
   in the single immutable publication after every private qualification. Then
   immediately verify the public/tagged path; because publication is
   irreversible, any public-verification failure is recorded and work stops
   rather than mutating or deleting the release.
6. **Release acceptance:** reuse only the old SNV/M09 checks or exercise the
   new public surface. Extend the checked qualification helpers and their tests
   to cover focused help, progress/quiet, model-only, and HTTP so v0.2.0 is not
   published on evidence that proves only v0.1.0 behavior.

## Dependencies

- Ticket 049 is complete and documents the temporary release distinction.
- Immutable public `snv-grch38-v1` and `runtime-grch38-v1` remain available.
- GitHub repository/security controls and the exact commit's remote gate must be
  green before the coordinator performs any publication effect.

## Notes

- Only the coordinator may dispatch the packaging workflow, create/upload/
  publish the release, or perform cleanup of a failed private draft.
- The publication-ready implementation commit uses eventual-tag wording: it
  describes v0.2.0's exact shipped behavior and v0.1.0 as the prior release,
  avoiding a stale immutable tag. Post-publication edits add only observed URLs,
  digests, run IDs, and completion/frontier state.
- `v0.1.0` is immutable and remains available. The installer `--version` value
  omits the `v` prefix; the tag includes it.
- The Linux executable remains x86-64/amd64 only. Native ARM64 is the following
  container-publication ticket, not an expansion of this binary release.

## Coordinator Authorship

Coordinator: Codex. Drafted from the shipped Ticket 049 outcome, the proven
Ticket 038 release lifecycle, the current package workflow, and live public
release inventory.

## Independent Ticket Review

Reviewer: independent design reviewer `ticket050_design_review` — ACCEPT.

Initial review rejected a permanently stale tagged README, a contradictory
Latest/public-verification order, and qualification that proved only v0.1.0
behavior. The coordinator revised the ticket so the publication-ready README is
tag-accurate, immutable Latest publication occurs once after all private gates,
public failure can only stop and be recorded, and both packaged/public
qualification cover focused help, progress/quiet sync, model-only, and HTTP in
addition to the retained SNV/M09 oracles. Re-review accepted all findings.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: pending. This ticket has a reviewed public irreversible effect and
must stop at `publication-ready` until the exact commit is pushed, remotely
green, freshly audited, packaged, admitted, and production-qualified.

## Coordinator Final Check

Coordinator: pending
