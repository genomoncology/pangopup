#!/usr/bin/env bash
set -euo pipefail

[[ $# == 5 ]] || {
  printf 'usage: qualify-container-production.sh <IMAGE> <SOURCE_TREE> <PANGOPUP_BUILD> <DOWNLOAD_DIR> <OUTPUT_JSONL>\n' >&2
  exit 2
}

image=$1
source_tree=$(realpath "$2")
pangopup_build=$(realpath "$3")
download=$(realpath -m "$4")
output=$(realpath -m "$5")
[[ -d "$source_tree" && -x "$pangopup_build" && ! -e "$download" && ! -e "$output" ]] || {
  printf 'production qualification inputs are invalid\n' >&2
  exit 2
}

case "$(uname -m)" in
  x86_64) expected_arch=amd64 ;;
  aarch64) expected_arch=arm64 ;;
  *) printf 'production qualification requires native AMD64 or ARM64\n' >&2; exit 2 ;;
esac
[[ "$(docker image inspect --format '{{.Architecture}}' "$image")" == "$expected_arch" ]]

umask 077
mkdir "$download"
profile="$source_tree/release-profiles/runtime-release-profile.json"
while IFS=$'\t' read -r name bytes digest url; do
  [[ "$name" =~ ^[A-Za-z0-9._-]+$ && "$digest" =~ ^[0-9a-f]{64}$ && "$bytes" =~ ^[0-9]+$ ]]
  curl --fail --location --silent --show-error --output "$download/$name" "$url"
  [[ "$(stat -c %s "$download/$name")" == "$bytes" ]]
  printf '%s  %s\n' "$digest" "$download/$name" | sha256sum --check --strict >/dev/null
done < <(jq -r '.transport.members[] | [.asset_name, (.size|tostring), (.sha256|sub("^sha256:";"")), .url] | @tsv' "$profile")
[[ "$(find "$download" -maxdepth 1 -type f | wc -l)" == 10 ]]
[[ "$(find "$download" -maxdepth 1 -type f -printf '%s\n' | awk '{n += $1} END {print n}')" == 691874664 ]]

runtime="$download-decoded"
[[ ! -e "$runtime" ]]
"$pangopup_build" runtime-transport unpack --transport "$download" --output "$runtime" >/dev/null
find "$runtime" -type d -exec chmod 0555 {} +
find "$runtime" -type f -exec chmod 0444 {} +

args=(lookup --model-only --format jsonl
  --model-bundle /runtime/model --reference-bundle /runtime/reference
  --mask /runtime/mask/domains.pgm)
oracle="$source_tree/tests/fixtures/container-qualification/production-model-oracle.json"
while IFS= read -r variant; do args+=(--variant "$variant"); done < <(
  jq -r '.results[] | "GRCh38:\(.contig):\(.position):\(.ref):\(.alt)"' \
    "$oracle"
)

cache="pangopup-production-q-$PPID-$$-cache"
cleanup() { docker volume rm --force "$cache" >/dev/null 2>&1 || true; }
trap cleanup EXIT
docker volume create "$cache" >/dev/null
docker run --rm --network none --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  -v "$runtime:/runtime:ro" -v "$cache:/var/cache/pangopup" \
  "$image" "${args[@]}" >"$output"
[[ "$(wc -l <"$output")" == 14 ]]
jq -e --slurpfile expected "$oracle" \
  'length == 14 and
   all(.[]; .provenance == $expected[0].provenance) and
   map(del(.provenance)) == $expected[0].results' \
  < <(jq -s . "$output") >/dev/null
printf 'production container qualified architecture=%s cases=14\n' "$expected_arch"
