# Ticket 051 public container record

State: **PREPARED — no GHCR package, leaf, tag, or manifest has been published
by this ticket yet.**

The reviewed target is one thin public OCI index at
`ghcr.io/genomoncology/pangopup`. It has exactly two native leaves,
`linux/amd64` and `linux/arm64`, and the human tags `0.2.0` and `v0.2.0` plus
the moving tag `latest`. Scoring assets and SQLite data remain outside the
image. The index digest, not a tag, is the immutable deployment identity.

GitHub creates the first GHCR package as private and provides no supported API
for changing its visibility. Publication therefore has two explicit manual
workflow modes. The first run stages private leaves and emits one canonical
receipt. The coordinator then stops and gives the organization owner the exact
package-settings URL. Only after the owner confirms the irreversible public
visibility change and anonymous digest access succeeds may the coordinator
dispatch finalization with that exact stage run ID. Finalization authenticates
the stage run and receipt itself; it never trusts hand-copied digests or chooses
a latest run.

The runbooks below contain no credential. They use the operator's existing
authenticated `gh` session only for GitHub reads and reviewed workflow
dispatches. Public verification uses a fresh empty Docker configuration.

## Stage runbook — stop at visibility checkpoint

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
printf '%s\n' 'Ask the organization owner to make exactly this GHCR package public:'
printf '%s\n' 'https://github.com/orgs/genomoncology/packages/container/pangopup/settings'
printf '%s\n' 'Do not dispatch finalize until the owner confirms and anonymous pulls pass.'
```

The coordinator records the printed stage run ID outside the disposable shell
directory. The organization owner changes only this package's visibility to
Public and confirms the warning that public visibility cannot be reversed.

## Finalize runbook — only after owner confirmation

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

Coordinator: pending. Record only the exact reviewed commit, green gate run
IDs, stage and finalize run IDs, package URL and public visibility, the OCI
index digest, the two child digests, tag resolution, and anonymous
qualification results. Do not record tokens, authorization headers, signed
artifact URLs, environment dumps, registry configuration, or private paths.
