#!/usr/bin/env bash
set -euo pipefail

root=${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}
readme="$root/README.md"

fail() {
  printf 'README image check failed: %s\n' "$*" >&2
  exit 1
}

[[ -f "$readme" ]] || fail "missing README.md"

source_marks=(
  docs/images/pangopup.svg
  docs/images/genomoncology.png
)
hero=docs/images/pangopup-performance.png
hero_alt='PangoPup lookup-first performance overview showing mmap SNV lookup, CPU ONNX model fallback, SQLite reuse, and measured resource use'
hero_line="![$hero_alt]($hero)"
required=("${source_marks[@]}" "$hero")

for relative in "${required[@]}"; do
  [[ -f "$root/$relative" ]] || fail "missing $relative"
done

[[ $(grep -Fxc "$hero_line" "$readme" || true) == 1 ]] \
  || fail "README must contain the exact performance hero line once"

markdown_image_count=$(
  (grep -Fo '![' "$readme" || true) | wc -l | tr -d '[:space:]'
)
[[ "$markdown_image_count" == 1 ]] \
  || fail "README must contain no Markdown image other than the exact hero"

if grep -Eqi '<[[:space:]]*(img|picture|source)([[:space:]/>])' "$readme"; then
  fail "README must not contain HTML image-bearing tags"
fi

mapfile -t image_assets < <(
  find "$root/docs/images" -maxdepth 1 -type f \
    \( -name '*.png' -o -name '*.svg' \) -printf '%f\n' | sort
)
expected_assets=(genomoncology.png pangopup-performance.png pangopup.svg)
[[ "${image_assets[*]}" == "${expected_assets[*]}" ]] \
  || fail "docs/images must contain exactly the hero and its two retained source marks"

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
  local relative=$1 max_width=$2 max_height=$3 max_bytes=$4 bytes width height svg_stream
  bytes=$(file_bytes "$root/$relative")
  (( bytes <= max_bytes )) || fail "$relative is $bytes bytes; limit is $max_bytes"
  width=$(sed -nE '1s/.* width="([0-9.]+)".*/\1/p' "$root/$relative")
  height=$(sed -nE '1s/.* height="([0-9.]+)".*/\1/p' "$root/$relative")
  [[ -n "$width" && -n "$height" ]] || fail "$relative needs numeric width and height"
  awk -v width="$width" -v height="$height" -v max_width="$max_width" -v max_height="$max_height" \
    'BEGIN { exit !(width <= max_width && height <= max_height) }' \
    || fail "$relative is ${width}x${height}; limit is ${max_width}x${max_height}"

  svg_stream=$(tr '\r\n\t' '   ' < "$root/$relative")
  if grep -Eqi '<[[:space:]]*(script|foreignObject)([[:space:]>])|[[:space:]]on[a-z]+[[:space:]]*=' <<<"$svg_stream"; then
    fail "$relative contains executable or foreign SVG content"
  fi
  if grep -Eqi '(^|[[:space:]])(xlink:)?(href|src)[[:space:]]*=' <<<"$svg_stream"; then
    fail "$relative contains a linked SVG reference"
  fi
  if grep -Eqi 'url[[:space:]]*\(|@[[:space:]]*import([[:space:]]|\()' <<<"$svg_stream"; then
    fail "$relative contains a linked CSS reference"
  fi
  if grep -Eqi '<![[:space:]]*(DOCTYPE|ENTITY)([[:space:]>])' <<<"$svg_stream"; then
    fail "$relative contains a document type or entity declaration"
  fi
}

check_svg docs/images/pangopup.svg 800 400 100000
check_png docs/images/genomoncology.png 800 400 100000
check_png docs/images/pangopup-performance.png 2000 1125 400000

printf 'README image assets verified\n'
