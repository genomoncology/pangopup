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
