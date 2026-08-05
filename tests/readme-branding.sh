#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
checker="$repo/scripts/check-readme-images.sh"

"$checker" "$repo"

expect_rejected() {
  local name=$1 root=$2
  if "$checker" "$root" >"$root/$name.out" 2>"$root/$name.err"; then
    printf 'README image mutation unexpectedly passed: %s\n' "$name" >&2
    exit 1
  fi
}

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
cp "$repo/README.md" "$fixture/README.md"
mkdir -p "$fixture/docs/images"
cp "$repo/docs/images/"* "$fixture/docs/images/"

rm "$fixture/docs/images/genomoncology.png"
expect_rejected missing-source-mark "$fixture"
cp "$repo/docs/images/genomoncology.png" "$fixture/docs/images/genomoncology.png"

printf "\n<img src='docs/images/pangopup.svg' alt='Second displayed image'>\n" >> "$fixture/README.md"
expect_rejected second-displayed-image "$fixture"
cp "$repo/README.md" "$fixture/README.md"

printf "\n![Second displayed image][source-mark]\n[source-mark]: docs/images/pangopup.svg\n" >> "$fixture/README.md"
expect_rejected reference-style-image "$fixture"
cp "$repo/README.md" "$fixture/README.md"

printf "\n<IMG SRC='docs/images/pangopup.svg' ALT='Second displayed image'>\n" >> "$fixture/README.md"
expect_rejected uppercase-html-img "$fixture"
cp "$repo/README.md" "$fixture/README.md"

printf "\n<PICTURE><SOURCE SRCSET='docs/images/pangopup.svg'></PICTURE>\n" >> "$fixture/README.md"
expect_rejected uppercase-html-picture-source "$fixture"
cp "$repo/README.md" "$fixture/README.md"

sed -i '/pangopup-performance.png/d' "$fixture/README.md"
expect_rejected missing-hero "$fixture"
cp "$repo/README.md" "$fixture/README.md"

cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/extra-source-mark.svg"
expect_rejected extra-source-mark "$fixture"
rm "$fixture/docs/images/extra-source-mark.svg"

cp "$repo/docs/images/pangopup-performance.png" "$fixture/docs/images/pangopup-performance.png"
truncate -s 400001 "$fixture/docs/images/pangopup-performance.png"
expect_rejected oversized-image "$fixture"
cp "$repo/docs/images/pangopup-performance.png" "$fixture/docs/images/pangopup-performance.png"

printf '\x00\x00\x07\xd1' | dd of="$fixture/docs/images/pangopup-performance.png" bs=1 seek=16 conv=notrunc status=none
expect_rejected oversized-dimensions "$fixture"
cp "$repo/docs/images/pangopup-performance.png" "$fixture/docs/images/pangopup-performance.png"

sed -i 's#</svg>#<script>alert(1)</script></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-script "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#<svg #<svg onload="alert(1)" #' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-event-handler "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<foreignObject></foreignObject></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-foreign-object "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<image href="https://example.invalid/logo.png" /></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-external-reference "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<image href="data:image/png;base64,AAAA" /></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-data-reference "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<image href="other.png" /></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-relative-reference "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<image href\n="other.png" /></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-multiline-relative-reference "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<a href="javascript:alert(1)"></a></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-javascript-reference "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<style>.mark{fill:url(https://example.invalid/fill.svg)}</style></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-css-external-url "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<style>.mark{fill:url(other.svg)}</style></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-css-relative-url "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i 's#</svg>#<style>@import "https://example.invalid/style.css";</style></svg>#' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-css-import "$fixture"
cp "$repo/docs/images/pangopup.svg" "$fixture/docs/images/pangopup.svg"

sed -i '1i<!DOCTYPE svg [<!ENTITY external SYSTEM "https://example.invalid/entity.svg">]>' "$fixture/docs/images/pangopup.svg"
expect_rejected svg-external-entity "$fixture"

printf 'README image rejection cases verified\n'
