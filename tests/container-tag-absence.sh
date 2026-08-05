#!/usr/bin/env bash
set -euo pipefail

root="target/container-tag-absence-$$"
[[ ! -e "$root" && ! -L "$root" ]]
mkdir -p "$root"
cleanup() { rm -rf -- "$root"; }
trap cleanup EXIT

helper=scripts/require-container-tag-absent.sh
printf '{"errors":[{"code":"MANIFEST_UNKNOWN","message":"missing"}]}\n' >"$root/missing.json"
"$helper" 0.3.0 404 "$root/missing.json"

expect_rejected() {
  local label=$1
  shift
  if "$@" >"$root/$label.out" 2>"$root/$label.err"; then
    printf 'container tag absence accepted %s\n' "$label" >&2
    exit 1
  fi
}

expect_rejected existing "$helper" 0.3.0 200 "$root/missing.json"
expect_rejected unauthorized "$helper" 0.3.0 401 "$root/missing.json"
expect_rejected server-error "$helper" 0.3.0 500 "$root/missing.json"
printf '{"errors":[{"code":"DENIED"}]}\n' >"$root/denied.json"
expect_rejected disguised-denial "$helper" 0.3.0 404 "$root/denied.json"
printf 'not-json\n' >"$root/not-json"
expect_rejected malformed-404 "$helper" 0.3.0 404 "$root/not-json"
