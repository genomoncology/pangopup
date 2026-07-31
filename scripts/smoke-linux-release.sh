#!/usr/bin/env bash
set -euo pipefail

[[ $# == 4 ]] || {
  printf 'usage: smoke-linux-release.sh <PANGOPUP> <SOURCE_TREE> <DATA_DIR> <CACHE_PARENT>\n' >&2
  exit 2
}

pangopup=$1
source_tree=$2
data_dir=$3
cache_parent=$4

[[ -x "$pangopup" && ! -L "$pangopup" ]] || { printf 'Pangopup executable is invalid\n' >&2; exit 1; }
[[ -d "$source_tree" && ! -L "$source_tree" ]] || { printf 'source tree is invalid\n' >&2; exit 1; }
[[ "$data_dir" == /* ]] || { printf 'data directory must be absolute\n' >&2; exit 1; }
[[ "$cache_parent" == /tmp/* && -n "${cache_parent#/tmp/}" && "${cache_parent#/tmp/}" != */* ]] || {
  printf 'cache parent must be an immediate child of /tmp\n' >&2
  exit 1
}

"$pangopup" --version
"$pangopup" --help >/dev/null
"$pangopup" status --data-dir "$data_dir" | grep -Fq '"status":"missing"'
"$pangopup" lookup \
  --bundle "$source_tree/tests/fixtures/snv-regression/bundle" \
  --variant GRCh38:chr12:6801301:G:A \
  | grep -Fq '"kind":"precomputed"'
[[ ! -e "$cache_parent" && ! -L "$cache_parent" ]] || { printf 'cache parent already exists\n' >&2; exit 1; }
mkdir -m 0700 -- "$cache_parent"
[[ -d "$cache_parent" && ! -L "$cache_parent" ]] || { printf 'cache parent is invalid\n' >&2; exit 1; }
[[ "$(stat -c %u "$cache_parent")" == "$(id -u)" ]] || { printf 'cache parent has the wrong owner\n' >&2; exit 1; }
[[ "$(stat -c %a "$cache_parent")" == 700 ]] || { printf 'cache parent has the wrong mode\n' >&2; exit 1; }
"$pangopup" lookup \
  --bundle "$source_tree/tests/fixtures/snv-regression/bundle" \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle "$source_tree/tests/fixtures/reference-route-test/bundle" \
  --mask "$source_tree/tests/fixtures/route-mask/domains.pgm" \
  --model-bundle "$source_tree/tests/fixtures/pangolin-model-kernel-mini/bundle" \
  --model-cache "$cache_parent/model.sqlite3" \
  | grep -Fq '"kind":"model"'
