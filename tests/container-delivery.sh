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
[[ "$(jq '.results | length' tests/fixtures/container-qualification/production-model-oracle.json)" == 14 ]]
[[ "$(jq -r '.results[].records[].loss_score' tests/fixtures/container-qualification/production-model-oracle.json | grep -Fxc -- '-0.08')" == 1 ]]
