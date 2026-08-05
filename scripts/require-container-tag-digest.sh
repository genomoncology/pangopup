#!/usr/bin/env bash
set -euo pipefail

[[ $# == 4 ]] || {
  printf 'usage: require-container-tag-digest.sh <TAG> <HTTP_STATUS> <RESPONSE_HEADERS> <EXPECTED_DIGEST>\n' >&2
  exit 2
}

tag=$1
status=$2
headers=$3
expected=$4
[[ "$tag" =~ ^[A-Za-z0-9_][A-Za-z0-9._-]{0,127}$ ]] || {
  printf 'container tag is invalid\n' >&2
  exit 2
}
[[ "$status" =~ ^[0-9]{3}$ && -f "$headers" && ! -L "$headers" ]] || {
  printf 'container tag response inputs are invalid\n' >&2
  exit 2
}
[[ "$expected" =~ ^sha256:[0-9a-f]{64}$ ]] || {
  printf 'expected container digest is invalid\n' >&2
  exit 2
}
[[ "$status" == 200 ]] || {
  printf 'could not authenticate container tag %s: HTTP %s\n' "$tag" "$status" >&2
  exit 1
}

mapfile -t observed < <(
  awk 'tolower($0) ~ /^docker-content-digest:[[:space:]]*/ {
      sub(/^[^:]*:[[:space:]]*/, "")
      sub(/\r$/, "")
      print
    }' "$headers"
)
[[ "${#observed[@]}" == 1 && "${observed[0]}" =~ ^sha256:[0-9a-f]{64}$ ]] || {
  printf 'container tag %s returned an invalid digest header\n' "$tag" >&2
  exit 1
}
[[ "${observed[0]}" == "$expected" ]] || {
  printf 'container tag %s no longer resolves to its reviewed predecessor\n' "$tag" >&2
  exit 1
}
