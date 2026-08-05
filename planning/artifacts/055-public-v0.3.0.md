# Ticket 055 v0.3.0 publication record

State: **PREPARED — no Ticket 055 public effect has run.**

This record is the coordinator's fail-closed runbook and, after publication,
the redacted evidence ledger for the application-only v0.3.0 release. It does
not build, upload, retag, delete, or otherwise mutate `snv-grch38-v1`,
`runtime-grch38-v1`, v0.1.0, v0.2.0, or their historical container tags.

The intended public result is one exact reviewed commit represented in both
forms:

- an immutable GitHub release `v0.3.0` containing exactly six Linux x86-64
  executable files; and
- one public GHCR OCI index, tagged `0.3.0`, `v0.3.0`, and `latest`, containing
  exactly native Linux AMD64 and ARM64 leaves.

The release body is the exact bytes of
`planning/artifacts/054-release-notes.md`. The container stays thin: it contains
the executable and notices, not scoring assets.

## Fixed predecessor identities

Publication may proceed only while both moving aliases still identify the
independently qualified v0.2.0 release:

- GitHub Latest: tag `v0.2.0`, immutable release ID `364960381`, target commit
  `c50dd1399b10b8e85e140305c7bd68fe849f77dd`.
- GHCR `latest`: OCI index
  `sha256:ad1aa8c27cc61d107310f609cd63f8fcbaf591a4f9760db475384a0a71049de4`.

The finalization workflow carries the latter digest as `PREVIOUS_INDEX` and
checks it at the last practical point immediately before creating the new
index. A changed predecessor is a stop condition, not permission to overwrite
an unexpected release.

## Coordinator-only inputs

Replace these placeholders only after reviewed preparation is committed and
pushed:

```bash
readonly REPO=genomoncology/pangopup
readonly COMMIT=REPLACE_WITH_40_LOWERCASE_PUBLICATION_READY_COMMIT
readonly CI_RUN_ID=REPLACE_WITH_EXACT_SUCCESSFUL_CI_RUN_ID
readonly CONTAINER_RUN_ID=REPLACE_WITH_EXACT_SUCCESSFUL_CONTAINER_RUN_ID
readonly QUALIFICATION_ROOT=REPLACE_WITH_ABSENT_ABSOLUTE_PRIVATE_DIRECTORY
readonly VERSION=0.3.0
readonly TAG=v0.3.0
readonly IMAGE=ghcr.io/genomoncology/pangopup
readonly PREVIOUS_RELEASE_ID=364960381
readonly PREVIOUS_COMMIT=c50dd1399b10b8e85e140305c7bd68fe849f77dd
readonly PREVIOUS_INDEX=sha256:ad1aa8c27cc61d107310f609cd63f8fcbaf591a4f9760db475384a0a71049de4
```

Never put a token, authenticated or temporary download address, request header,
or credential path in this file. `gh` and Docker use their existing local authentication;
anonymous checks use a fresh temporary Docker configuration and explicitly
unset GitHub token variables.

## 1. Local and remote preflight

Run from the reviewed checkout with `set -euo pipefail` and `umask 077`:

```bash
[[ "$COMMIT" =~ ^[0-9a-f]{40}$ ]]
[[ "$CI_RUN_ID" =~ ^[1-9][0-9]*$ && "$CONTAINER_RUN_ID" =~ ^[1-9][0-9]*$ ]]
[[ "$QUALIFICATION_ROOT" == /* && ! -e "$QUALIFICATION_ROOT" && ! -L "$QUALIFICATION_ROOT" ]]
test "$(git remote get-url origin)" = git@github.com:genomoncology/pangopup.git
test "$(git rev-parse HEAD)" = "$COMMIT"
git diff --quiet --
git diff --cached --quiet --
test -z "$(git replace -l)"
git fetch --force --prune origin main
test "$(git rev-parse origin/main)" = "$COMMIT"
for tool in gh jq docker curl sha256sum unzip tar; do command -v "$tool" >/dev/null; done
gh auth status
docker buildx version

test "$(gh api "repos/$REPO" --jq .visibility)" = public
test "$(gh api "orgs/genomoncology/packages/container/pangopup" --jq .visibility)" = public
test "$(gh api "repos/$REPO/immutable-releases" --jq .enabled)" = true
test "$(gh api "repos/$REPO/releases/latest" --jq .id)" = "$PREVIOUS_RELEASE_ID"
test "$(gh api "repos/$REPO/releases/latest" --jq .tag_name)" = v0.2.0
test "$(gh api "repos/$REPO/releases/latest" --jq .target_commitish)" = "$PREVIOUS_COMMIT"
test -z "$(gh api --paginate "repos/$REPO/releases" --jq '.[].tag_name' | grep -Fx "$TAG" || true)"
test "$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length)" -eq 0
```

Use a fresh anonymous Docker configuration to resolve `latest` and require the
fixed predecessor digest. Also require `0.3.0` and `v0.3.0` to be absent using
the checked `scripts/require-container-tag-absent.sh`; an authorization,
protocol, malformed-response, or registry failure is not absence.

Authenticate the exact green remote gates, not merely the newest runs:

```bash
gh run view "$CI_RUN_ID" --repo "$REPO" --json headSha,name,status,conclusion,jobs >ci.json
jq -e --arg commit "$COMMIT" '.headSha == $commit and .name == "ci" and
  .status == "completed" and .conclusion == "success" and
  ([.jobs[] | select(.name == "gate" and .conclusion == "success")] | length) == 1' ci.json >/dev/null
gh run view "$CONTAINER_RUN_ID" --repo "$REPO" --json headSha,name,status,conclusion,jobs >container.json
jq -e --arg commit "$COMMIT" '.headSha == $commit and .name == "container" and
  .status == "completed" and .conclusion == "success" and
  ([.jobs[] | select(.name | startswith("native-smoke (")) |
    select(.conclusion == "success")] | length) == 2' container.json >/dev/null
```

Repeat the repository security/ruleset audit from the retained Ticket 050
record. Stop for an open secret alert or changed Actions, Dependabot, secret
scanning, ruleset, immutable-release, repository-visibility, or package-
visibility state. This is an observation only; the release ticket does not
change those settings.

## 2. Stage the two container leaves

Snapshot existing `publish-container.yml` workflow-dispatch run IDs, dispatch
`stage` on `main` with `commit=$COMMIT` and an empty `stage_run_id`, then require
exactly one new run. Wait for success and authenticate all of the following by
API:

- event `workflow_dispatch`, path `.github/workflows/publish-container.yml`,
  and `head_sha=$COMMIT`;
- exactly one successful `preflight-stage`, two successful native
  `stage-leaf` jobs, and one successful `aggregate-stage-receipt`;
- exactly one unexpired artifact named
  `pangopup-container-stage-$COMMIT-$STAGE_RUN_ID`;
- the artifact archive's API-reported SHA-256; and
- the canonical receipt through
  `scripts/admit-container-stage-receipt.sh metadata` and `archive`.

The receipt must have only the schema, mode, run, commit, workflow commit, and
two distinct leaf digests defined by `pangopup-container-stage-v1`. With a
fresh anonymous Docker configuration, both digest-only leaves must be readable.
Run `scripts/qualify-container.sh` natively for each architecture. There are no
user-facing v0.3.0 tags at this stage.

If staging fails, keep any untagged leaves and stop. Do not reuse a failed or
superseded stage run.

## 3. Build and admit the executable artifact

Snapshot existing `package-linux.yml` dispatch run IDs, dispatch exactly
`commit=$COMMIT`, and admit exactly one new successful run. Authenticate its
event, workflow path/name, head SHA, successful `package` job, and unique
unexpired artifact named `pangopup-linux-$COMMIT`.

Download the artifact archive through its API ID. Require the server-reported
artifact digest to equal the downloaded ZIP SHA-256 before extracting it into
an absent private directory. Then run:

```bash
scripts/qualify-linux-release.sh "$RELEASE_DIR" 0.3.0 "$COMMIT"
```

Require exactly these direct regular, single-link members and record each exact
size and SHA-256:

```text
LICENSE
NOTICE
pangopup-linux-x86_64
pangopup-linux-x86_64.cdx.json
pangopup-linux-x86_64.sha256
release-manifest.json
```

The manifest must say version 0.3.0 and target commit `$COMMIT`; the checksum,
CycloneDX SBOM, notices, executable version, and maximum imported GLIBC 2.39
must pass the checked qualifier. Repeat the clean pinned Ubuntu smoke used by
`package-linux.yml`.

## 4. Create, verify, and publish the executable release

Create one private draft titled `PangoPup v0.3.0`, targeting `$COMMIT`, with
the byte-exact body from `planning/artifacts/054-release-notes.md`. Upload only
the six admitted files. After each upload and once more at the end, compare the
remote asset name, size, and digest inventory with the held local inventory.
Compare the fetched draft body byte-for-byte with the source file.

Immediately before changing the draft to public, repeat the GitHub Latest
predecessor checks from section 1, require
`repos/$REPO/git/matching-refs/tags/$TAG` to remain empty, and recheck that the
draft still targets `$COMMIT`, is not a prerelease, and has exactly six assets.
These checks must be adjacent to the publish request, after all draft uploads
and comparisons:

```bash
test "$(gh api "repos/$REPO/releases/latest" --jq .id)" = "$PREVIOUS_RELEASE_ID"
test "$(gh api "repos/$REPO/releases/latest" --jq .tag_name)" = v0.2.0
test "$(gh api "repos/$REPO/releases/latest" --jq .target_commitish)" = "$PREVIOUS_COMMIT"
test "$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length)" -eq 0
gh api "repos/$REPO/releases/$RELEASE_ID" >"$PRIVATE/prepublish.json"
jq -e --arg tag "$TAG" --arg target "$COMMIT" \
  '.tag_name == $tag and .target_commitish == $target and .draft == true and
   .prerelease == false and (.assets | length) == 6' \
  "$PRIVATE/prepublish.json" >/dev/null
jq -jr .body "$PRIVATE/prepublish.json" >"$PRIVATE/prepublish-body"
cmp "$BODY" "$PRIVATE/prepublish-body"
gh api --method PATCH "repos/$REPO/releases/$RELEASE_ID" \
  -F draft=false -f make_latest=true >"$PRIVATE/published.json"
```

Before publication, a failure may delete only the authenticated private draft
when its tag is still absent. After publication, never delete or edit the
release automatically: record the exact partial state and stop.

Through unauthenticated API and downloads, require the public release to be
immutable, non-draft, non-prerelease, Latest, targeted at `$COMMIT`, and to
retain the exact title, body, six names, sizes, and SHA-256 digests. Require the
public tag object to resolve to `$COMMIT`.

## 5. Qualify the pinned public installer and uninstall boundaries

Use an absent private qualification root and a pinned Ubuntu 24.04 container.
Copy the retained compatible installation and cache from
`/home/ian/workspace/data/pangopup-release-050-c50dd13/` into disposable
directories (prefer a checked same-filesystem reflink; never run destructive
tests against the retained originals). Install as a non-root user with:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh \
  | bash -s -- --version 0.3.0
```

Require clean stderr and `pangopup 0.3.0` from both `--version` and `-V`.
Run `scripts/run-production-qualification.sh ... --reuse-installed` followed by
`scripts/check-production-qualification.py ... --reuse-installed`. This checks
focused help, offline zero-download reuse, ready status, all 1,000 retained SNV
oracles, automatic and forced model routes, durable SQLite reuse, and HTTP
health/status/SNV/model behavior.

Separately copy only the public executable into two absent disposable trees.
For code-only behavior, run interactive `uninstall`, select `1`, verify the
displayed executable/data/cache paths, require the executable to be removed,
and require data/cache markers to remain. For full behavior, run
`uninstall --full --yes`, verify the displayed paths, and require executable,
managed data, and managed cache to be removed without a prompt. Use only
explicit `PANGOPUP_DATA_DIR` and `PANGOPUP_CACHE_DIR` paths beneath those
disposable trees. Never point uninstall at the retained qualification assets.

## 6. Finalize the container aliases

Only after the executable release and installer qualification pass, snapshot
workflow run IDs and dispatch `publish-container.yml` in `finalize` mode with
exactly `commit=$COMMIT` and the authenticated successful `$STAGE_RUN_ID`.
Supply no hand-copied leaf digest. The workflow re-admits the receipt, repeats
anonymous native leaf qualification, proves `0.3.0` and `v0.3.0` absent, and
proves GHCR `latest` still equals `$PREVIOUS_INDEX` immediately before
creating the index.

Authenticate exactly one new successful finalize run and its one successful
`load-stage-receipt`, two successful `qualify-public-leaf` jobs, and one
successful `finalize-manifest`. Through a fresh anonymous Docker configuration,
require:

- one OCI image index with exactly `linux/amd64` and `linux/arm64`;
- children exactly equal to the two staged receipt digests;
- source, revision `$COMMIT`, version `0.3.0`, and GPL-3.0-only annotations;
- `0.3.0`, `v0.3.0`, and `latest` all resolving to that index; and
- native `scripts/qualify-container.sh` success for both public leaves.

If finalization fails after executable publication, stop. Keep the executable
release and staged leaves; do not delete, retag, or retry from a different
commit without a separately reviewed recovery.

## 7. Evidence to retain after effects

Replace `pending` below with redacted facts only after every corresponding
check has passed. Preserve failures and superseded runs as such.

- Exact publication commit: pending.
- Exact green CI run: pending.
- Exact green native container run: pending.
- Package run and admitted artifact ID/digest: pending.
- GitHub release ID, tag, target, immutability, and Latest evidence: pending.
- Six executable member names, sizes, and SHA-256 digests: pending.
- Pinned installer and production qualification: pending.
- Isolated interactive code-only and `--full --yes` uninstall evidence:
  pending.
- Container stage run and canonical receipt artifact ID/digest: pending.
- AMD64 and ARM64 leaf digests: pending.
- Container finalize run and OCI index digest: pending.
- Anonymous public API/download/registry and native qualification: pending.
- Final exact-commit remote gates: pending.

After public success, update this state to COMPLETE and reconcile only rolling
planning documents that still call publication the next outcome. The README
and release notes in the tag are already final and must not be rewritten after
publication.
