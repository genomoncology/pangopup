#!/usr/bin/env bash
set -euo pipefail

[[ $# == 5 ]] || {
  printf 'usage: run-production-qualification.sh <PANGOPUP> <SOURCE_TREE> <XDG_DATA_HOME> <XDG_CACHE_HOME> <ABSENT_OUTPUT_DIR>\n' >&2
  exit 2
}

pangopup=$1
source_tree=$2
xdg_data_home=$3
xdg_cache_home=$4
output_dir=$5
requests=$source_tree/tests/fixtures/snv-regression/requests.tsv

[[ -x "$pangopup" && -f "$pangopup" && ! -L "$pangopup" ]] || { printf 'pangopup executable is unsafe\n' >&2; exit 1; }
[[ -d "$source_tree" && ! -L "$source_tree" && -f "$requests" && ! -L "$requests" ]] || { printf 'source tree is unsafe\n' >&2; exit 1; }
[[ "$xdg_data_home" == /* && "$xdg_cache_home" == /* && "$output_dir" == /* ]] || { printf 'runtime paths must be absolute\n' >&2; exit 1; }
for directory in "$xdg_data_home" "$xdg_cache_home" "$output_dir"; do
  [[ ! -e "$directory" && ! -L "$directory" ]] || { printf 'qualification directories must be absent\n' >&2; exit 1; }
done

install -d -m 700 "$xdg_data_home" "$xdg_cache_home" "$output_dir"
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

run_clean "$output_dir/sync-online.json" "$pangopup" sync
run_clean "$output_dir/sync-offline.json" "$pangopup" sync --offline
run_clean "$output_dir/status.json" "$pangopup" status

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

printf 'production qualification outputs written to %s\n' "$output_dir"
