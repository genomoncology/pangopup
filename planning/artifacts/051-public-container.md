# Ticket 051 public container record

State: **COMPLETE — the public `0.2.0`, `v0.2.0`, and `latest` tags resolve to
one anonymously qualified native AMD64/ARM64 OCI index.**

The first stage attempt, GitHub Actions run `30928210091`, failed in both native
leaf jobs before any registry push. GitHub's default Buildx `docker` driver does
not implement the required `push-by-digest` exporter. The retry workflow now
creates and explicitly selects the pinned official Docker-container Buildx
builder and an immutable multi-architecture BuildKit image. The workflow also
boots the selected builder and verifies its driver before beginning either
native build.

The repaired staging run `30929323700` succeeded at commit
`c4dae255a8766a21ec8e56339a0cb6afd69a8d53` and created the two digest-only
native leaves. During finalize run `30931154337`, both anonymous native
qualification jobs passed. The manifest job then failed on its first
child-label inspection: the single-quoted Docker Go template contained escaped
double quotes, so Docker
received literal backslashes and rejected the template. This happened before
the final tag-absence checks and before `imagetools create`; consequently no
`0.2.0`, `v0.2.0`, or `latest` tag and no multi-platform index was created. The
workflow now sends the two label templates without backslashes and has an exact
static regression for both commands.

Stage run `30929323700` and finalize run `30931154337` are now abandoned
evidence, not recovery inputs. The stage receipt binds its commit and
`workflow_sha` to the old commit, so it must not be reused after the quoting
fix is committed. Recovery requires the remediation commit to be pushed, its
CI and native-container gates to pass, and then a new exact-commit `stage` run.
Only the new stage run ID and its receipt may be supplied to `finalize`; the
workflow's existing commit and receipt checks must not be weakened.

The two old untagged leaves do not collide with the absent human tags or the
future index. They are retained as failed-publication evidence for now. This
recovery does not add broad package deletion authority or risk deleting a leaf
referenced by an in-flight or future manifest; any cleanup can be a separate,
bounded operation after successful publication.

The reviewed target is one thin public OCI index at
`ghcr.io/genomoncology/pangopup`. It has exactly two native leaves,
`linux/amd64` and `linux/arm64`, and the human tags `0.2.0` and `v0.2.0` plus
the moving tag `latest`. Scoring assets and SQLite data remain outside the
image. The index digest, not a tag, is the immutable deployment identity.

Publication uses two explicit manual workflow modes. The first run stages
digest-addressed leaves without user-facing tags and emits one canonical
receipt. The workflow is prepared to stop for a package-visibility checkpoint,
but this repository-linked package was already Public when the owner opened its
settings; no visibility mutation was required. Anonymous digest access was
still proved before finalization. Finalization authenticates the stage run and
receipt itself; it never trusts hand-copied digests or chooses a latest run.

The runbooks below contain no credential. They use the operator's existing
authenticated `gh` session only for GitHub reads and reviewed workflow
dispatches. Public verification uses a fresh empty Docker configuration.

## Stage runbook — create a new commit-bound receipt

```bash
set -euo pipefail
umask 077

readonly REPO=genomoncology/pangopup
readonly WORKFLOW=publish-container.yml
readonly COMMIT=REPLACE_WITH_40_LOWERCASE_PUBLICATION_READY_COMMIT
readonly CI_RUN_ID=REPLACE_WITH_SUCCESSFUL_CI_RUN_ID
readonly CONTAINER_RUN_ID=REPLACE_WITH_SUCCESSFUL_NATIVE_CONTAINER_RUN_ID
readonly CHECKOUT=$PWD

[[ "$COMMIT" =~ ^[0-9a-f]{40}$ ]]
[[ "$CI_RUN_ID" =~ ^[1-9][0-9]*$ ]]
[[ "$CONTAINER_RUN_ID" =~ ^[1-9][0-9]*$ ]]
test "$(git remote get-url origin)" = git@github.com:genomoncology/pangopup.git
test "$(git rev-parse HEAD)" = "$COMMIT"
git diff --quiet --
git diff --cached --quiet --
git fetch --force --prune origin main
test "$(git rev-parse origin/main)" = "$COMMIT"
test -z "$(git replace -l)"
for tool in gh jq docker unzip; do command -v "$tool" >/dev/null; done

PRIVATE=$(mktemp -d)
chmod 0700 "$PRIVATE"
cleanup() { rm -rf -- "$PRIVATE"; }
trap cleanup EXIT

test "$(gh api "repos/$REPO" --jq .visibility)" = public
gh run view "$CI_RUN_ID" --repo "$REPO" --json headSha,name,status,conclusion,jobs \
  >"$PRIVATE/ci.json"
jq -e --arg commit "$COMMIT" '.headSha == $commit and .name == "ci" and
  .status == "completed" and .conclusion == "success" and
  ([.jobs[] | select(.name == "gate" and .conclusion == "success")] | length) == 1' \
  "$PRIVATE/ci.json" >/dev/null
gh run view "$CONTAINER_RUN_ID" --repo "$REPO" \
  --json headSha,name,status,conclusion,jobs >"$PRIVATE/container.json"
jq -e --arg commit "$COMMIT" '.headSha == $commit and .name == "container" and
  .status == "completed" and .conclusion == "success" and
  ([.jobs[] | select(.name | startswith("smoke ("))] | length) == 2 and
  all(.jobs[] | select(.name | startswith("smoke (")); .conclusion == "success")' \
  "$PRIVATE/container.json" >/dev/null

gh api "repos/$REPO/actions/workflows/$WORKFLOW" >"$PRIVATE/workflow.json"
jq -e '.path == ".github/workflows/publish-container.yml" and
  .name == "publish-container" and .state == "active"' "$PRIVATE/workflow.json" >/dev/null
gh run list --repo "$REPO" --workflow "$WORKFLOW" --event workflow_dispatch \
  --limit 100 --json databaseId | jq -r '.[].databaseId' | sort -n >"$PRIVATE/before"
gh workflow run "$WORKFLOW" --repo "$REPO" --ref main \
  -f mode=stage -f commit="$COMMIT" -f stage_run_id=''
STAGE_RUN_ID=
for _ in $(seq 1 60); do
  gh run list --repo "$REPO" --workflow "$WORKFLOW" --event workflow_dispatch \
    --limit 100 --json databaseId | jq -r '.[].databaseId' | sort -n >"$PRIVATE/after"
  mapfile -t new_runs < <(comm -13 "$PRIVATE/before" "$PRIVATE/after")
  if (( ${#new_runs[@]} == 1 )); then STAGE_RUN_ID=${new_runs[0]}; break; fi
  (( ${#new_runs[@]} == 0 )) || { printf 'ambiguous publication runs\n' >&2; exit 1; }
  sleep 5
done
[[ "$STAGE_RUN_ID" =~ ^[1-9][0-9]*$ ]]
gh run watch "$STAGE_RUN_ID" --repo "$REPO" --exit-status
gh api "repos/$REPO/actions/runs/$STAGE_RUN_ID" >"$PRIVATE/stage-run.json"
jq -e --arg commit "$COMMIT" '.event == "workflow_dispatch" and
  .status == "completed" and .conclusion == "success" and .head_sha == $commit and
  .path == ".github/workflows/publish-container.yml"' \
  "$PRIVATE/stage-run.json" >/dev/null
gh api --paginate "repos/$REPO/actions/runs/$STAGE_RUN_ID/jobs?filter=latest&per_page=100" \
  --jq '.jobs[]' | jq -s '.' >"$PRIVATE/stage-jobs.json"
jq -e '([.[] | select(.name == "preflight-stage" and .conclusion == "success")] | length) == 1 and
  ([.[] | select(.name | startswith("stage-leaf (")) | select(.conclusion == "success")] | length) == 2 and
  ([.[] | select(.name == "aggregate-stage-receipt" and .conclusion == "success")] | length) == 1' \
  "$PRIVATE/stage-jobs.json" >/dev/null

gh api --paginate "repos/$REPO/actions/runs/$STAGE_RUN_ID/artifacts?per_page=100" \
  --jq '.artifacts[]' | jq -s '.' >"$PRIVATE/artifacts.json"
readonly RECEIPT_NAME="pangopup-container-stage-$COMMIT-$STAGE_RUN_ID"
IFS=$'\t' read -r ARTIFACT_ID ARTIFACT_DIGEST < <(
  "$CHECKOUT/scripts/admit-container-stage-receipt.sh" metadata \
    "$PRIVATE/artifacts.json" "$RECEIPT_NAME"
)
gh api "repos/$REPO/actions/artifacts/$ARTIFACT_ID/zip" >"$PRIVATE/receipt.zip"
test "$ARTIFACT_DIGEST" = "sha256:$(sha256sum "$PRIVATE/receipt.zip" | cut -d' ' -f1)"
readonly RECEIPT=$PRIVATE/stage-receipt.json
"$CHECKOUT/scripts/admit-container-stage-receipt.sh" archive \
  "$PRIVATE/artifacts.json" "$RECEIPT_NAME" "$PRIVATE/receipt.zip" \
  "$COMMIT" "$COMMIT" "$STAGE_RUN_ID" "$RECEIPT"
test "$(jq -cS . "$RECEIPT")" = "$(cat "$RECEIPT")"
jq -e --arg commit "$COMMIT" --argjson run_id "$STAGE_RUN_ID" \
  'keys == ["amd64","arm64","commit","mode","run_id","schema","workflow_sha"] and
  .schema == "pangopup-container-stage-v1" and .mode == "stage" and
  .commit == $commit and .workflow_sha == $commit and .run_id == $run_id and
  (.amd64 | test("^sha256:[0-9a-f]{64}$")) and
  (.arm64 | test("^sha256:[0-9a-f]{64}$")) and .amd64 != .arm64' "$RECEIPT" >/dev/null

printf 'STOP: stage run %s is authenticated.\n' "$STAGE_RUN_ID"
test "$(gh api "/orgs/genomoncology/packages/container/pangopup" --jq .visibility)" = public
printf '%s\n' 'The existing GHCR package is public; no visibility mutation is required.'
printf '%s\n' 'Package settings reference:'
printf '%s\n' 'https://github.com/orgs/genomoncology/packages/container/pangopup/settings'
printf '%s\n' 'Do not dispatch finalize until anonymous pulls of this new receipt pass.'
```

The coordinator records the printed stage run ID outside the disposable shell
directory. For this recovery, the package is already public. The new stage ID
must replace, never reuse, superseded stage ID `30929323700` in the finalize
runbook.

## Finalize runbook — only with the newly authenticated stage ID

```bash
set -euo pipefail
umask 077

readonly REPO=genomoncology/pangopup
readonly WORKFLOW=publish-container.yml
readonly IMAGE=ghcr.io/genomoncology/pangopup
readonly COMMIT=REPLACE_WITH_40_LOWERCASE_PUBLICATION_READY_COMMIT
readonly STAGE_RUN_ID=REPLACE_WITH_EXACT_SUCCESSFUL_STAGE_RUN_ID
readonly CHECKOUT=$PWD

[[ "$COMMIT" =~ ^[0-9a-f]{40}$ && "$STAGE_RUN_ID" =~ ^[1-9][0-9]*$ ]]
test "$(git remote get-url origin)" = git@github.com:genomoncology/pangopup.git
test "$(git rev-parse HEAD)" = "$COMMIT"
git diff --quiet --
git diff --cached --quiet --
git fetch --force --prune origin main
test "$(git rev-parse origin/main)" = "$COMMIT"
test -z "$(git replace -l)"
for tool in gh jq docker unzip; do command -v "$tool" >/dev/null; done

PRIVATE=$(mktemp -d)
PUBLIC_DOCKER=$(mktemp -d)
chmod 0700 "$PRIVATE" "$PUBLIC_DOCKER"
cleanup() { rm -rf -- "$PRIVATE" "$PUBLIC_DOCKER"; }
trap cleanup EXIT

test "$(gh api "/orgs/genomoncology/packages/container/pangopup" --jq .visibility)" = public
gh api "repos/$REPO/actions/runs/$STAGE_RUN_ID" >"$PRIVATE/stage-run.json"
jq -e --arg commit "$COMMIT" '.event == "workflow_dispatch" and
  .status == "completed" and .conclusion == "success" and .head_sha == $commit and
  .path == ".github/workflows/publish-container.yml"' \
  "$PRIVATE/stage-run.json" >/dev/null
gh api --paginate "repos/$REPO/actions/runs/$STAGE_RUN_ID/jobs?filter=latest&per_page=100" \
  --jq '.jobs[]' | jq -s '.' >"$PRIVATE/stage-jobs.json"
jq -e '([.[] | select(.name == "preflight-stage" and .conclusion == "success")] | length) == 1 and
  ([.[] | select(.name | startswith("stage-leaf (")) | select(.conclusion == "success")] | length) == 2 and
  ([.[] | select(.name == "aggregate-stage-receipt" and .conclusion == "success")] | length) == 1' \
  "$PRIVATE/stage-jobs.json" >/dev/null
gh api --paginate "repos/$REPO/actions/runs/$STAGE_RUN_ID/artifacts?per_page=100" \
  --jq '.artifacts[]' | jq -s '.' >"$PRIVATE/artifacts.json"
readonly RECEIPT_NAME="pangopup-container-stage-$COMMIT-$STAGE_RUN_ID"
IFS=$'\t' read -r ARTIFACT_ID ARTIFACT_DIGEST < <(
  "$CHECKOUT/scripts/admit-container-stage-receipt.sh" metadata \
    "$PRIVATE/artifacts.json" "$RECEIPT_NAME"
)
gh api "repos/$REPO/actions/artifacts/$ARTIFACT_ID/zip" >"$PRIVATE/receipt.zip"
test "$ARTIFACT_DIGEST" = "sha256:$(sha256sum "$PRIVATE/receipt.zip" | cut -d' ' -f1)"
readonly RECEIPT=$PRIVATE/stage-receipt.json
"$CHECKOUT/scripts/admit-container-stage-receipt.sh" archive \
  "$PRIVATE/artifacts.json" "$RECEIPT_NAME" "$PRIVATE/receipt.zip" \
  "$COMMIT" "$COMMIT" "$STAGE_RUN_ID" "$RECEIPT"
test "$(jq -cS . "$RECEIPT")" = "$(cat "$RECEIPT")"
jq -e --arg commit "$COMMIT" --argjson run_id "$STAGE_RUN_ID" \
  'keys == ["amd64","arm64","commit","mode","run_id","schema","workflow_sha"] and
  .schema == "pangopup-container-stage-v1" and .mode == "stage" and
  .commit == $commit and .workflow_sha == $commit and .run_id == $run_id and
  (.amd64 | test("^sha256:[0-9a-f]{64}$")) and
  (.arm64 | test("^sha256:[0-9a-f]{64}$")) and .amd64 != .arm64' "$RECEIPT" >/dev/null
amd64=$(jq -er .amd64 "$RECEIPT")
arm64=$(jq -er .arm64 "$RECEIPT")
for digest in "$amd64" "$arm64"; do
  DOCKER_CONFIG="$PUBLIC_DOCKER" docker buildx imagetools inspect "$IMAGE@$digest" >/dev/null
done

gh run list --repo "$REPO" --workflow "$WORKFLOW" --event workflow_dispatch \
  --limit 100 --json databaseId | jq -r '.[].databaseId' | sort -n >"$PRIVATE/before"
gh workflow run "$WORKFLOW" --repo "$REPO" --ref main \
  -f mode=finalize -f commit="$COMMIT" -f stage_run_id="$STAGE_RUN_ID"
FINALIZE_RUN_ID=
for _ in $(seq 1 60); do
  gh run list --repo "$REPO" --workflow "$WORKFLOW" --event workflow_dispatch \
    --limit 100 --json databaseId | jq -r '.[].databaseId' | sort -n >"$PRIVATE/after"
  mapfile -t new_runs < <(comm -13 "$PRIVATE/before" "$PRIVATE/after")
  if (( ${#new_runs[@]} == 1 )); then FINALIZE_RUN_ID=${new_runs[0]}; break; fi
  (( ${#new_runs[@]} == 0 )) || { printf 'ambiguous publication runs\n' >&2; exit 1; }
  sleep 5
done
[[ "$FINALIZE_RUN_ID" =~ ^[1-9][0-9]*$ ]]
gh run watch "$FINALIZE_RUN_ID" --repo "$REPO" --exit-status
gh api "repos/$REPO/actions/runs/$FINALIZE_RUN_ID" >"$PRIVATE/finalize-run.json"
jq -e --arg commit "$COMMIT" \
  '.event == "workflow_dispatch" and .status == "completed" and
  .conclusion == "success" and .head_sha == $commit and
  .path == ".github/workflows/publish-container.yml"' \
  "$PRIVATE/finalize-run.json" >/dev/null
gh api --paginate "repos/$REPO/actions/runs/$FINALIZE_RUN_ID/jobs?filter=latest&per_page=100" \
  --jq '.jobs[]' | jq -s '.' >"$PRIVATE/finalize-jobs.json"
jq -e '([.[] | select(.name == "load-stage-receipt" and .conclusion == "success")] | length) == 1 and
  ([.[] | select(.name | startswith("qualify-public-leaf (")) | select(.conclusion == "success")] | length) == 2 and
  ([.[] | select(.name == "finalize-manifest" and .conclusion == "success")] | length) == 1' \
  "$PRIVATE/finalize-jobs.json" >/dev/null

index_digest=$(DOCKER_CONFIG="$PUBLIC_DOCKER" docker buildx imagetools inspect \
  "$IMAGE:0.2.0" --format '{{json .Manifest.Digest}}' | jq -er '.')
[[ "$index_digest" =~ ^sha256:[0-9a-f]{64}$ ]]
raw=$(DOCKER_CONFIG="$PUBLIC_DOCKER" docker buildx imagetools inspect --raw "$IMAGE@$index_digest")
test "$(jq -r .mediaType <<<"$raw")" = application/vnd.oci.image.index.v1+json
test "$(jq -r '.annotations."org.opencontainers.image.source"' <<<"$raw")" = https://github.com/genomoncology/pangopup
test "$(jq -r '.annotations."org.opencontainers.image.revision"' <<<"$raw")" = "$COMMIT"
test "$(jq -r '.annotations."org.opencontainers.image.version"' <<<"$raw")" = 0.2.0
test "$(jq -r '.annotations."org.opencontainers.image.licenses"' <<<"$raw")" = GPL-3.0-only
test "$(jq '.manifests | length' <<<"$raw")" = 2
test "$(jq '[.manifests[].platform | (.os + "/" + .architecture)] | sort == ["linux/amd64","linux/arm64"]' <<<"$raw")" = true
test "$(jq --arg amd64 "$amd64" --arg arm64 "$arm64" \
  '[.manifests[].digest] | sort == ([$amd64,$arm64] | sort)' <<<"$raw")" = true
for tag in 0.2.0 v0.2.0 latest; do
  resolved=$(DOCKER_CONFIG="$PUBLIC_DOCKER" docker buildx imagetools inspect \
    "$IMAGE:$tag" --format '{{json .Manifest.Digest}}' | jq -er '.')
  test "$resolved" = "$index_digest"
done
DOCKER_CONFIG="$PUBLIC_DOCKER" docker pull "$IMAGE@$index_digest"
DOCKER_CONFIG="$PUBLIC_DOCKER" docker pull "$IMAGE:0.2.0"
printf 'Ticket 051 publication passed: stage_run_id=%s finalize_run_id=%s index=%s amd64=%s arm64=%s\n' \
  "$STAGE_RUN_ID" "$FINALIZE_RUN_ID" "$index_digest" "$amd64" "$arm64"
```

## External effect evidence

Coordinator: Codex.

- Exact publication commit: `e2d3c2c89813cbdf54d2c76887113e8d68e44b4a`.
- Green CI run: `30932187846`.
- Green native-container run: `30932190203`.
- Authenticated stage run: `30932555180`; receipt artifact: `8901846495`.
- Successful finalize run: `30932912158`.
- Public package: `https://github.com/orgs/genomoncology/packages/container/pangopup`.
- OCI index: `sha256:ad1aa8c27cc61d107310f609cd63f8fcbaf591a4f9760db475384a0a71049de4`.
- AMD64 leaf: `sha256:40c6da99893d0785dc19a390064b5891298d3328c99caf591e5dd049e83ca768`.
- ARM64 leaf: `sha256:54b6f8e70368e6e686ee24eb4838f92bed32b386f4ee6c785abd5a917338f476`.
- Anonymous native qualification passed for both leaves. Independent anonymous
  registry reads confirmed the exact two-child index, annotations, and that
  `0.2.0`, `v0.2.0`, and `latest` all resolve to the index digest above.

Superseded stage `30929323700` and failed finalize `30931154337` were not reused.
Their old untagged leaves remain separate from the published index.
