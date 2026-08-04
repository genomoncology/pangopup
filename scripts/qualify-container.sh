#!/usr/bin/env bash
set -euo pipefail

stage=argument-validation
report_failure() {
  local status=$1 line=$2 command=$3
  trap - ERR
  printf 'container qualification failed: stage=%s line=%s exit=%s command=%q\n' \
    "$stage" "$line" "$status" "$command" >&2
  exit "$status"
}
trap 'report_failure "$?" "$LINENO" "$BASH_COMMAND"' ERR

expect_equal() {
  local name=$1 expected=$2 observed=$3
  if [[ "$observed" != "$expected" ]]; then
    printf 'container qualification failed: stage=%s check=%s expected=%q observed=%q\n' \
      "$stage" "$name" "$expected" "$observed" >&2
    exit 1
  fi
}

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
read_only_data_run=(docker run --rm --network none --read-only
  --tmpfs /tmp:rw,noexec,nosuid,size=64m
  -v "$data_volume:/var/lib/pangopup:ro"
  -v "$cache_volume:/var/cache/pangopup")
help_run=(docker run --rm --network none --read-only
  --tmpfs /tmp:rw,noexec,nosuid,size=64m)

check_focused_help() {
  local name=$1 expected=$2
  shift 2
  local flag suffix
  for flag in -h --help; do
    suffix=${flag#-}
    "${help_run[@]}" "$image" "$@" "$flag" \
      >"$work/help-$name-$suffix.out" 2>"$work/help-$name-$suffix.err"
    expect_equal "$name-$suffix-first-line" "$expected" \
      "$(head -1 "$work/help-$name-$suffix.out")"
    if grep -Fq '"status":"error"' "$work/help-$name-$suffix.err"; then
      printf 'container qualification failed: stage=%s check=%s emitted PangoPup JSON error\n' \
        "$stage" "$name-$suffix" >&2
      exit 1
    fi
  done
}

copy_cache_database() {
  local destination=$1
  docker create --name "$cache_copy_container" --read-only \
    -v "$cache_volume:/var/cache/pangopup:ro" "$image" --version >/dev/null
  docker cp "$cache_copy_container:/var/cache/pangopup/model-results.sqlite3" \
    "$work/$destination"
  docker rm "$cache_copy_container" >/dev/null
}

stage=image-metadata
expect_equal architecture "$expected_arch" "$(docker image inspect --format '{{.Architecture}}' "$image")"
expect_equal user '65532:65532' "$(docker image inspect --format '{{.Config.User}}' "$image")"
expect_equal entrypoint '["/usr/local/bin/pangopup"]' "$(docker image inspect --format '{{json .Config.Entrypoint}}' "$image")"
expect_equal command '["serve","--listen","0.0.0.0:8080"]' "$(docker image inspect --format '{{json .Config.Cmd}}' "$image")"
expect_equal stop-signal SIGTERM "$(docker image inspect --format '{{.Config.StopSignal}}' "$image")"
expect_equal exposed-ports '{"8080/tcp":{}}' "$(docker image inspect --format '{{json .Config.ExposedPorts}}' "$image")"
expect_equal title Pangopup "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.title"}}' "$image")"
expect_equal source https://github.com/genomoncology/pangopup "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.source"}}' "$image")"
expect_equal license GPL-3.0-only "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.licenses"}}' "$image")"
expect_equal revision "$(git -C "$source_tree" rev-parse HEAD)" "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$image")"
expect_equal version "$(sed -nE 's/^version = "([^"]+)"$/\1/p' "$source_tree/Cargo.toml" | head -1)" "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.version"}}' "$image")"
environment=$(docker image inspect --format '{{json .Config.Env}}' "$image")
grep -Fq 'PANGOPUP_DATA_DIR=/var/lib/pangopup' <<<"$environment"
grep -Fq 'PANGOPUP_CACHE_DIR=/var/cache/pangopup' <<<"$environment"
grep -Fq 'PANGOPUP_MODEL_CACHE=/var/cache/pangopup/model-results.sqlite3' <<<"$environment"
size=$(docker image inspect --format '{{.Size}}' "$image")
[[ "$size" =~ ^[0-9]+$ ]] || {
  printf 'container qualification failed: stage=%s check=image-size observed=%q\n' \
    "$stage" "$size" >&2
  exit 1
}
((size <= 78643200)) || {
  printf 'container qualification failed: stage=%s check=image-size limit=%s observed=%s\n' \
    "$stage" 78643200 "$size" >&2
  exit 1
}

stage=focused-help-no-assets
check_focused_help sync 'Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]' sync
check_focused_help status 'Usage: pangopup status [--data-dir <ABSOLUTE_PATH>]' status
check_focused_help serve 'Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]' serve
check_focused_help assets 'Usage: pangopup assets <ACTION>' assets
check_focused_help assets-install 'Usage: pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]' assets install
check_focused_help assets-runtime 'Usage: pangopup assets runtime <ACTION>' assets runtime
check_focused_help assets-runtime-install 'Usage: pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]' assets runtime install
check_focused_help lookup 'Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]' lookup

stage=filesystem-inventory
docker create --name "$container" "$image" >/dev/null
docker export "$container" >"$work/rootfs.tar"
if tar -tf "$work/rootfs.tar" | grep -E '(^|/)(\.git|target|\.venv)(/|$)|\.(pgi|pgr|pgm|onnx)$|(^|/)(bin/(ba)?sh|usr/bin/(apt|apt-get|dpkg|apk))$' >/dev/null; then
  printf 'final image contains a forbidden build, asset, shell, or package-manager path\n' >&2
  exit 1
fi

docker volume create "$data_volume" >/dev/null
docker volume create "$cache_volume" >/dev/null
docker volume create "$empty_volume" >/dev/null

stage=empty-volume-smoke
expected_version=$(sed -nE 's/^version = "([^"]+)"$/\1/p' "$source_tree/Cargo.toml" | head -1)
"${run[@]}" "$image" --version | grep -Fxq "pangopup $expected_version"
"${run[@]}" "$image" status | grep -Fq '"status":"missing"'

if docker run --rm --network none --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  -v "$empty_volume:/var/lib/pangopup" -v "$cache_volume:/var/cache/pangopup" \
  "$image" >"$work/default.out" 2>"$work/default.err"; then
  printf 'default service unexpectedly started without installed assets\n' >&2
  exit 1
fi
grep -Fq 'run pangopup sync' "$work/default.err"

stage=miniature-snv-install
"$pangopup_build" transport pack \
  --bundle "$source_tree/tests/fixtures/snv-regression/bundle" \
  --output "$work/snv-transport" >/dev/null
chmod -R a+rX "$work/snv-transport"
"${run[@]}" -v "$work/snv-transport:/fixtures/snv-transport:ro" \
  "$image" assets install --transport /fixtures/snv-transport >/dev/null
"${run[@]}" "$image" lookup --variant GRCh38:chr12:6801301:G:A \
  | grep -Fq '"kind":"precomputed"'

stage=read-only-installed-status
"${read_only_data_run[@]}" "$image" status >"$work/read-only-status.json"
grep -Fq '"status":"partial"' "$work/read-only-status.json"
grep -Fq '"installing":false' "$work/read-only-status.json"
grep -Fq '"snv":{"status":"ready"' "$work/read-only-status.json"
grep -Fq '"runtime":{"status":"missing"}' "$work/read-only-status.json"

stage=miniature-model-cache
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
