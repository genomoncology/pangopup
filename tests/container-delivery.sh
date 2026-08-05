#!/usr/bin/env bash
set -euo pipefail

grep -Fq 'cargo build --locked --release --package pangopup-cli' Dockerfile
grep -Fq 'USER 65532:65532' Dockerfile
grep -Fq 'ENTRYPOINT ["/usr/local/bin/pangopup"]' Dockerfile
grep -Fq 'CMD ["serve", "--listen", "0.0.0.0:8080"]' Dockerfile
grep -Fq 'STOPSIGNAL SIGTERM' Dockerfile
if grep -Eq '^[[:space:]]*(VOLUME|HEALTHCHECK)[[:space:]]' Dockerfile; then
  printf 'Dockerfile must not declare VOLUME or HEALTHCHECK\n' >&2
  exit 1
fi
grep -Fq 'contents: read' .github/workflows/container.yml
if grep -Eq 'packages:[[:space:]]*write|docker/(login|build-push)-action|docker push' .github/workflows/container.yml; then
  printf 'container workflow must not have publication authority\n' >&2
  exit 1
fi

publish_workflow=.github/workflows/publish-container.yml
[[ -f "$publish_workflow" ]]
grep -Fxq '  workflow_dispatch:' "$publish_workflow"
if grep -Eq '^  (push|pull_request|schedule):' "$publish_workflow"; then
  printf 'container publication workflow must be manually dispatched only\n' >&2
  exit 1
fi
grep -Fq '          - stage' "$publish_workflow"
grep -Fq '          - finalize' "$publish_workflow"
grep -Fq 'group: pangopup-container-publication' "$publish_workflow"
grep -Fq 'cancel-in-progress: false' "$publish_workflow"
[[ "$(grep -Fc 'packages: write' "$publish_workflow")" == 2 ]]
grep -Fq 'packages: write' < <(sed -n '/^  stage-leaf:/,/^  aggregate-stage-receipt:/p' "$publish_workflow")
grep -Fq 'packages: write' < <(sed -n '/^  finalize-manifest:/,$p' "$publish_workflow")
if grep -Fq 'packages:' < <(sed -n '/^  qualify-public-leaf:/,/^  finalize-manifest:/p' "$publish_workflow"); then
  printf 'anonymous native finalization qualification must have no package permission\n' >&2
  exit 1
fi
grep -Fq 'ubuntu-24.04-arm' "$publish_workflow"
grep -Fq 'native_machine: x86_64' "$publish_workflow"
grep -Fq 'native_machine: aarch64' "$publish_workflow"
setup_buildx='docker/setup-buildx-action@e468171a9de216ec08956ac3ada2f0791b6bd435'
buildkit_image='moby/buildkit@sha256:87afb62ed6a762bb65b85d53819f3b341fb74a36d1fc0a1153a64f367637bfda'
[[ "$(grep -Fc "$setup_buildx" "$publish_workflow")" == 1 ]]
[[ "$(grep -Fc '          BUILDER: ${{ steps.buildx.outputs.name }}' "$publish_workflow")" == 2 ]]
grep -Fq '        id: buildx' "$publish_workflow"
grep -Fq '          driver: docker-container' "$publish_workflow"
grep -Fq "          driver-opts: image=$buildkit_image" "$publish_workflow"
grep -Fq 'docker buildx inspect "$BUILDER" --bootstrap' "$publish_workflow"
grep -Fq "grep -Eq '^Driver:[[:space:]]+docker-container$'" "$publish_workflow"
grep -Fq 'docker buildx build --builder "$BUILDER" --pull --provenance=false --sbom=false' "$publish_workflow"
grep -Fq 'push-by-digest=true,name-canonical=true,push=true,oci-mediatypes=true' "$publish_workflow"
if grep -Eq 'docker push|--tag .*stage-|\$IMAGE:stage-|ghcr[.]io/[^[:space:]]*:stage-' "$publish_workflow"; then
  printf 'stage leaves must be pushed by digest without persistent staging tags\n' >&2
  exit 1
fi
for action in $(sed -nE 's/^[[:space:]]*- uses: ([^ #]+).*/\1/p' "$publish_workflow"); do
  [[ "$action" =~ @[0-9a-f]{40}$ ]] || {
    printf 'publication workflow action is not pinned: %s\n' "$action" >&2
    exit 1
  }
done
[[ "$(grep -Fc 'test "$EVENT_SHA" = "$EXACT_COMMIT"' "$publish_workflow")" == 6 ]]
[[ "$(grep -Fc 'test "$WORKFLOW_SHA" = "$EXACT_COMMIT"' "$publish_workflow")" == 6 ]]
[[ "$(grep -Fc 'test "$(git rev-parse HEAD)" = "$EXACT_COMMIT"' "$publish_workflow")" == 6 ]]
[[ "$(grep -Fc 'test "$(git rev-parse origin/main)" = "$EXACT_COMMIT"' "$publish_workflow")" == 6 ]]
[[ "$(grep -Ec '^[[:space:]]+for tag in "\$VERSION" "v\$VERSION"; do$' "$publish_workflow")" == 2 ]]
grep -Fq 'pangopup-container-leaf-${{ matrix.architecture }}-${{ inputs.commit }}-${{ github.run_id }}' "$publish_workflow"
grep -Fq 'test "${#files[@]}" = 2' "$publish_workflow"
grep -Fq 'pangopup-container-stage-v1' "$publish_workflow"
grep -Fq 'test "$(jq -cS . "$RUNNER_TEMP/stage-receipt.json")" = "$(cat "$RUNNER_TEMP/stage-receipt.json")"' "$publish_workflow"
grep -Fq '"/repos/$GITHUB_REPOSITORY/actions/runs/$STAGE_RUN_ID"' "$publish_workflow"
grep -Fq '"/repos/$GITHUB_REPOSITORY/actions/runs/$STAGE_RUN_ID/jobs?filter=latest&per_page=100"' "$publish_workflow"
grep -Fq '.name == "aggregate-stage-receipt" and .conclusion == "success"' "$publish_workflow"
grep -Fq '"/repos/$GITHUB_REPOSITORY/actions/runs/$STAGE_RUN_ID/artifacts?per_page=100"' "$publish_workflow"
grep -Fq 'scripts/admit-container-stage-receipt.sh metadata' "$publish_workflow"
grep -Fq 'scripts/admit-container-stage-receipt.sh archive' "$publish_workflow"
grep -Fq 'test "$artifact_digest" = "sha256:$(sha256sum "$RUNNER_TEMP/receipt.zip" | cut -d'"'"' '"'"' -f1)"' "$publish_workflow"
grep -Fq 'keys == ["amd64","arm64","commit","mode","run_id","schema","workflow_sha"]' scripts/admit-container-stage-receipt.sh
grep -Fq 'docker logout ghcr.io >/dev/null 2>&1 || true' "$publish_workflow"
grep -Fq 'scripts/qualify-container.sh "$IMAGE@$digest"' "$publish_workflow"
revision_inspect='            test "$(docker image inspect --format '\''{{index .Config.Labels "org.opencontainers.image.revision"}}'\'' "$IMAGE@$digest")" = "$EXACT_COMMIT"'
version_inspect='            test "$(docker image inspect --format '\''{{index .Config.Labels "org.opencontainers.image.version"}}'\'' "$IMAGE@$digest")" = "$VERSION"'
grep -Fxq "$revision_inspect" "$publish_workflow"
grep -Fxq "$version_inspect" "$publish_workflow"
if grep -Fq '{{index .Config.Labels \"' "$publish_workflow"; then
  printf 'Docker label inspection templates must not pass literal backslashes\n' >&2
  exit 1
fi
grep -Fq 'docker buildx imagetools create' "$publish_workflow"
grep -Fq -- '--metadata-file "$RUNNER_TEMP/index-metadata.json"' "$publish_workflow"
grep -Fq 'application/vnd.oci.image.index.v1+json' "$publish_workflow"
[[ "$(grep -Fc 'scripts/require-container-tag-absent.sh "$tag" "$code"' "$publish_workflow")" == 2 ]]
[[ "$(grep -Fc 'Authorization: Bearer $registry_token' "$publish_workflow")" == 3 ]]
collision_accept='Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json'
[[ "$(grep -Fc "$collision_accept" "$publish_workflow")" == 2 ]]
grep -Fq '.[0].code == "MANIFEST_UNKNOWN"' scripts/require-container-tag-absent.sh
grep -Fq 'could not prove version tag %s absent: HTTP %s' scripts/require-container-tag-absent.sh
grep -Fq 'PREVIOUS_INDEX: sha256:ad1aa8c27cc61d107310f609cd63f8fcbaf591a4f9760db475384a0a71049de4' "$publish_workflow"
grep -Fq 'scripts/require-container-tag-digest.sh latest "$latest_code"' "$publish_workflow"
grep -Fq '"$RUNNER_TEMP/latest.headers" "$PREVIOUS_INDEX"' "$publish_workflow"
grep -Fq 'container tag %s no longer resolves to its reviewed predecessor' scripts/require-container-tag-digest.sh
second_absence_line=$(grep -nF '            assert_tag_absent "$tag"' "$publish_workflow" | tail -1 | cut -d: -f1)
create_line=$(grep -nF '          docker buildx imagetools create \' "$publish_workflow" | cut -d: -f1)
predecessor_line=$(grep -nF '          scripts/require-container-tag-digest.sh latest "$latest_code" \' "$publish_workflow" | cut -d: -f1)
[[ "$predecessor_line" -gt "$second_absence_line" && "$create_line" -gt "$predecessor_line" ]]
grep -Fq 'test "$(jq '\''[.manifests[].platform | (.os + "/" + .architecture)] | sort == ["linux/amd64","linux/arm64"]'\'' <<<"$raw")" = true' "$publish_workflow"
for annotation in source revision version licenses; do
  grep -Fq ".annotations.\"org.opencontainers.image.$annotation\"" "$publish_workflow"
done
if grep -Eq 'cosign|--attest|provenance=true|sbom=true|docker/(login|build-push)-action' "$publish_workflow"; then
  printf 'publication workflow introduced deferred supply-chain or mutable helper surface\n' >&2
  exit 1
fi

grep -Fq '[EXPECTED_REGISTRY_DIGEST]' scripts/qualify-container.sh
grep -Fq 'held-image-reference' scripts/qualify-container.sh
grep -Fq 'check=held-registry-digest' scripts/qualify-container.sh
grep -Fq 'length == 1 and .[0] == $held' scripts/qualify-container.sh
invalid_digest_work="target/container-invalid-digest-$$"
invalid_digest_error="target/container-invalid-digest-$$.err"
if scripts/qualify-container.sh example.invalid/pangopup@sha256:bad . /bin/true \
  "$invalid_digest_work" sha256:bad 2>"$invalid_digest_error"; then
  printf 'container qualification accepted an invalid held digest\n' >&2
  exit 1
else
  [[ $? == 2 ]]
fi
grep -Fq 'expected registry digest is invalid' "$invalid_digest_error"
rm -f -- "$invalid_digest_error"
bash tests/container-receipt-admission.sh
bash tests/container-tag-absence.sh
bash tests/container-tag-digest.sh

publication_record=planning/artifacts/051-public-container.md
grep -Fq 'State: **COMPLETE' "$publication_record"
grep -Fq 'Exact publication commit: `e2d3c2c89813cbdf54d2c76887113e8d68e44b4a`.' "$publication_record"
grep -Fq 'Successful finalize run: `30932912158`.' "$publication_record"
grep -Fq 'OCI index: `sha256:ad1aa8c27cc61d107310f609cd63f8fcbaf591a4f9760db475384a0a71049de4`.' "$publication_record"
grep -Fq 'https://github.com/orgs/genomoncology/packages/container/pangopup/settings' "$publication_record"
grep -Fq 'readonly STAGE_RUN_ID=REPLACE_WITH_EXACT_SUCCESSFUL_STAGE_RUN_ID' "$publication_record"
grep -Fq 'actions/runs/$STAGE_RUN_ID/artifacts?per_page=100' "$publication_record"
grep -Fq 'pangopup-container-stage-v1' "$publication_record"
grep -Fq 'Stage run `30929323700` and finalize run `30931154337` are now abandoned' "$publication_record"
grep -Fq 'it must not be reused after the quoting' "$publication_record"
grep -Fq 'Recovery requires the remediation commit to be pushed' "$publication_record"
grep -Fq 'Superseded stage `30929323700` and failed finalize `30931154337` were not reused.' "$publication_record"
[[ "$(grep -Fc 'ARTIFACT_DIGEST" = "sha256:$(sha256sum' "$publication_record")" == 2 ]]
[[ "$(grep -Fc 'admit-container-stage-receipt.sh" archive' "$publication_record")" == 2 ]]
grep -Fq 'DOCKER_CONFIG="$PUBLIC_DOCKER" docker buildx imagetools inspect' "$publication_record"
for annotation in source revision version licenses; do
  grep -Fq ".annotations.\"org.opencontainers.image.$annotation\"" "$publication_record"
done
if grep -Eqi '(authorization:|bearer |ghp_|github_pat_|signed[_ -]?url)' "$publication_record"; then
  printf 'public container record must not contain credential material\n' >&2
  exit 1
fi
grep -Fq 'ubuntu-24.04-arm' .github/workflows/container.yml
grep -Fq 'qualify-container-production.sh' .github/workflows/container.yml
if grep -Fq 'assets runtime install' scripts/qualify-container-production.sh; then
  printf 'runtime-only qualification must use explicit authenticated assets\n' >&2
  exit 1
fi
grep -Fq 'find "$runtime" -type d -exec chmod 0555 {} +' scripts/qualify-container-production.sh
grep -Fq -- '--slurpfile expected "$oracle"' scripts/qualify-container-production.sh
grep -Fq 'all(.[]; .provenance == $expected[0].provenance)' scripts/qualify-container-production.sh
grep -Fq '691874664' scripts/qualify-container-production.sh
grep -Fq -- '-v "$cache:/var/cache/pangopup"' scripts/qualify-container-production.sh
if grep -Fq -- '-v "$cache:/cache"' scripts/qualify-container-production.sh; then
  printf 'production cache must use the image-prepared writable directory\n' >&2
  exit 1
fi
grep -Fq 'cmp "$work/cache-before.sqlite3" "$work/cache-after.sqlite3"' scripts/qualify-container.sh
grep -Fq 'container qualification failed: stage=%s check=%s' scripts/qualify-container.sh
grep -Fq "expect_equal exposed-ports '{\"8080/tcp\":{}}'" scripts/qualify-container.sh
grep -Fq 'stage=read-only-installed-status' scripts/qualify-container.sh
grep -Fq 'stage=focused-help-no-assets' scripts/qualify-container.sh
grep -Fq 'check_focused_help assets-runtime-install' scripts/qualify-container.sh
grep -Fq 'help_run=(docker run --rm --network none --read-only' scripts/qualify-container.sh
sync_help_check=$(grep '^check_focused_help sync ' scripts/qualify-container.sh)
expected_sync_usage='Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]'
expected_sync_help_check="check_focused_help sync '$expected_sync_usage' sync"
if [[ "$sync_help_check" != "$expected_sync_help_check" ]]; then
  printf 'container qualification must assert the exact shipped sync usage line\n' >&2
  exit 1
fi
eval "$(sed -n '/^expect_equal() {/,/^}/p' scripts/qualify-container.sh)"
(stage=sync-help-negative-control; expect_equal sync "$expected_sync_usage" "$expected_sync_usage")
for rejected_sync_usage in \
  'Usage: pangopup sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]' \
  'Usage: pangopup sync [OPTIONS]'; do
  if (stage=sync-help-negative-control; expect_equal sync "$expected_sync_usage" "$rejected_sync_usage") \
    >/dev/null 2>&1; then
    printf 'container qualification accepted stale or broadened sync usage\n' >&2
    exit 1
  fi
done
if sed -n '/stage=focused-help-no-assets/,/stage=filesystem-inventory/p' scripts/qualify-container.sh \
  | grep -Fq -- '-v '; then
  printf 'container help qualification must not mount assets or caches\n' >&2
  exit 1
fi
grep -Fq -- '-v "$data_volume:/var/lib/pangopup:ro"' scripts/qualify-container.sh
grep -Fq '"runtime":{"status":"missing"}' scripts/qualify-container.sh
[[ "$(jq '.results | length' tests/fixtures/container-qualification/production-model-oracle.json)" == 14 ]]
[[ "$(jq -r '.results[].records[].loss_score' tests/fixtures/container-qualification/production-model-oracle.json | grep -Fxc -- '-0.08')" == 1 ]]
