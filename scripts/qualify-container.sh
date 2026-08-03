#!/usr/bin/env bash
set -euo pipefail

[[ $# == 4 ]] || {
  printf 'usage: qualify-container.sh <IMAGE> <SOURCE_TREE> <PANGOPUP_BUILD> <WORK_DIR>\n' >&2
  exit 2
}

image=$1
source_tree=$(realpath "$2")
pangopup_build=$(realpath "$3")
work=$(realpath -m "$4")
[[ -d "$source_tree" && -x "$pangopup_build" && ! -e "$work" ]] || {
  printf 'container qualification inputs are invalid\n' >&2
  exit 2
}

case "$(uname -m)" in
  x86_64) expected_arch=amd64 ;;
  aarch64) expected_arch=arm64 ;;
  *) printf 'container qualification requires native AMD64 or ARM64\n' >&2; exit 2 ;;
esac

prefix="pangopup-q-$PPID-$$"
data_volume="$prefix-data"
cache_volume="$prefix-cache"
empty_volume="$prefix-empty"
container="$prefix-inventory"
cache_copy_container="$prefix-cache-copy"
owned=0
cleanup() {
  if ((owned)); then
    docker rm --force "$container" >/dev/null 2>&1 || true
    docker rm --force "$cache_copy_container" >/dev/null 2>&1 || true
    docker volume rm --force "$data_volume" "$cache_volume" "$empty_volume" >/dev/null 2>&1 || true
    rm -rf -- "$work"
  fi
}
trap cleanup EXIT
umask 077
mkdir "$work"
owned=1

run=(docker run --rm --network none --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m
  -v "$data_volume:/var/lib/pangopup"
  -v "$cache_volume:/var/cache/pangopup")

copy_cache_database() {
  local destination=$1
  docker create --name "$cache_copy_container" --read-only \
    -v "$cache_volume:/var/cache/pangopup:ro" "$image" --version >/dev/null
  docker cp "$cache_copy_container:/var/cache/pangopup/model-results.sqlite3" \
    "$work/$destination"
  docker rm "$cache_copy_container" >/dev/null
}

[[ "$(docker image inspect --format '{{.Architecture}}' "$image")" == "$expected_arch" ]]
[[ "$(docker image inspect --format '{{.Config.User}}' "$image")" == '65532:65532' ]]
[[ "$(docker image inspect --format '{{json .Config.Entrypoint}}' "$image")" == '["/usr/local/bin/pangopup"]' ]]
[[ "$(docker image inspect --format '{{json .Config.Cmd}}' "$image")" == '["serve","--listen","0.0.0.0:8080"]' ]]
[[ "$(docker image inspect --format '{{.Config.StopSignal}}' "$image")" == 'SIGTERM' ]]
[[ "$(docker image inspect --format '{{index .Config.ExposedPorts "8080/tcp"}}' "$image")" == '{}' ]]
[[ "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.title"}}' "$image")" == 'Pangopup' ]]
[[ "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.source"}}' "$image")" == 'https://github.com/genomoncology/pangopup' ]]
[[ "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.licenses"}}' "$image")" == 'GPL-3.0-only' ]]
[[ "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$image")" == "$(git -C "$source_tree" rev-parse HEAD)" ]]
[[ "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.version"}}' "$image")" == "$(sed -nE 's/^version = "([^"]+)"$/\1/p' "$source_tree/Cargo.toml" | head -1)" ]]
environment=$(docker image inspect --format '{{json .Config.Env}}' "$image")
grep -Fq 'PANGOPUP_DATA_DIR=/var/lib/pangopup' <<<"$environment"
grep -Fq 'PANGOPUP_CACHE_DIR=/var/cache/pangopup' <<<"$environment"
grep -Fq 'PANGOPUP_MODEL_CACHE=/var/cache/pangopup/model-results.sqlite3' <<<"$environment"
size=$(docker image inspect --format '{{.Size}}' "$image")
((size <= 78643200))

docker create --name "$container" "$image" >/dev/null
docker export "$container" >"$work/rootfs.tar"
if tar -tf "$work/rootfs.tar" | grep -E '(^|/)(\.git|target|\.venv)(/|$)|\.(pgi|pgr|pgm|onnx)$|(^|/)(bin/(ba)?sh|usr/bin/(apt|apt-get|dpkg|apk))$' >/dev/null; then
  printf 'final image contains a forbidden build, asset, shell, or package-manager path\n' >&2
  exit 1
fi

docker volume create "$data_volume" >/dev/null
docker volume create "$cache_volume" >/dev/null
docker volume create "$empty_volume" >/dev/null

"${run[@]}" "$image" --version | grep -Fxq 'pangopup 0.1.0'
"${run[@]}" "$image" status | grep -Fq '"status":"missing"'

if docker run --rm --network none --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  -v "$empty_volume:/var/lib/pangopup" -v "$cache_volume:/var/cache/pangopup" \
  "$image" >"$work/default.out" 2>"$work/default.err"; then
  printf 'default service unexpectedly started without installed assets\n' >&2
  exit 1
fi
grep -Fq 'run pangopup sync' "$work/default.err"

"$pangopup_build" transport pack \
  --bundle "$source_tree/tests/fixtures/snv-regression/bundle" \
  --output "$work/snv-transport" >/dev/null
chmod -R a+rX "$work/snv-transport"
"${run[@]}" -v "$work/snv-transport:/fixtures/snv-transport:ro" \
  "$image" assets install --transport /fixtures/snv-transport >/dev/null
"${run[@]}" "$image" lookup --variant GRCh38:chr12:6801301:G:A \
  | grep -Fq '"kind":"precomputed"'

model_args=(lookup --model-only --format jsonl
  --variant GRCh38:chr1:5051:A:AC
  --model-bundle /fixtures/model
  --reference-bundle /fixtures/reference
  --mask /fixtures/mask/domains.pgm)
fixture_mounts=(-v "$source_tree/tests/fixtures/pangolin-model-kernel-mini/bundle:/fixtures/model:ro"
  -v "$source_tree/tests/fixtures/reference-route-test/bundle:/fixtures/reference:ro"
  -v "$source_tree/tests/fixtures/route-mask:/fixtures/mask:ro")
"${run[@]}" "${fixture_mounts[@]}" "$image" "${model_args[@]}" >"$work/model-first.jsonl"
copy_cache_database cache-before.sqlite3
[[ -s "$work/cache-before.sqlite3" ]]
[[ "$(head -c 15 "$work/cache-before.sqlite3")" == 'SQLite format 3' ]]
"${run[@]}" "${fixture_mounts[@]}" "$image" "${model_args[@]}" >"$work/model-second.jsonl"
copy_cache_database cache-after.sqlite3
cmp "$work/model-first.jsonl" "$work/model-second.jsonl"
cmp "$work/cache-before.sqlite3" "$work/cache-after.sqlite3"
grep -Fq '"kind":"model"' "$work/model-first.jsonl"

printf 'container qualified architecture=%s image_size=%s\n' "$expected_arch" "$size"
