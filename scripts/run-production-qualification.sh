#!/usr/bin/env bash
set -euo pipefail

[[ $# == 5 || ( $# == 6 && $6 == --reuse-installed ) ]] || {
  printf 'usage: run-production-qualification.sh <PANGOPUP> <SOURCE_TREE> <XDG_DATA_HOME> <XDG_CACHE_HOME> <ABSENT_OUTPUT_DIR> [--reuse-installed]\n' >&2
  exit 2
}

pangopup=$1
source_tree=$2
xdg_data_home=$3
xdg_cache_home=$4
output_dir=$5
reuse_installed=${6:-}
requests=$source_tree/tests/fixtures/snv-regression/requests.tsv

[[ -x "$pangopup" && -f "$pangopup" && ! -L "$pangopup" ]] || { printf 'pangopup executable is unsafe\n' >&2; exit 1; }
[[ -d "$source_tree" && ! -L "$source_tree" && -f "$requests" && ! -L "$requests" ]] || { printf 'source tree is unsafe\n' >&2; exit 1; }
[[ "$xdg_data_home" == /* && "$xdg_cache_home" == /* && "$output_dir" == /* ]] || { printf 'runtime paths must be absolute\n' >&2; exit 1; }
[[ ! -e "$output_dir" && ! -L "$output_dir" ]] || { printf 'qualification output directory must be absent\n' >&2; exit 1; }
if [[ "$reuse_installed" == --reuse-installed ]]; then
  for directory in "$xdg_data_home" "$xdg_cache_home"; do
    [[ -d "$directory" && ! -L "$directory" ]] || { printf 'qualified installed directories are required\n' >&2; exit 1; }
  done
  install -d -m 700 "$output_dir"
else
  for directory in "$xdg_data_home" "$xdg_cache_home"; do
    [[ ! -e "$directory" && ! -L "$directory" ]] || { printf 'qualification directories must be absent\n' >&2; exit 1; }
  done
  install -d -m 700 "$xdg_data_home" "$xdg_cache_home" "$output_dir"
fi
unset PANGOPUP_DATA_DIR PANGOPUP_CACHE_DIR PANGOPUP_MODEL_CACHE PANGOPUP_MODEL_CACHE_MAX_ENTRIES
export XDG_DATA_HOME=$xdg_data_home
export XDG_CACHE_HOME=$xdg_cache_home
export HOME=$output_dir/home
install -d -m 700 "$HOME"

run_clean() {
  local output=$1
  shift
  if ! "$@" >"$output" 2>"$output.stderr"; then
    printf 'qualification command failed: %s\n' "${output##*/}" >&2
    return 1
  fi
  [[ ! -s "$output.stderr" ]] || { printf 'qualification command wrote stderr: %s\n' "${output##*/}" >&2; return 1; }
  rm "$output.stderr"
}

sync_command=("$pangopup" sync --progress)
if [[ "$reuse_installed" == --reuse-installed ]]; then sync_command+=(--offline); fi
if ! "${sync_command[@]}" >"$output_dir/sync-online.json" 2>"$output_dir/sync-online.progress"; then
  printf 'qualification command failed: sync-online.json\n' >&2
  exit 1
fi
[[ -s "$output_dir/sync-online.progress" ]] || { printf 'online sync emitted no progress\n' >&2; exit 1; }
if grep -Ev '^sync: ' "$output_dir/sync-online.progress" | grep -q .; then
  printf 'online sync progress contained foreign stderr\n' >&2
  exit 1
fi
run_clean "$output_dir/sync-offline.json" "$pangopup" sync --offline
run_clean "$output_dir/sync-quiet.json" "$pangopup" sync --offline --quiet
run_clean "$output_dir/status.json" "$pangopup" status
for command in sync status lookup serve; do
  run_clean "$output_dir/help-$command.txt" "$pangopup" "$command" --help
done

# Production SNV qualification deliberately bypasses installed-profile model
# fallback. Admit exactly the one immutable SNV bundle that the fresh sync
# installed, then give that path explicitly to every SNV oracle invocation.
snv_root=$xdg_data_home/pangopup
bundles_dir=$snv_root/bundles
current_uid=$(id -u)
safe_directory() {
  local path=$1
  local mode=$2
  [[ -d "$path" && ! -L "$path" ]] || return 1
  [[ $(stat -c '%u' -- "$path") == "$current_uid" ]] || return 1
  [[ $(stat -c '%a' -- "$path") == "$mode" ]] || return 1
}
safe_directory "$snv_root" 700 && safe_directory "$bundles_dir" 700 || {
  printf 'installed SNV bundle root is unsafe\n' >&2
  exit 1
}
shopt -s nullglob dotglob
bundle_wrappers=("$bundles_dir"/*)
shopt -u nullglob dotglob
(( ${#bundle_wrappers[@]} == 1 )) || {
  printf 'expected exactly one installed SNV bundle, found %d\n' "${#bundle_wrappers[@]}" >&2
  exit 1
}
bundle_wrapper=${bundle_wrappers[0]}
snv_bundle=$bundle_wrapper/bundle
safe_directory "$bundle_wrapper" 555 && safe_directory "$snv_bundle" 555 || {
  printf 'installed SNV bundle is unsafe\n' >&2
  exit 1
}

groups=(
  ENSG00000010610
  ENSG00000141499
  ENSG00000141510
  ENSG00000169129
  ENSG00000175727
  ENSG00000185974
  unfiltered
)
for group in "${groups[@]}"; do
  mapfile -t variants < <(awk -F '\t' -v group="$group" 'NR > 1 && $2 == group { print $4 }' "$requests")
  (( ${#variants[@]} > 0 )) || { printf 'SNV group is empty: %s\n' "$group" >&2; exit 1; }
  command=("$pangopup" lookup --bundle "$snv_bundle" --format jsonl)
  for variant in "${variants[@]}"; do command+=(--variant "$variant"); done
  if [[ "$group" != unfiltered ]]; then command+=(--gene "$group"); fi
  run_clean "$output_dir/snv-$group.jsonl" "${command[@]}"
done

run_clean "$output_dir/model-M09.jsonl" "$pangopup" lookup \
  --variant GRCh38:chr12:6801303:G:GA --format jsonl
run_clean "$output_dir/model-only-SNV.jsonl" "$pangopup" lookup --model-only \
  --variant GRCh38:chr12:6801301:G:A --format jsonl

http_request() {
  local method=$1 path=$2 body=$3 output=$4
  exec 3<>/dev/tcp/127.0.0.1/18080 || return 1
  printf '%s %s HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n' "$method" "$path" >&3
  if [[ -n "$body" ]]; then
    printf 'Content-Type: application/json\r\nContent-Length: %s\r\n' "${#body}" >&3
  fi
  printf '\r\n%s' "$body" >&3
  cat <&3 >"$output"
  exec 3>&- 3<&-
}

"$pangopup" serve --listen 127.0.0.1:18080 --model-workers 1 --model-threads 1 \
  >"$output_dir/service.stdout" 2>"$output_dir/service.stderr" &
service_pid=$!
stop_service() {
  kill -TERM "$service_pid" 2>/dev/null || true
  wait "$service_pid" 2>/dev/null || true
}
trap stop_service EXIT
ready=0
for _ in $(seq 1 30); do
  if http_request GET /livez '' "$output_dir/http-livez.txt" 2>/dev/null \
    && grep -Fq 'HTTP/1.1 200' "$output_dir/http-livez.txt"; then
    ready=1
    break
  fi
  sleep 1
done
((ready)) || { printf 'HTTP service did not become live\n' >&2; exit 1; }
http_request GET /readyz '' "$output_dir/http-readyz.txt"
http_request GET /v1/status '' "$output_dir/http-status.txt"
http_request POST /v1/score \
  '{"variants":["GRCh38:chr12:6801301:G:A"]}' "$output_dir/http-snv.txt"
http_request POST /v1/score \
  '{"variants":["GRCh38:chr12:6801303:G:GA"]}' "$output_dir/http-model.txt"
http_request POST /v1/score \
  '{"variants":["GRCh38:chr12:6801301:G:A"],"model_only":true}' \
  "$output_dir/http-model-only.txt"
stop_service
trap - EXIT

printf 'production qualification outputs written to %s\n' "$output_dir"
