#!/usr/bin/env bash
set -euo pipefail

[[ $# == 4 ]] || {
  printf 'usage: smoke-linux-release.sh <PANGOPUP> <SOURCE_TREE> <DATA_DIR> <MODEL_CACHE>\n' >&2
  exit 2
}

pangopup=$1
source_tree=$2
data_dir=$3
model_cache=$4

[[ -x "$pangopup" && ! -L "$pangopup" ]] || { printf 'Pangopup executable is invalid\n' >&2; exit 1; }
[[ -d "$source_tree" && ! -L "$source_tree" ]] || { printf 'source tree is invalid\n' >&2; exit 1; }
[[ "$data_dir" == /* ]] || { printf 'data directory must be absolute\n' >&2; exit 1; }
[[ "$model_cache" == /* ]] || { printf 'model cache must be absolute\n' >&2; exit 1; }

"$pangopup" --version
"$pangopup" --help >/dev/null
"$pangopup" status --data-dir "$data_dir" | grep -Fq '"status":"missing"'
"$pangopup" lookup \
  --bundle "$source_tree/tests/fixtures/snv-regression/bundle" \
  --variant GRCh38:chr12:6801301:G:A \
  | grep -Fq '"kind":"precomputed"'
"$pangopup" lookup \
  --bundle "$source_tree/tests/fixtures/snv-regression/bundle" \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle "$source_tree/tests/fixtures/reference-route-test/bundle" \
  --mask "$source_tree/tests/fixtures/route-mask/domains.pgm" \
  --model-bundle "$source_tree/tests/fixtures/pangolin-model-kernel-mini/bundle" \
  --model-cache "$model_cache" \
  | grep -Fq '"kind":"model"'
