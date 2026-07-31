#!/usr/bin/env bash
set -euo pipefail

[[ $# == 3 ]] || { printf 'usage: qualify-linux-release.sh <RELEASE_DIR> <VERSION> <40_LOWERCASE_COMMIT>\n' >&2; exit 2; }
release=$1
version=$2
commit=$3
[[ -d "$release" && ! -L "$release" ]] || { printf 'release directory is unsafe\n' >&2; exit 1; }
[[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] || { printf 'version is invalid\n' >&2; exit 1; }
[[ "$commit" =~ ^[0-9a-f]{40}$ ]] || { printf 'commit is invalid\n' >&2; exit 1; }

expected=(LICENSE NOTICE pangopup-linux-x86_64 pangopup-linux-x86_64.cdx.json pangopup-linux-x86_64.sha256 release-manifest.json)
shopt -s dotglob nullglob
entries=("$release"/*)
(( ${#entries[@]} == 6 )) || { printf 'release inventory must contain exactly six entries\n' >&2; exit 1; }
observed=()
for entry in "${entries[@]}"; do
  [[ -f "$entry" && ! -L "$entry" ]] || { printf 'every release entry must be a regular non-symlink file\n' >&2; exit 1; }
  observed+=("${entry##*/}")
done
mapfile -t observed < <(printf '%s\n' "${observed[@]}" | sort)
[[ "${observed[*]}" == "${expected[*]}" ]] || { printf 'release inventory names are invalid\n' >&2; exit 1; }

binary="$release/pangopup-linux-x86_64"
allowed='^(libstdc\+\+\.so\.6|libgcc_s\.so\.1|libm\.so\.6|libc\.so\.6|ld-linux-x86-64\.so\.2)$'
mapfile -t needed < <(readelf -d "$binary" | sed -n 's/.*(NEEDED).*\[\([^]]*\)\].*/\1/p')
(( ${#needed[@]} > 0 )) || { printf 'release binary has no dynamic dependencies\n' >&2; exit 1; }
for library in "${needed[@]}"; do [[ "$library" =~ $allowed ]] || { printf 'release binary has a foreign dependency\n' >&2; exit 1; }; done
maximum=$(readelf --version-info "$binary" | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | cut -d_ -f2 | sort -V | tail -1)
[[ -n "$maximum" && "$(printf '%s\n' "$maximum" 2.35 | sort -V | tail -1)" == 2.35 ]] || { printf 'release binary exceeds GLIBC 2.35\n' >&2; exit 1; }
(cd "$release" && sha256sum --check --strict pangopup-linux-x86_64.sha256 >/dev/null)

uv run --no-project --python "$(command -v python3)" python - \
  "$release/release-manifest.json" "$binary" "$version" "$commit" "$maximum" "${needed[@]}" <<'PY'
import hashlib, json, os, sys
manifest_path, binary, version, commit, maximum, *needed = sys.argv[1:]
with open(manifest_path, encoding="utf-8") as stream:
    manifest = json.load(stream)
assert set(manifest) == {
    "schema", "version", "target_commit", "target", "rust_toolchain",
    "binary_size", "maximum_glibc_version", "dynamic_dependencies", "members",
}
assert manifest["schema"] == "pangopup-executable-release-v1"
assert manifest["version"] == version
assert manifest["target_commit"] == commit
assert manifest["target"] == "x86_64-unknown-linux-gnu"
assert manifest["rust_toolchain"] == "1.93.1"
assert manifest["binary_size"] == os.path.getsize(binary)
assert manifest["maximum_glibc_version"] == maximum
assert manifest["dynamic_dependencies"] == needed
expected = ["LICENSE", "NOTICE", "pangopup-linux-x86_64", "pangopup-linux-x86_64.cdx.json", "pangopup-linux-x86_64.sha256"]
assert [member["name"] for member in manifest["members"]] == expected
assert all(set(member) == {"name", "size", "sha256"} for member in manifest["members"])
root = os.path.dirname(manifest_path)
for member in manifest["members"]:
    path = os.path.join(root, member["name"])
    with open(path, "rb") as stream:
        content = stream.read()
    assert member["size"] == len(content)
    assert member["sha256"] == hashlib.sha256(content).hexdigest()
PY
