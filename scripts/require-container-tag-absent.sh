#!/usr/bin/env bash
set -euo pipefail

[[ $# == 3 ]] || {
  printf 'usage: require-container-tag-absent.sh <TAG> <HTTP_STATUS> <RESPONSE_JSON>\n' >&2
  exit 2
}

tag=$1
status=$2
response=$3
[[ "$tag" =~ ^[A-Za-z0-9_][A-Za-z0-9._-]{0,127}$ ]] || {
  printf 'container version tag is invalid\n' >&2
  exit 2
}
[[ "$status" =~ ^[0-9]{3}$ && -f "$response" && ! -L "$response" ]] || {
  printf 'container tag response inputs are invalid\n' >&2
  exit 2
}

case "$status" in
  404)
    jq -e '.errors | type == "array" and length == 1 and .[0].code == "MANIFEST_UNKNOWN"' \
      "$response" >/dev/null || {
      printf 'registry returned a noncanonical not-found response for %s\n' "$tag" >&2
      exit 1
    }
    ;;
  200)
    printf 'refusing to replace existing version tag %s\n' "$tag" >&2
    exit 1
    ;;
  *)
    printf 'could not prove version tag %s absent: HTTP %s\n' "$tag" "$status" >&2
    exit 1
    ;;
esac
