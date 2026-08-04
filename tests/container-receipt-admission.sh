#!/usr/bin/env bash
set -euo pipefail

root="target/container-receipt-admission-$$"
[[ ! -e "$root" && ! -L "$root" ]]
mkdir -p "$root/good" "$root/bad"
cleanup() { rm -rf -- "$root"; }
trap cleanup EXIT

helper=scripts/admit-container-stage-receipt.sh
name=pangopup-container-stage-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-123
commit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
amd64=sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
arm64=sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
jq -cnS --arg amd64 "$amd64" --arg arm64 "$arm64" --arg commit "$commit" \
  '{amd64:$amd64,arm64:$arm64,commit:$commit,mode:"stage",run_id:123,schema:"pangopup-container-stage-v1",workflow_sha:$commit}' \
  >"$root/good/stage-receipt.json"
(cd "$root/good" && zip -q ../good.zip stage-receipt.json)
good_digest="sha256:$(sha256sum "$root/good.zip" | cut -d' ' -f1)"
jq -cn --arg name "$name" --arg digest "$good_digest" \
  '[{id:456,name:$name,expired:false,digest:$digest}]' >"$root/good-artifacts.json"

test "$("$helper" metadata "$root/good-artifacts.json" "$name")" = $'456\t'"$good_digest"
"$helper" archive "$root/good-artifacts.json" "$name" "$root/good.zip" \
  "$commit" "$commit" 123 "$root/admitted.json"
cmp "$root/good/stage-receipt.json" "$root/admitted.json"

expect_rejected() {
  local label=$1
  shift
  if "$@" >"$root/$label.out" 2>"$root/$label.err"; then
    printf 'container receipt admission accepted %s\n' "$label" >&2
    exit 1
  fi
  grep -Fq 'container stage receipt rejected:' "$root/$label.err"
}

printf '[]\n' >"$root/missing.json"
expect_rejected missing "$helper" metadata "$root/missing.json" "$name"

jq -cn --arg name "$name" '[{id:456,name:$name,expired:false,digest:"sha256:bad"}]' \
  >"$root/malformed-digest.json"
expect_rejected malformed-digest "$helper" metadata "$root/malformed-digest.json" "$name"

jq -cn --arg name "$name" --arg digest "$good_digest" \
  '[{id:456,name:$name,expired:false,digest:$digest},{id:789,name:$name,expired:false,digest:$digest}]' \
  >"$root/duplicate.json"
expect_rejected duplicate "$helper" metadata "$root/duplicate.json" "$name"

jq -cn --arg name "$name" --arg digest "$good_digest" \
  '[{id:456,name:$name,expired:true,digest:$digest}]' >"$root/expired.json"
expect_rejected expired "$helper" metadata "$root/expired.json" "$name"

cp "$root/good.zip" "$root/corrupt.zip"
printf 'corruption' >>"$root/corrupt.zip"
expect_rejected corrupt-archive "$helper" archive "$root/good-artifacts.json" "$name" \
  "$root/corrupt.zip" "$commit" "$commit" 123 "$root/corrupt-output.json"

printf '{"bad":true}\n' >"$root/bad/stage-receipt.json"
(cd "$root/bad" && zip -q ../bad.zip stage-receipt.json)
bad_digest="sha256:$(sha256sum "$root/bad.zip" | cut -d' ' -f1)"
jq -cn --arg name "$name" --arg digest "$bad_digest" \
  '[{id:456,name:$name,expired:false,digest:$digest}]' >"$root/bad-artifacts.json"
expect_rejected malformed-receipt "$helper" archive "$root/bad-artifacts.json" "$name" \
  "$root/bad.zip" "$commit" "$commit" 123 "$root/bad-output.json"
