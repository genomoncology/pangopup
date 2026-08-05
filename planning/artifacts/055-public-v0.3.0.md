# Ticket 055 v0.3.0 publication record

State: **COMPLETE — immutable v0.3.0 executable and native container are public and qualified.**

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

Never put a token value, authenticated or temporary download address, request
header, or credential path in this file. `gh` uses its existing local
authentication. Registry checks use a short-lived anonymous pull token and a
fresh Docker configuration with no registry credentials.

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
test "$(gh api "repos/$REPO/immutable-releases" --jq .enabled)" = true
test "$(gh api "repos/$REPO/releases/latest" --jq .id)" = "$PREVIOUS_RELEASE_ID"
test "$(gh api "repos/$REPO/releases/latest" --jq .tag_name)" = v0.2.0
test "$(gh api "repos/$REPO/releases/latest" --jq .target_commitish)" = "$PREVIOUS_COMMIT"
test -z "$(gh api --paginate "repos/$REPO/releases" --jq '.[].tag_name' | grep -Fx "$TAG" || true)"
test "$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length)" -eq 0
```

Do not query the authenticated package-settings API: it requires a
`read:packages` scope that publication does not otherwise need and proves less
than a public pull. Instead, create a fresh Docker configuration, obtain an
unauthenticated GHCR pull token, and require `latest` to return the fixed
predecessor digest. Also require `0.3.0` and `v0.3.0` to be absent using the
checked `scripts/require-container-tag-absent.sh`; an authorization, protocol,
malformed-response, or registry failure is not absence.

```bash
PRIVATE=$(mktemp -d)
PUBLIC_DOCKER=$(mktemp -d)
readonly PRIVATE PUBLIC_DOCKER
chmod 0700 "$PRIVATE" "$PUBLIC_DOCKER"
DOCKER_CONFIG="$PUBLIC_DOCKER" docker logout ghcr.io >/dev/null 2>&1 || true
if [[ -e "$PUBLIC_DOCKER/config.json" ]]; then
  jq -e '((.auths // {}) | length) == 0' "$PUBLIC_DOCKER/config.json" >/dev/null
fi
anonymous_token=$(curl -q -fsS --get \
  --data-urlencode service=ghcr.io \
  --data-urlencode scope=repository:genomoncology/pangopup:pull \
  https://ghcr.io/token | jq -er .token)
test -n "$anonymous_token"

anonymous_digest() {
  local reference=$1 expected=$2 label=$3 code
  code=$(curl -q -sS --oauth2-bearer "$anonymous_token" \
    -D "$PRIVATE/$label.headers" -o "$PRIVATE/$label.json" -w '%{http_code}' \
    -H 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' \
    "https://ghcr.io/v2/genomoncology/pangopup/manifests/$reference")
  scripts/require-container-tag-digest.sh "$label" "$code" \
    "$PRIVATE/$label.headers" "$expected"
}
anonymous_absent() {
  local tag=$1 code
  code=$(curl -q -sS --oauth2-bearer "$anonymous_token" \
    -o "$PRIVATE/absent-$tag.json" -w '%{http_code}' \
    -H 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' \
    "https://ghcr.io/v2/genomoncology/pangopup/manifests/$tag")
  scripts/require-container-tag-absent.sh "$tag" "$code" \
    "$PRIVATE/absent-$tag.json"
}
anonymous_digest latest "$PREVIOUS_INDEX" latest
anonymous_absent 0.3.0
anonymous_absent v0.3.0
```

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
scanning, ruleset, immutable-release, or repository-visibility state. This is
an observation only; the release ticket does not change those settings.

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
two distinct leaf digests defined by `pangopup-container-stage-v1`. Reuse the
fresh anonymous token and credential-free Docker configuration from preflight;
require each digest request to return HTTP 200 with
`Docker-Content-Digest` exactly equal to the requested receipt digest:

```bash
amd64=$(jq -er .amd64 "$RECEIPT")
arm64=$(jq -er .arm64 "$RECEIPT")
anonymous_digest "$amd64" "$amd64" staged-amd64
anonymous_digest "$arm64" "$arm64" staged-arm64
DOCKER_CONFIG="$PUBLIC_DOCKER" docker buildx imagetools inspect "$IMAGE@$amd64" >/dev/null
DOCKER_CONFIG="$PUBLIC_DOCKER" docker buildx imagetools inspect "$IMAGE@$arm64" >/dev/null
```

Run `scripts/qualify-container.sh` natively for each architecture using that
same credential-free Docker configuration. There are no user-facing v0.3.0
tags at this stage.

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

The following redacted facts were recorded only after every corresponding
check passed. Failures and superseded preparation are preserved as such.

- Exact publication commit: `3a857f7def2c11ad9d9e38ed62b7204bf7d6b691`.
- Exact green CI run: `30993530989`; exact green native-container run:
  `30993531181` (native AMD64 and ARM64 smoke jobs passed).
- Package run `30994324171`; admitted artifact ID `8925565070`, digest
  `sha256:e1dc222db3180359b2dba203a5c0dc4a851df32b7511e049feb357c88072b244`.
- GitHub release ID `365425336`, tag `v0.3.0`, target the exact publication
  commit, immutable, non-prerelease, and Latest. Anonymous API and all six
  downloads matched the reviewed body and inventory.
- Executable members (name, bytes, SHA-256):
  - `LICENSE`, 35,149,
    `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986`;
  - `NOTICE`, 1,899,
    `19d4942d45f87794e304cf8a3d72a7c7a685fb4641a772f9a35acf8b701754c7`;
  - `pangopup-linux-x86_64`, 29,069,480,
    `cd5a451190c35af1fe5dd481abf64d06f430c4154db94068fc889131ccaa3578`;
  - `pangopup-linux-x86_64.cdx.json`, 201,980,
    `6b63fa3761f4bd81b3cd949f3fb68a06caf4962d9e6490b2cfda1ac1d62bc1da`;
  - `pangopup-linux-x86_64.sha256`, 88,
    `976d6b12925f11ce021a44f9a9480684db71c0db0f7df7187ed69fc0212384fe`;
  - `release-manifest.json`, 950,
    `5b5df42e65d6f35d37147e59e40ef7745212e94e0e1655677dff2b3a26d34d59`.
- The pinned tagged curl installer passed in a clean Ubuntu 24.04 container as
  UID 12345. Reusing disposable copies of retained assets, all 1,000 SNV
  oracles, automatic and forced model routes, SQLite reuse, offline status,
  focused help, and HTTP health/status/SNV/model checks passed.
- Isolated interactive code-only uninstall displayed all paths, removed the
  executable, and preserved data/cache. Isolated `--full --yes` displayed the
  paths, prompted for nothing, and removed executable/data/cache. Failed
  exploratory fixtures were rejected before mutation; retained assets were
  never uninstall targets.
- Container stage run `30993928539`; receipt artifact ID `8925408153`, digest
  `sha256:f439070360c28c436cdc6fc0baaf5a95ccb1d4ae0783669d56012eac7144c432`.
- AMD64 leaf
  `sha256:cc85a70eb6549e35a3641070217c9252759f2b9e22ddfc7ef83605ca54470aba`;
  ARM64 leaf
  `sha256:d73958dbd3dc7c2252b01b3354968f8d24f9585e7e5b4aa6d76f8b1e5bc5b1c0`.
- Container finalize run `30995022437`; OCI index
  `sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`.
  Anonymous registry reads proved the exact two children, annotations, and
  equality of `0.3.0`, `v0.3.0`, and `latest`. Native qualification passed on
  both GitHub architectures and was repeated locally for AMD64.
- The initial effect-free preflight stopped on a missing optional
  `read:packages` scope. The independently reviewed amendment replaced that
  weaker settings read with credential-free exact registry reads; its new
  exact commit and gates passed before any public effect.

The README and release notes in the tag were final before publication and were
not rewritten afterward. Local redacted evidence is retained under
`/home/ian/workspace/data/pangopup-release-055-3a857f7/`; it contains no saved
registry token or authenticated URL.
