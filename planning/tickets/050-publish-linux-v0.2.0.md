# 050 — Publish the current Linux executable as v0.2.0

Status: complete

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
  `sync --progress` with unchanged final JSON, silent `sync --quiet`, one
  indexed SNV routed automatically to exact precomputed output and explicitly
  through `--model-only` to an exact checked model result, foreground service
  startup, `/livez`, `/readyz`, `/v1/status`, automatic and forced-model forms
  of that same SNV, and an automatic non-SNV modeled or SQLite-reused
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

Developer: independent developer `ticket050_implementation`.

- Advanced all eight workspace packages, `Cargo.lock`, CLI/spec identity, HTTP
  status identity, and container qualification to `0.2.0` while retaining
  historical `builder_version: 0.1.0` receipts, fixtures, causal source
  fingerprints, and the unrelated `serde_jcs` dependency version.
- Made the compact README and delivery architecture eventual-tag accurate for
  v0.2.0, retained v0.1.0 as the older immutable release, and added exact
  release notes plus a credential-free, fail-closed coordinator publication
  record with a syntax-checked runbook.
- Extended clean-package smoke and production/public-reuse qualification to
  cover focused help, visible monotonic progress, quiet JSON compatibility,
  explicit model-only scoring of an indexed SNV, and foreground HTTP
  health/status/SNV/model scoring without weakening the retained 1,000-SNV and
  automatic M09 oracles.
- Focused evidence: `cargo test --locked -p pangopup-cli` passed 59 tests with
  one retained-production test intentionally ignored; `bash
  tests/executable-delivery.sh` passed; `bash
  tests/production-release-qualification.sh` passed fresh and installed-reuse
  paths plus fail-closed mutations; shell syntax, Python compilation, Rust
  formatting, and coordinator-runbook syntax checks passed.
- The complete local gate also passed after the historical-provenance repair:
  `make lint`; `make test`; and `make spec` (268 passed, 7 skipped). The final
  release-runbook hardening test and `git diff --check` passed after adding
  exact ruleset/workflow/artifact authentication, held-descriptor upload
  checks, and credential-free public downloads of all five small members.
- Code-review remediation binds every release note, script, fixture, and
  qualification step to a private exact `git archive` of the target commit;
  adds an independently checked indexed-SNV model oracle and proves automatic
  lookup versus forced model routing in CLI and HTTP; includes the one final
  progress record in monotonic accounting; updates the FAQ for eventual
  v0.2.0; and makes anonymous curl ignore user configuration while checking
  release, tag, Latest, body, inventory, and member digests.
- The exact-commit archive also refuses any configured `git replace` refs and
  sets `GIT_NO_REPLACE_OBJECTS=1` for commit-object authentication and archive
  materialization; static mutation coverage rejects a replacement-enabled
  archive. Publication wording now distinguishes automatic M09 from forced
  indexed-SNV model scoring, and the tagged FAQ names v0.2.0 directly.
- The new indexed-SNV oracle is mechanically equal to the first frozen
  production container-oracle result and was independently reproduced
  byte-for-byte with the retained production runtime. After all five review
  remediations, `make lint`, `make test`, `make spec` (268 passed, 7 skipped),
  coordinator-runbook syntax, mutation tests, and `git diff --check` all pass.
- No workflow was dispatched; no GitHub release, tag, asset, draft, commit, or
  push was created.

## Adversarial Code Review

Reviewer: independent code reviewer `ticket050_code_review` — ACCEPT.

Review rejected mutable-checkout publication authority, a non-SNV model-only
check that could not prove lookup bypass, incomplete final-progress
monotonicity, stale FAQ guidance, and curl calls that did not disable user
configuration. Remediation moved all post-bind authority to a private exact
commit archive, qualified one indexed SNV as precomputed versus exact forced
model in CLI/HTTP, hardened completion accounting, updated the FAQ, and made
public checks explicitly anonymous.

Re-review then demonstrated that `git archive` can honor local replacement
objects. The runbook now rejects replacement refs and sets
`GIT_NO_REPLACE_OBJECTS=1` for object authentication and archive creation, with
mutation coverage. Two final tagged-document wording defects were corrected.
Final re-review accepted the complete diff with no remaining findings.

## External Effect Evidence

Coordinator: Codex.

- Publication-ready commit: `c50dd1399b10b8e85e140305c7bd68fe849f77dd`.
- Required CI gate: [run 30921327421](https://github.com/genomoncology/pangopup/actions/runs/30921327421), one successful `gate` job.
- Native container regression: [run 30921328259](https://github.com/genomoncology/pangopup/actions/runs/30921328259), successful AMD64 and ARM64 smoke jobs.
- Exact package workflow: [run 30921809944](https://github.com/genomoncology/pangopup/actions/runs/30921809944), one successful `package` job.
- Admitted GitHub Actions artifact ID: `8897538272`.
- Immutable public release ID `364960381`: [`v0.2.0`](https://github.com/genomoncology/pangopup/releases/tag/v0.2.0), non-prerelease and Latest.
- The reviewed prepublication clean-container qualification passed online sync,
  offline reuse, ready status, the 1,000-SNV oracle, automatic M09 inference,
  forced model scoring of an indexed SNV, focused help, progress/quiet output,
  SQLite reuse, and foreground HTTP health/status/scoring.
- Anonymous public metadata, tag, Latest, body, inventory, and all five small
  member downloads matched the held local release. The tagged public installer
  installed `0.2.0`, reused the qualified assets, and passed the same applicable
  CLI/model/cache/HTTP qualification.

## Coordinator Final Check

Coordinator: final `make lint`, `make test`, and `make spec` passed (268 passed,
7 skipped), but the stale-claim audit found `architecture/delivery.md` still
describing the retired model-only M09 check instead of forced indexed-SNV
scoring. That documentation finding returned to the same developer and code
reviewer. The corrected automatic-M09/forced-indexed-SNV wording was accepted
on re-review; the focused final stale scan and `git diff --check` pass. The
reviewed preparation was committed and pushed. Its exact remote gate, package
workflow, local admission, clean production qualification, immutable GitHub
publication, anonymous public verification, and tagged-installer reuse
qualification all passed. Ticket 050 is complete.
