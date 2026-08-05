#!/usr/bin/env bash
set -euo pipefail

root=${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}
readme="$root/README.md"

fail() {
  printf 'README image check failed: %s\n' "$*" >&2
  exit 1
}

[[ -f "$readme" ]] || fail "missing README.md"

required=(
  docs/images/pangopup.svg
  docs/images/genomoncology.png
  docs/images/pangopup-performance.png
)

for relative in "${required[@]}"; do
  [[ -f "$root/$relative" ]] || fail "missing $relative"
  grep -Fq "$relative" "$readme" || fail "README does not reference $relative"
done

grep -Fq 'alt="PangoPup pangolin logo"' "$readme" || fail "PangoPup logo needs useful alt text"
grep -Fq 'alt="GenomOncology logo"' "$readme" || fail "GenomOncology logo needs useful alt text"
grep -Fq '![PangoPup lookup-first performance overview showing mmap SNV lookup, CPU ONNX model fallback, SQLite reuse, and measured resource use]' "$readme" \
  || fail "performance overview needs useful alt text"

while IFS= read -r relative; do
  [[ "$relative" == docs/images/* ]] || fail "README image path is not repository-relative: $relative"
  [[ -f "$root/$relative" ]] || fail "README image does not resolve: $relative"
done < <(
  {
    grep -Eo 'src="[^"]+"' "$readme" | cut -d'"' -f2
    grep -Eo "src='[^']+'" "$readme" | cut -d"'" -f2
    grep -Eo '!\[[^]]*\]\([^ )]+' "$readme" | sed -E 's/^.*\]\(//'
  } | sort -u
)

file_bytes() {
  wc -c < "$1" | tr -d '[:space:]'
}

png_dimensions() {
  local file=$1 signature width_hex height_hex
  signature=$(od -An -tx1 -N8 "$file" | tr -d '[:space:]')
  [[ "$signature" == 89504e470d0a1a0a ]] || fail "$file is not a PNG"
  width_hex=$(od -An -tx1 -j16 -N4 "$file" | tr -d '[:space:]')
  height_hex=$(od -An -tx1 -j20 -N4 "$file" | tr -d '[:space:]')
  printf '%d %d\n' "$((16#$width_hex))" "$((16#$height_hex))"
}

check_png() {
  local relative=$1 max_width=$2 max_height=$3 max_bytes=$4 bytes width height
  bytes=$(file_bytes "$root/$relative")
  (( bytes <= max_bytes )) || fail "$relative is $bytes bytes; limit is $max_bytes"
  read -r width height < <(png_dimensions "$root/$relative")
  (( width <= max_width && height <= max_height )) \
    || fail "$relative is ${width}x${height}; limit is ${max_width}x${max_height}"
}

check_svg() {
  local relative=$1 max_width=$2 max_height=$3 max_bytes=$4 bytes width height
  bytes=$(file_bytes "$root/$relative")
  (( bytes <= max_bytes )) || fail "$relative is $bytes bytes; limit is $max_bytes"
  width=$(sed -nE '1s/.* width="([0-9.]+)".*/\1/p' "$root/$relative")
  height=$(sed -nE '1s/.* height="([0-9.]+)".*/\1/p' "$root/$relative")
  [[ -n "$width" && -n "$height" ]] || fail "$relative needs numeric width and height"
  awk -v width="$width" -v height="$height" -v max_width="$max_width" -v max_height="$max_height" \
    'BEGIN { exit !(width <= max_width && height <= max_height) }' \
    || fail "$relative is ${width}x${height}; limit is ${max_width}x${max_height}"

  if grep -Eqi '<[[:space:]]*(script|foreignObject)([[:space:]>])|[[:space:]]on[a-z]+[[:space:]]*=' "$root/$relative"; then
    fail "$relative contains executable or foreign SVG content"
  fi
  if grep -Eqi '(^|[[:space:]])(xlink:)?(href|src)[[:space:]]*=' "$root/$relative"; then
    fail "$relative contains a linked SVG reference"
  fi
  if grep -Eqi 'url[[:space:]]*\(|@[[:space:]]*import([[:space:]]|\()' "$root/$relative"; then
    fail "$relative contains a linked CSS reference"
  fi
  if grep -Eqi '<![[:space:]]*(DOCTYPE|ENTITY)([[:space:]>])' "$root/$relative"; then
    fail "$relative contains a document type or entity declaration"
  fi
}

check_svg docs/images/pangopup.svg 800 400 100000
check_png docs/images/genomoncology.png 800 400 100000
check_png docs/images/pangopup-performance.png 2000 1125 400000

printf 'README image assets verified\n'
