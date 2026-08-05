# 055 — Publish the exact v0.3.0 application release

Status: complete

## Why

The reviewed 0.3.0 candidate is on `main`, but users still receive v0.2.0 from
the tagged installer and GHCR. This ticket publishes one exact source commit as
both the six-file Linux x86-64 executable release and the native AMD64/ARM64
thin container index, then anonymously verifies the public bytes. It changes no
biological asset and does not republish upstream inputs.

## Scope

- Add `planning/artifacts/055-public-v0.3.0.md` containing a credential-free,
  fail-closed coordinator runbook and the eventual redacted effect evidence.
- Before the publication-ready commit, make `README.md` and
  `planning/artifacts/054-release-notes.md` timeless final-release documents.
  The tagged README must give working pinned v0.3.0 commands without calling
  the tree a candidate or saying publication is pending. The release body must
  be taken byte-for-byte from the final release-notes file and contain no
  pending-publication boundary.
- Select the exact publication-ready commit only after implementation and code
  review. The executable release tag, executable manifest target, OCI revision
  labels, and both OCI leaves/index must all bind that same full commit.
- Before any public effect, require:
  - clean local checkout with no Git replacements and `origin/main` at the
    exact commit;
  - public repository and anonymous exact-digest reads from the existing GHCR
    package without relying on package-settings API scope;
  - absent Git tag/release `v0.3.0` and absent container tags `0.3.0` and
    `v0.3.0`;
  - GitHub's current Latest release is exactly the known immutable `v0.2.0`
    release and GHCR `latest` resolves to the known v0.2.0 index before either
    moving alias is replaced;
  - successful exact-commit `ci/gate` and both native jobs from
    `.github/workflows/container.yml`;
  - active authenticated `gh`, `jq`, Docker/Buildx, and the reviewed workflows.
- Dispatch `.github/workflows/package-linux.yml` for the exact commit after
  snapshotting existing run IDs. Require exactly one new workflow-dispatch run
  with the expected workflow path, head SHA, successful status/jobs, and then
  admit exactly one unexpired artifact by API ID and digest rather than a
  latest-run or filename guess. Re-run the checked release qualifier
  locally and verify the closed six-member inventory, manifest version/commit,
  checksums, SBOM, license, notice, binary `--version`, and GLIBC baseline.
- Dispatch `publish-container.yml` in `stage` mode for the same commit before
  creating user-facing container tags. Authenticate the unique successful
  stage run and canonical two-leaf receipt; prove both digest-only leaves are
  anonymously readable at the exact requested digest and natively qualified.
  The existing package is public, so no visibility mutation is expected or
  authorized; anonymous registry reads, not the authenticated package-settings
  API, prove that boundary.
- Create a draft GitHub release `v0.3.0` targeting the exact commit, upload only
  the six admitted executable members, compare every remote name, size, and
  digest, then publish it as non-prerelease/Latest and verify its immutable
  state through unauthenticated API/download reads. The release body is
  `planning/artifacts/054-release-notes.md`.
- Qualify the immutable tagged curl installer from a clean non-root Linux
  environment while reusing the retained compatible assets:

  ```bash
  curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh \
    | bash -s -- --version 0.3.0
  ```

  It must pass version/help, offline asset reuse, ready status, the retained
  SNV oracle, automatic and forced model paths, persistent SQLite reuse, and
  HTTP health/status/SNV/model checks.
- Qualify `pangopup uninstall` in disposable isolated paths: interactive
  code-only removal must show all resolved paths, remove only the executable,
  and preserve data/cache; `uninstall --full --yes` must remove the executable,
  data, and cache without prompting. Never point these checks at retained
  production assets.
- Dispatch `publish-container.yml` in `finalize` mode with only the
  authenticated stage run ID. Verify the public OCI index contains exactly
  `linux/amd64` and `linux/arm64`, exact staged leaf digests, version/source/
  revision/license annotations, and that tags `0.3.0`, `v0.3.0`, and `latest`
  resolve to that index. Repeat anonymous native qualification.
- Harden container finalization so it fails closed unless GHCR `latest` still
  resolves to the reviewed v0.2.0 predecessor at the last practical point
  before replacement. The coordinator likewise rechecks GitHub Latest
  immediately before publishing the executable release.
- Record release ID/tag/target, package and stage/finalize run IDs, artifact
  identity, exact six-file sizes/digests, OCI index and leaf digests, anonymous
  qualification, and final remote gates in the retained artifact. Do not record
  tokens, authenticated URLs, headers, or local credential paths.
- After public success, update only the retained publication evidence and any
  rolling planning status that cannot truthfully be finalized before the
  effect. User-facing README and immutable release notes are already final in
  the tagged commit.
- Do not rebuild, upload, retag, delete, or mutate `snv-grch38-v1`,
  `runtime-grch38-v1`, `v0.1.0`, `v0.2.0`, or their historical container tags.

## Success Checklist

- One exact commit is the target of public tag/release `v0.3.0`, all six
  executable manifest fields, and every 0.3.0 OCI revision label.
- GitHub release `v0.3.0` is public, immutable, non-prerelease, Latest, and has
  exactly the reviewed six assets with matching unauthenticated sizes/digests.
- The pinned public installer installs `pangopup 0.3.0` and the complete clean
  qualification passes without redownloading or rebuilding biological assets.
- Anonymous GHCR inspection sees one exact two-leaf index; `0.3.0`, `v0.3.0`,
  and `latest` all resolve to it and each native leaf passes qualification.
- Public executable and container provenance use the same source commit,
  closing the v0.2.0 two-commit ambiguity.
- The retained publication record contains enough redacted evidence to audit
  every effect and partial-failure boundary.
- Final public README no longer says v0.3.0 publication is pending and gives
  working pinned install/container commands.
- The README read from tag `v0.3.0` and the immutable public release body
  contain no candidate/pending claims; the remote body matches the reviewed
  release-notes bytes exactly.
- Disposable code-only and `--full --yes` uninstall qualification proves the
  documented removal boundaries without modifying retained assets.
- `make lint`, `make test`, and `make spec` pass before the publication-ready
  commit; the exact commit's remote `ci` and `container` gates are green.

## Decisions

1. **One exact commit for both delivery forms.** A common source identity is
   more valuable than publishing executable and container from nearby commits.
2. **Stage container leaves before human tags.** Digest-only leaves permit
   native qualification and receipt authentication without exposing a partial
   0.3.0 index. Finalization alone creates version/moving tags.
3. **Draft executable release before publication.** A mutable private draft is
   the correction window. Every member is compared remotely before the
   irreversible public/immutable transition.
4. **Reuse qualified biological assets.** v0.3.0 changes application code and
   documentation only. Rebuilding or republishing the 15 GB index or runtime
   tuple would add risk without changing the product contract.
5. **No automatic rollback by deletion.** If a public effect partially
   succeeds, stop and record the exact state. Do not delete a public release,
   tag, or registry object without a separately reviewed recovery plan.
6. **Executable release becomes public before the container aliases.** The
   pinned installer is the narrowest independently verifiable delivery form.
   After it is published and qualified, container finalization replaces the
   three OCI aliases. If container finalization fails, stop with v0.3.0
   executable available and the staged digest-only leaves retained; do not
   delete or republish either system. If executable publication fails, do not
   finalize container aliases.

## Dependencies

Ticket 054.

## Notes

- Current `main` is the cleaned Ticket 054 result at `e60a5ea`; the exact
  publication commit will be the later reviewed `publication-ready` commit
  containing this ticket's runbook.
- Reuse the proven release lifecycle in
  `planning/artifacts/050-public-linux-release.md` and container lifecycle in
  `planning/artifacts/051-public-container.md`, replacing every historical
  literal deliberately rather than editing those records.
- The retained qualified installation/cache is under
  `/home/ian/workspace/data/pangopup-release-050-c50dd13/`. It may be reused
  read-only for post-publication qualification; do not synchronize 17 GB again.
- The coordinator alone may dispatch workflows, create/publish a release, or
  finalize container tags. Developer and reviewers prepare and inspect only.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05. Drafted from the completed v0.3.0
candidate and the established v0.2.0 executable/container publication records.

## Independent Ticket Review

Reviewer: Codex subagent `/root/ticket055_design_review`, 2026-08-05.

Initial verdict: REJECT. The reviewer found that the proposed tag and immutable
release body would retain candidate/pending wording, moving `latest` aliases
lacked known-predecessor guards, uninstall was absent from release
qualification, package-run admission needed exact identity, and cross-system
partial-publication order was implicit.

Resolution: the ticket now requires timeless final documents in the tagged
commit, byte-identical release notes, fail-closed v0.2.0 predecessor checks,
isolated code-only/full uninstall checks, exact package run/artifact admission,
and executable-first public effects with an explicit stop boundary.

Final verdict: ACCEPT.

Post-approval amendment: the first coordinator preflight stopped safely before
effects because the package-settings API required `read:packages`, which the
otherwise sufficient release identity did not carry. The same reviewer
accepted replacing that scope-dependent assertion with stronger anonymous
registry evidence: a fresh credential-free Docker configuration and anonymous
GHCR pull token must authenticate the exact v0.2.0 `latest` digest and both
exact staged receipt digests. No visibility mutation or additional OAuth scope
is introduced.

## Implementation Evidence

Developer: Codex subagent `/root/ticket055_implementation`, 2026-08-05.

- Added the credential-free, fail-closed coordinator runbook and empty effect
  ledger at `planning/artifacts/055-public-v0.3.0.md`. It fixes the known
  v0.2.0 GitHub Latest release and GHCR index identities, orders container
  staging before executable publication and container finalization afterward,
  authenticates exact runs/artifacts/digests, qualifies the public installer
  against disposable copies of retained assets, and isolates both uninstall
  scopes from those retained assets.
- Made the tagged README and release body timeless v0.3.0 documents. The README
  now gives immediately usable pinned executable/source/container commands;
  the release notes contain an immutable installer command and container name
  and no candidate or pending-publication boundary.
- Added `scripts/require-container-tag-digest.sh` and integrated it immediately
  before `imagetools create`. Finalization now fails unless GHCR `latest`
  returns HTTP 200 with exactly one valid `Docker-Content-Digest` equal to the
  reviewed v0.2.0 index. The parser treats header names case-insensitively
  without relying on nonportable `awk` `IGNORECASE` behavior.
- Added positive, lowercase-header, changed-predecessor, HTTP failure, missing,
  duplicate, malformed, symlink, and invalid-expected-digest tests. Extended
  static container and executable-delivery checks and updated the README spec
  from candidate wording to immutable release wording.
- Corrected one pre-existing stale static assertion so it checks the exact
  retained Ticket 051 superseded-run sentence rather than a sentence that is
  not present in that immutable record.
- After code review identified a tag-creation race, made tag-ref absence a
  second adjacent prepublication check between the final draft validation and
  publish request; the executable-delivery test pins that ordering.
- After the first effect-free coordinator preflight exposed an unnecessary
  `read:packages` dependency, removed the package-settings API assertion. The
  runbook now uses a fresh Docker configuration with no registry credentials
  plus an anonymous GHCR pull token, and requires exact HTTP/digest proof for
  both the known v0.2.0 `latest` index and each staged receipt leaf.

Focused evidence:

```text
bash tests/container-tag-digest.sh                         PASS
bash tests/container-delivery.sh                           PASS
bash tests/executable-delivery.sh                          PASS
PATH="$PWD/target/debug:$PATH" mustmatch test spec/readme-first-use.md
                                                           10 passed
anonymous GHCR token + fresh Docker config `latest` probe  HTTP 200, exact
                                                           v0.2.0 digest
git diff --check                                           PASS
```

No workflow was dispatched; no GitHub release, tag, package, image, registry
alias, biological asset, or retained qualification directory was changed.

## Adversarial Code Review

Reviewer: Codex subagent `/root/ticket055_code_review`, 2026-08-05.

Initial verdict: REJECT. The publication runbook checked tag absence only in
the initial preflight, leaving a race in which another actor could create
`v0.3.0` before the draft became public.

Resolution: the developer added a last-practical-point tag-ref absence check,
followed by re-authentication of the draft target/body/inventory and the
immediate publish request. Static qualification pins that order.

Prior verdict: ACCEPT. The reviewer also confirmed the GHCR predecessor guard
is fail-closed and immediately precedes index creation; all focused delivery
tests passed.

The later anonymous-registry amendment materially changed the reviewed
preflight and staged-leaf qualification. The same reviewer inspected the
complete amended diff and returned ACCEPT: the credential-free token/read
boundary, exact digest checks, and static regression fully resolve the failed
scope-dependent preflight.

## External Effect Evidence

Coordinator: Codex (`/root`), 2026-08-05.

- Published immutable GitHub release `v0.3.0` (ID `365425336`) from exact
  commit `3a857f7def2c11ad9d9e38ed62b7204bf7d6b691`, with exactly six admitted
  members and byte-identical reviewed release notes. Anonymous API and all six
  downloads matched names, sizes, and SHA-256 digests.
- Exact green CI/native runs were `30993530989` and `30993531181`. Package run
  `30994324171` produced admitted artifact `8925565070` with digest
  `sha256:e1dc222db3180359b2dba203a5c0dc4a851df32b7511e049feb357c88072b244`.
- The pinned curl installer passed in clean Ubuntu 24.04 as a non-root user.
  Disposable retained-asset copies passed all 1,000 SNV oracles, model/model-
  only, SQLite reuse, offline status, focused help, and HTTP qualification.
  Isolated interactive code-only and noninteractive `--full --yes` uninstall
  checks passed without targeting retained assets.
- Container stage run `30993928539` produced canonical receipt artifact
  `8925408153`; finalize run `30995022437` published exact staged AMD64/ARM64
  leaves in OCI index
  `sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`.
  Anonymous reads proved `0.3.0`, `v0.3.0`, and `latest` resolve to that exact
  two-leaf index with the reviewed annotations.
- The initial effect-free preflight stopped on an unnecessary `read:packages`
  dependency. No effect occurred. The independently reviewed anonymous-
  registry amendment was committed, gated, and used successfully without
  broadening credentials.

Full redacted identities and file digests are retained in
`planning/artifacts/055-public-v0.3.0.md`; local evidence is under
`/home/ian/workspace/data/pangopup-release-055-3a857f7/` and contains no saved
token or authenticated URL.

## Coordinator Final Check

Coordinator: Codex (`/root`), 2026-08-05 — the prior publication-ready
conclusion was superseded when its package-settings preflight assumption failed
safely before any effect. After independent amendment review, the coordinator
inspected the complete diff and reran `make lint`, `make test`, and `make spec`;
all passed, with 276 specifications passed and 7 intentionally skipped. The
amended preparation changes only the credential-free publication proof and
tests, performs no public effect, and is ready to become the new exact
publication commit. Its own remote CI/native gates and complete preflight must
pass before publication.

Post-effect final check: every reviewed preflight and effect boundary passed;
the public executable and container use the same exact commit, the immutable
biological assets were neither rebuilt nor republished, and rolling planning
now records independent public qualification as the next outcome. Final local
gates are rerun before committing this evidence.
