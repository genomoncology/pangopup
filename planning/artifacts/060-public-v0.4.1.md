# PangoPup v0.4.1 publication record

State: **PREPARED — no v0.4.1 tag, release, or container alias exists.**

This fail-closed runbook prepares one immutable Linux x86-64 executable release and one native Linux AMD64/ARM64 OCI index from one exact reviewed commit. It does not rebuild, upload, retag, delete, or change either scoring asset release, any earlier application release, the public v0.3.0 container index, or the abandoned v0.4.0 native leaves.

The GitHub release body uses the exact bytes from `planning/artifacts/059-release-notes.md`. The release contains exactly six executable files. The container contains only the executable, `LICENSE`, and `NOTICE`.

## Fixed split predecessor state

Before executable publication, require GitHub Latest to remain immutable release ID `383614742`, tag `v0.4.0`, title `PangoPup v0.4.0`, and target commit `ea4438e50762e32f09052b364060c89201ed78bc`. Require the direct tag ref to resolve to that commit. After executable publication, container finalization requires GitHub Latest to be the immutable v0.4.1 release at the selected publication commit. Before every public effect, require GHCR `latest`, `0.3.0`, and `v0.3.0` to remain OCI index `sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`.

Require the v0.4.1 GitHub release and Git tag to be absent. Require GHCR `0.4.1` and `v0.4.1` to return the canonical anonymous `MANIFEST_UNKNOWN` response. The absent historical `0.4.0` and `v0.4.0` container aliases are recorded in `planning/artifacts/058-public-v0.4.0.md`; they are not publication targets for this release. The checked container workflow keeps the fixed v0.3.0 index under `PREVIOUS_INDEX`.

## Release inputs and authority

```text
repository: genomoncology/pangopup
version: 0.4.1
tag: v0.4.1
image: ghcr.io/genomoncology/pangopup
release title: PangoPup v0.4.1
release body: planning/artifacts/059-release-notes.md
publication date (UTC): 2026-09-06
```

Ian Maurer explicitly authorized building, deploying, and pushing PangoPup v0.4.1 on 2026-09-06. The final publication commit cannot name itself inside this committed runbook. After the final commit reaches `origin/main` and before the first remote mutation, obtain and retain an out-of-commit authorization receipt or user message from Ian Maurer. It must name the exact 40-character `origin/main` hash, authorization date, v0.4.1, and both executable and container publication. A replacement commit requires a new receipt before any public effect. The COMPLETE record must name the exact authorized commit and retained authorization evidence after publication.

The UTC publication date must equal `CITATION.cff` immediately before the first remote mutation and again immediately before publishing the private executable draft. A changed date stops publication. Update the citation date and its checks, recommit, bind authorization to the replacement commit, and repeat all commit-bound qualification.

Never put a token, request header, authenticated download URL, or credential path in this record. Use the existing authenticated `gh` client only for coordinator mutations. Use fresh anonymous credentials for public verification.

## 1. Authenticate source and gates

Require a clean checkout at the selected exact commit on `origin/main`, no Git replacement refs, and the expected repository origin. Require `make lint`, `make test`, and `make spec` to pass. Authenticate exact successful `ci` and native `container` workflow runs for the same commit. The container run must contain successful native AMD64 and ARM64 smoke jobs.

Observe public repository visibility, immutable releases, read-only default Actions permissions, disabled Actions pull-request approval, enabled Dependabot security updates, enabled secret scanning and push protection, no open secret alerts, and the expected active main rulesets. Do not change repository settings during publication.

Recheck the complete split predecessor state, v0.4.1 release and tag absence, both v0.4.1 container-tag absences, and the UTC citation date immediately before the first remote mutation. Any ambiguous, unauthorized, malformed, or failed response is a stop condition and never proves absence.

## 2. Stage and admit native container leaves

Dispatch `.github/workflows/publish-container.yml` on current `main` with `mode=stage`, the exact publication commit, and an empty `stage_run_id`. Admit exactly one new successful workflow-dispatch run with one successful preflight, native AMD64 and ARM64 stage jobs, and aggregate receipt job.

Download the unique unexpired receipt artifact through the Actions API and verify its API-reported SHA-256 before extraction. Admit the canonical receipt with `scripts/admit-container-stage-receipt.sh`. Require its commit and workflow commit to equal the publication commit, its run ID to equal the authenticated stage run, and its AMD64 and ARM64 leaf digests to be distinct. Verify each leaf anonymously by exact digest and qualify it natively. Require every user-facing v0.4.1 container alias to remain absent. A failed or superseded stage and its untagged leaves must not be reused.

## 3. Build and admit the executable

Dispatch `.github/workflows/package-linux.yml` on current `main` with the exact publication commit. Admit exactly one new successful run and package job. Download the unique unexpired `pangopup-linux-<commit>` artifact through its API ID. Verify the archive against the API-reported SHA-256 before extraction into an absent private directory.

Run `scripts/qualify-linux-release.sh <release-directory> 0.4.1 <publication-commit>`. Require exactly six regular single-link files and retain each name, size, and SHA-256:

```text
LICENSE
NOTICE
pangopup-linux-x86_64
pangopup-linux-x86_64.cdx.json
pangopup-linux-x86_64.sha256
release-manifest.json
```

Require the manifest version and target commit, checksum, SBOM, executable version, notices, dynamic-library allowlist, and maximum GLIBC 2.39 check to pass.

## 4. Create, verify, and publish the executable release

Create one private draft titled `PangoPup v0.4.1`, tagged `v0.4.1`, targeted at the publication commit, and carrying the byte-identical `planning/artifacts/059-release-notes.md` body. Upload only the six admitted files. After every upload and after the final upload, compare every remote asset name, size, and SHA-256 with the held local inventory. Compare the fetched draft title, target, tag, prerelease state, and body with the admitted inputs.

If the date changed, preserve the staged leaves from the prior commit. Discard only the still-private authenticated draft through a separately reviewed safe path. Update the date and bound checks, recommit, bind authorization to the replacement commit, and repeat every commit-bound qualification before creating another draft.

PRE-EXECUTABLE-PUBLISH GATE: Immediately repeat the GitHub v0.4.0 Latest identity and direct-tag checks, the GHCR v0.3.0 `latest` digest check, v0.4.1 release/tag absence outside the authenticated draft, both v0.4.1 container-tag absence checks, private draft target/title/body/six-member inventory checks, and the UTC/CITATION date check.

EXECUTABLE-PUBLISH ACTION: Publish the draft as non-prerelease and Latest. After publication, never delete or edit it automatically. Through fresh anonymous API reads and downloads, require one immutable, Latest, non-draft, non-prerelease v0.4.1 release with the exact title, target commit, direct tag resolution, byte-identical body, and six admitted member names, sizes, and SHA-256 values.

## 5. Qualify the public installer and uninstall

Use an absent private directory and non-root user in clean Ubuntu 24.04. Install through the exact tagged script and require clean stderr plus `pangopup 0.4.1` from `--version` and `-V`. Verify the executable checksum against the public release. Use disposable copies of the retained compatible assets and cache for offline sync, status, representative lookup, model, cache, and HTTP checks.

Verify code-only and full uninstall in separate disposable trees. Code-only removal must preserve data and cache. `pangopup uninstall --full --yes` must remove a normal read-only managed profile, cache, and executable without prompting. Never point uninstall at retained original assets. A failed installer or uninstall check stops before container finalization.

## 6. Finalize and verify the container index

Only after the public executable and installer checks pass, dispatch `.github/workflows/publish-container.yml` with `mode=finalize`, the exact publication commit, and the authenticated successful stage run ID. Finalize mode must authenticate the current workflow event, workflow source, and `origin/main` independently from the staged release commit. It must then authenticate the immutable public v0.4.1 release and direct tag as the exact staged publication commit.

Re-admit the exact retained receipt and repeat anonymous native qualification of both held leaves.

PRE-INDEX-CREATION GATE: Immediately recheck the exact retained receipt, GitHub Latest v0.4.1 and its direct tag at the publication commit, the unchanged GHCR v0.3.0 predecessor under `latest`, and absence of GHCR `0.4.1` and `v0.4.1`.

INDEX-CREATION ACTION: Create one OCI index from exactly the held AMD64 and ARM64 digests. Require source, exact revision, version `0.4.1`, and `GPL-3.0-only` annotations.

Through fresh anonymous reads, require `0.4.1`, `v0.4.1`, and `latest` to resolve to the same index digest. Require the index to contain exactly the held Linux AMD64 and ARM64 children. Run the native qualifier against the public AMD64 image on this host.

If finalization fails after executable publication, preserve the immutable executable release and staged leaves. Do not delete, retag, or retry from another commit without reviewed recovery.

## 7. Record final evidence

After every check passes, replace PREPARED with COMPLETE and append the authorization binding, verified UTC date, publication commit, CI and container smoke runs, package run and artifact digest, exact executable inventory, release ID, stage run and receipt artifact identities, native leaf digests, finalize run, OCI index digest, anonymous tag checks, installer result, and isolated code-only and full-uninstall results.

Update observed-current architecture and planning claims only after both delivery forms are public and qualified. Keep v0.4.0 executable history, the v0.3.0 container publication record, earlier release bodies, and fixed-version fixtures unchanged.
