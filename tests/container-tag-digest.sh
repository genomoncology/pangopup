#!/usr/bin/env bash
set -euo pipefail

root="target/container-tag-digest-$$"
[[ ! -e "$root" && ! -L "$root" ]]
mkdir -p "$root"
cleanup() { rm -rf -- "$root"; }
trap cleanup EXIT

helper=scripts/require-container-tag-digest.sh
expected=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
printf 'HTTP/1.1 200 OK\r\nDocker-Content-Digest: %s\r\nContent-Type: application/vnd.oci.image.index.v1+json\r\n\r\n' \
  "$expected" >"$root/good.headers"
"$helper" latest 200 "$root/good.headers" "$expected"
printf 'docker-content-digest: %s\r\n' "$expected" >"$root/lowercase.headers"
"$helper" latest 200 "$root/lowercase.headers" "$expected"

expect_rejected() {
  local label=$1
  shift
  if "$@" >"$root/$label.out" 2>"$root/$label.err"; then
    printf 'container tag digest check accepted %s\n' "$label" >&2
    exit 1
  fi
}

other=sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
expect_rejected changed "$helper" latest 200 "$root/good.headers" "$other"
expect_rejected unauthorized "$helper" latest 401 "$root/good.headers" "$expected"
expect_rejected missing "$helper" latest 404 "$root/good.headers" "$expected"
: >"$root/missing.headers"
expect_rejected no-digest "$helper" latest 200 "$root/missing.headers" "$expected"
printf 'Docker-Content-Digest: %s\r\nDocker-Content-Digest: %s\r\n' \
  "$expected" "$expected" >"$root/duplicate.headers"
expect_rejected duplicate "$helper" latest 200 "$root/duplicate.headers" "$expected"
printf 'Docker-Content-Digest: not-a-digest\r\n' >"$root/malformed.headers"
expect_rejected malformed "$helper" latest 200 "$root/malformed.headers" "$expected"
ln -s good.headers "$root/link.headers"
expect_rejected linked "$helper" latest 200 "$root/link.headers" "$expected"
expect_rejected bad-expected "$helper" latest 200 "$root/good.headers" sha256:bad
