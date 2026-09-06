# PangoPup v0.4.0 publication record

State: **PREPARED — no v0.4.0 tag, release, or container alias exists.**

This document is the fail-closed runbook and final redacted evidence record for the application-only v0.4.0 release. The release publishes one immutable Linux x86-64 executable release and one native Linux AMD64/ARM64 OCI image index. It does not rebuild, upload, retag, delete, or otherwise change `snv-grch38-v1`, `runtime-grch38-v1`, or an earlier application release.

The GitHub release body uses the exact bytes from `planning/artifacts/057-release-notes.md`. The container contains only the executable, `LICENSE`, and `NOTICE`. Scoring assets remain separate and unchanged.

## Fixed predecessor state

Publication may start only while GitHub Latest remains immutable release ID `365425336`, tag `v0.3.0`, target commit `3a857f7def2c11ad9d9e38ed62b7204bf7d6b691`, and while GHCR `latest` remains OCI index `sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`. The Git tag `v0.4.0` and GHCR tags `0.4.0` and `v0.4.0` must all be absent.

The checked container workflow carries that exact predecessor index under `PREVIOUS_INDEX`. A changed predecessor is a stop condition. It does not grant permission to overwrite unexpected public state.

## Release inputs

Use these values from one clean publication-ready commit on `origin/main`:

```text
repository: genomoncology/pangopup
version: 0.4.0
tag: v0.4.0
image: ghcr.io/genomoncology/pangopup
release title: PangoPup v0.4.0
release body: planning/artifacts/057-release-notes.md
publication date (UTC): 2026-09-06
```

Ian Maurer explicitly authorized building, deploying, and pushing PangoPup v0.4.0 on 2026-09-06. Before the first remote mutation, bind this authorization to the exact 40-character publication commit selected from `origin/main` and retain evidence that names Ian Maurer, the authorization date, the exact commit, and this scope. Authorization for another commit does not carry forward.

Never write a token, request header, authenticated download URL, or credential path into this record. Use the existing authenticated `gh` client for coordinator mutations. Use fresh anonymous credentials for public verification.

## 1. Authenticate the publication commit and repository

Require a clean checkout at the exact 40-character commit on `origin/main`. Require no Git replacement refs. Require `make lint`, `make test`, and `make spec` to pass. Authenticate exact successful `ci` and native `container` workflow runs for that same commit. The container run must contain one successful native smoke job for AMD64 and one for ARM64.

Require public repository visibility, immutable releases, read-only default Actions permissions, disabled Actions pull-request approval, enabled Dependabot security updates, enabled secret scanning and push protection, no open secret alerts, and the two expected active main rulesets. Observe these settings. Do not change them during publication.

Require GitHub Latest and GHCR `latest` to match the fixed predecessor state. Require the v0.4.0 GitHub release and Git tag to be absent. Use an anonymous GHCR pull token plus `scripts/require-container-tag-absent.sh` to prove that `0.4.0` and `v0.4.0` are absent. An authorization error, malformed response, or registry failure does not prove absence.

Immediately before the first public effect, require the current UTC date to equal `2026-09-06`, the `date-released` value in `CITATION.cff`. If the dates differ, stop before mutation, update `CITATION.cff` and its bound checks, recommit, and repeat every commit-bound qualification for the new exact publication commit. Do not publish with a stale or future citation date.

## 2. Stage and qualify native container leaves

Dispatch `.github/workflows/publish-container.yml` on `main` with `mode=stage`, the exact publication commit, and an empty `stage_run_id`. Admit exactly one new successful run. Require one successful preflight job, successful native AMD64 and ARM64 stage jobs, and one successful receipt job.

Download the unique retained stage receipt through the Actions API. Verify the artifact archive against its API-reported SHA-256. Admit the receipt with `scripts/admit-container-stage-receipt.sh`. Require the exact commit, workflow commit, run ID, and two distinct leaf digests. Verify both leaves anonymously by digest. No user-facing v0.4.0 container tag may exist at this stage.

If staging fails, retain the untagged leaves and stop. Never reuse a failed or superseded stage run.

## 3. Build and admit the Linux executable

Dispatch `.github/workflows/package-linux.yml` on `main` with the exact publication commit. Admit exactly one new successful run and one successful package job. Download the unique `pangopup-linux-<commit>` artifact through the Actions API. Verify the ZIP against its API-reported SHA-256 before extraction into an absent private directory.

Run `scripts/qualify-linux-release.sh <release-directory> 0.4.0 <publication-commit>`. Require exactly six regular, single-link files: `LICENSE`, `NOTICE`, `pangopup-linux-x86_64`, `pangopup-linux-x86_64.cdx.json`, `pangopup-linux-x86_64.sha256`, and `release-manifest.json`. Require the manifest version and target commit, checksum, SBOM, executable version, notices, dynamic-library allowlist, and maximum GLIBC 2.39 check to pass.

## 4. Publish and verify the immutable executable release

Create one private draft named `PangoPup v0.4.0` with tag `v0.4.0`, exact target commit, and the byte-identical release-note body. Upload only the six admitted files. Compare every remote asset name, size, and SHA-256 with the held local inventory. Compare the fetched draft body byte-for-byte with the source file.

Immediately before publication, repeat the predecessor Latest checks, tag-absence check, draft target check, body comparison, and six-member inventory comparison.

After those private-draft checks and immediately before publishing the private draft, require the current UTC date to equal `2026-09-06`, the `date-released` value in `CITATION.cff`. If the date changed, stop before publication. Preserve the staged native leaves from the prior commit. Discard only the still-private draft through a separately reviewed safe recovery path. Update `CITATION.cff` and its bound checks, recommit, and repeat every commit-bound qualification for the new exact publication commit before creating a replacement private draft.

Publish the draft as non-prerelease and Latest. After publication, never delete or edit it automatically. Record any partial state and stop.

Verify through anonymous API reads and downloads that the release is public, immutable, non-draft, non-prerelease, Latest, targeted at the exact commit, and retains the exact title, body, and six-member inventory. Require the public tag ref to resolve to the exact commit.

## 5. Test the public installer

Use an absent private directory and a non-root user in a clean Ubuntu 24.04 container. Install through the exact tagged script:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.4.0/install.sh \
  | bash -s -- --version 0.4.0
```

Require clean stderr and `pangopup 0.4.0` from `--version` and `-V`. Verify the installed executable checksum against the public release. Use disposable copies of the retained compatible data and cache for `pangopup sync --offline`, `pangopup status`, representative lookup, model, cache, and HTTP checks. Never point uninstall tests at retained assets.

Verify code-only and full uninstall in separate disposable trees. Code-only removal must preserve data and cache. `uninstall --full --yes` must remove the executable and managed disposable roots without prompting.

## 6. Finalize and verify the public container index

Only after the executable release and public installer pass, dispatch `.github/workflows/publish-container.yml` with `mode=finalize`, the exact publication commit, and the authenticated successful stage run ID. The workflow must re-admit the receipt, repeat anonymous native qualification, prove both version tags absent, and prove GHCR `latest` still matches the fixed predecessor immediately before publication.

Require one successful receipt-loading job, successful native AMD64 and ARM64 public qualification jobs, and one successful manifest finalization job. Through fresh anonymous reads, require one OCI index with exactly Linux AMD64 and ARM64 children equal to the staged digests. Require source, exact revision, version `0.4.0`, and `GPL-3.0-only` annotations. Require `0.4.0`, `v0.4.0`, and `latest` to resolve to the same index digest. Run the native qualifier against the public AMD64 image on this host.

If finalization fails after executable publication, preserve the executable release and staged leaves. Do not delete, retag, or retry from another commit without a reviewed recovery.

## 7. Record final evidence

After every corresponding check passes, replace the prepared state with complete and append Ian Maurer's 2026-09-06 authorization bound to the exact publication commit, the verified UTC publication date, publication commit, CI run, container smoke run, package run, package artifact ID and digest, six executable member sizes and SHA-256 values, GitHub release ID, stage run and receipt artifact identities, native leaf digests, finalize run, OCI index digest, anonymous tag checks, installer qualification result, and isolated uninstall results.

Then update observed-current architecture and planning claims from public v0.3.0 to public v0.4.0 in a post-publication commit. Set the version gate's observed public version to 0.4.0. Keep historical v0.3.0 records and fixed-version fixtures unchanged.
