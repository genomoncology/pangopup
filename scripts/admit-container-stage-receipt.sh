#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf 'container stage receipt rejected: %s\n' "$1" >&2
  exit 1
}

[[ $# -ge 1 ]] || fail 'missing action'
action=$1
shift

case "$action" in
  metadata)
    [[ $# == 2 ]] || fail 'usage: metadata <ARTIFACTS_JSON> <EXPECTED_NAME>'
    artifacts=$1
    expected_name=$2
    [[ -f "$artifacts" && ! -L "$artifacts" ]] || fail 'artifact metadata is not a direct regular file'
    jq -e 'type == "array"' "$artifacts" >/dev/null || fail 'artifact metadata is not an array'
    count=$(jq -r --arg name "$expected_name" '[.[] | select(.name == $name)] | length' "$artifacts")
    [[ "$count" == 1 ]] || fail 'expected exactly one named artifact'
    id=$(jq -er --arg name "$expected_name" '.[] | select(.name == $name) | .id' "$artifacts") ||
      fail 'artifact ID is missing'
    digest=$(jq -er --arg name "$expected_name" '.[] | select(.name == $name) | .digest' "$artifacts") ||
      fail 'artifact digest is missing'
    expired=$(jq -r --arg name "$expected_name" '.[] | select(.name == $name) | .expired' "$artifacts") ||
      fail 'artifact expiry state is missing'
    [[ "$id" =~ ^[1-9][0-9]*$ ]] || fail 'artifact ID is invalid'
    [[ "$digest" =~ ^sha256:[0-9a-f]{64}$ ]] || fail 'artifact digest is invalid'
    [[ "$expired" == false ]] || fail 'artifact is expired'
    printf '%s\t%s\n' "$id" "$digest"
    ;;
  archive)
    [[ $# == 7 ]] || fail 'usage: archive <ARTIFACTS_JSON> <EXPECTED_NAME> <ARCHIVE_ZIP> <COMMIT> <WORKFLOW_SHA> <RUN_ID> <ABSENT_OUTPUT>'
    artifacts=$1
    expected_name=$2
    archive=$3
    commit=$4
    workflow_sha=$5
    run_id=$6
    output=$7
    [[ "$commit" =~ ^[0-9a-f]{40}$ ]] || fail 'expected commit is invalid'
    [[ "$workflow_sha" =~ ^[0-9a-f]{40}$ ]] || fail 'expected workflow SHA is invalid'
    [[ "$run_id" =~ ^[1-9][0-9]*$ ]] || fail 'expected run ID is invalid'
    [[ -f "$archive" && ! -L "$archive" ]] || fail 'artifact archive is not a direct regular file'
    [[ ! -e "$output" && ! -L "$output" ]] || fail 'receipt output already exists'
    IFS=$'\t' read -r _ expected_digest < <("$0" metadata "$artifacts" "$expected_name")
    observed_digest="sha256:$(sha256sum "$archive" | cut -d' ' -f1)"
    [[ "$observed_digest" == "$expected_digest" ]] || fail 'artifact archive digest does not match Actions metadata'
    mapfile -t members < <(unzip -Z1 "$archive")
    [[ "${#members[@]}" == 1 && "${members[0]}" == stage-receipt.json ]] ||
      fail 'artifact archive inventory is not canonical'
    scratch=$(mktemp -d)
    chmod 0700 "$scratch"
    cleanup() { rm -rf -- "$scratch"; }
    trap cleanup EXIT
    unzip -q "$archive" -d "$scratch"
    receipt=$scratch/stage-receipt.json
    [[ -f "$receipt" && ! -L "$receipt" ]] || fail 'receipt is not a direct regular file'
    [[ "$(find "$scratch" -mindepth 1 -maxdepth 1 | wc -l)" == 1 ]] ||
      fail 'extracted artifact inventory is not canonical'
    [[ "$(wc -l <"$receipt")" == 1 && "$(stat -c %s "$receipt")" -le 1024 ]] ||
      fail 'receipt size or line count is invalid'
    [[ "$(jq -cS . "$receipt")" == "$(cat "$receipt")" ]] || fail 'receipt JSON is not canonical'
    jq -e --arg commit "$commit" --arg workflow_sha "$workflow_sha" --argjson run_id "$run_id" \
      'keys == ["amd64","arm64","commit","mode","run_id","schema","workflow_sha"] and
       .schema == "pangopup-container-stage-v1" and .mode == "stage" and
       .commit == $commit and .workflow_sha == $workflow_sha and .run_id == $run_id and
       (.amd64 | test("^sha256:[0-9a-f]{64}$")) and
       (.arm64 | test("^sha256:[0-9a-f]{64}$")) and .amd64 != .arm64' \
      "$receipt" >/dev/null || fail 'receipt identity or schema is invalid'
    cp --update=none -- "$receipt" "$output" || fail 'receipt output publication failed'
    [[ -f "$output" && ! -L "$output" ]] || fail 'receipt output is not a direct regular file'
    cmp "$receipt" "$output" || fail 'receipt output identity changed'
    chmod 0600 "$output"
    ;;
  *)
    fail 'unknown action'
    ;;
esac
