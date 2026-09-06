#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
root="$repo/target/executable-delivery-test"
version=$(grep -m1 '^version = ' "$repo/Cargo.toml" | cut -d'"' -f2)
[[ -n "$version" ]]
rm -rf -- "$root"
mkdir -p "$root"

fail() { printf 'executable delivery test: %s\n' "$*" >&2; exit 1; }

smoke_bin="$root/smoke-bin"
mkdir "$smoke_bin"
cat >"$smoke_bin/pangopup" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$SMOKE_LOG"
if [[ "${1:-}" == --version ]]; then
  printf 'pangopup 0.4.1\n'
elif [[ " $* " == *' --help '* ]]; then
  printf 'usage: pangopup\n'
elif [[ "${1:-}" == status ]]; then
  printf '{"status":"missing"}\n'
elif [[ " $* " == *' GRCh38:chr12:6801301:G:A '* ]]; then
  printf '{"provenance":{"kind":"precomputed"}}\n'
elif [[ " $* " == *' GRCh38:chr1:5051:A:AC '* ]]; then
  printf '{"provenance":{"kind":"model"}}\n'
else
  exit 2
fi
EOF
cat >"$smoke_bin/docker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
while (($#)); do
  if [[ "$1" == bash ]]; then exec "$@"; fi
  shift
done
exit 2
EOF
chmod +x "$smoke_bin/pangopup" "$smoke_bin/docker"

smoke_log="$root/smoke.log"
fake_cache_parent="/tmp/pangopup-smoke-fake-$PPID-$$"
[[ ! -e "$fake_cache_parent" && ! -L "$fake_cache_parent" ]]
SMOKE_LOG="$smoke_log" SMOKE_SCRIPT="$repo/scripts/smoke-linux-release.sh" \
  SMOKE_PANGOPUP="$smoke_bin/pangopup" \
  SMOKE_SOURCE="$repo" SMOKE_DATA="$root/smoke-data" \
  SMOKE_CACHE="$fake_cache_parent" PATH="$smoke_bin:$PATH" \
  docker run --rm --network none --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m \
    -v "$root:/release:ro" -v "$repo:/source:ro" smoke-image bash -ceu '
      "$SMOKE_SCRIPT" "$SMOKE_PANGOPUP" "$SMOKE_SOURCE" "$SMOKE_DATA" "$SMOKE_CACHE"
    '
[[ "$(wc -l <"$smoke_log")" == 10 ]]
grep -Fq -- "--model-cache $fake_cache_parent/model.sqlite3" "$smoke_log"
grep -Fq -- "--model-only" "$smoke_log"
[[ "$(stat -c %u "$fake_cache_parent")" == "$(id -u)" ]]
[[ "$(stat -c %a "$fake_cache_parent")" == 700 ]]

changed_smoke="$root/smoke-changed-expected.sh"
sed 's/"status":"missing"/"status":"ready"/' \
  "$repo/scripts/smoke-linux-release.sh" >"$changed_smoke"
chmod +x "$changed_smoke"
! cmp -s "$repo/scripts/smoke-linux-release.sh" "$changed_smoke"

if SMOKE_LOG="$smoke_log" SMOKE_SCRIPT="$changed_smoke" \
  SMOKE_PANGOPUP="$smoke_bin/pangopup" SMOKE_SOURCE="$repo" \
  SMOKE_DATA="$root/smoke-data" SMOKE_CACHE="$fake_cache_parent" \
  PATH="$smoke_bin:$PATH" \
  docker run --rm --network none --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m \
    -v "$root:/release:ro" -v "$repo:/source:ro" smoke-image bash -ceu '
      "$SMOKE_SCRIPT" "$SMOKE_PANGOPUP" "$SMOKE_SOURCE" "$SMOKE_DATA" "$SMOKE_CACHE"
    '; then
  fail 'shared container smoke accepted a changed expected JSON value'
fi
rmdir "$fake_cache_parent"

real_cli="$repo/target/debug/pangopup"
[[ -x "$real_cli" && ! -L "$real_cli" ]]
unsafe_cache="/tmp/pangopup-smoke-unsafe-$PPID-$$.sqlite3"
[[ ! -e "$unsafe_cache" && ! -L "$unsafe_cache" ]]
if "$real_cli" lookup \
  --bundle "$repo/tests/fixtures/snv-regression/bundle" \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle "$repo/tests/fixtures/reference-route-test/bundle" \
  --mask "$repo/tests/fixtures/route-mask/domains.pgm" \
  --model-bundle "$repo/tests/fixtures/pangolin-model-kernel-mini/bundle" \
  --model-cache "$unsafe_cache" \
  >"$root/unsafe-cache.out" 2>"$root/unsafe-cache.err"; then
  fail 'real CLI accepted /tmp as the immediate model-cache parent'
fi
grep -Fq 'MODEL_CACHE_INVALID' "$root/unsafe-cache.err"
[[ ! -e "$unsafe_cache" && ! -L "$unsafe_cache" ]]

real_cache_parent="/tmp/pangopup-smoke-real-$PPID-$$"
[[ ! -e "$real_cache_parent" && ! -L "$real_cache_parent" ]]
"$repo/scripts/smoke-linux-release.sh" \
  "$real_cli" "$repo" "$root/real-smoke-data" "$real_cache_parent" \
  >"$root/real-smoke.out"
[[ "$(stat -c %u "$real_cache_parent")" == "$(id -u)" ]]
[[ "$(stat -c %a "$real_cache_parent")" == 700 ]]
[[ -f "$real_cache_parent/model.sqlite3" && ! -L "$real_cache_parent/model.sqlite3" ]]
rm -f "$real_cache_parent/model.sqlite3" \
  "$real_cache_parent/model.sqlite3-shm" "$real_cache_parent/model.sqlite3-wal" \
  "$real_cache_parent/model-only.sqlite3" \
  "$real_cache_parent/model-only.sqlite3-shm" "$real_cache_parent/model-only.sqlite3-wal"
rmdir "$real_cache_parent"

expect_installer_failure() {
  local expected=$1
  shift
  if "$repo/install.sh" "$@" >"$root/rejected.out" 2>"$root/rejected.err"; then
    fail "installer rejection unexpectedly succeeded: $*"
  fi
  grep -Fq "$expected" "$root/rejected.err"
}

make_path() {
  local directory=$1
  local downloader=${2:-curl}
  local tool
  mkdir -p "$directory"
  for tool in bash mktemp chmod stat mkdir rm cp mv awk; do
    ln -s "$(command -v "$tool")" "$directory/$tool"
  done
  cat >"$directory/uname" <<'EOF'
#!/usr/bin/env bash
[[ "$1" == -s ]] && printf '%s\n' "${MOCK_OS:-Linux}" || printf '%s\n' "${MOCK_ARCH:-x86_64}"
EOF
  if [[ "$downloader" == none ]]; then
    chmod +x "$directory/uname"
    return
  fi
  cat >"$directory/$downloader" <<'EOF'
#!/usr/bin/env bash
out= url=
while (($#)); do
  if [[ "$1" == --output ]]; then out=$2; shift 2
  elif [[ "$1" == --output-document=* ]]; then out=${1#*=}; shift
  else url=$1; shift
  fi
done
[[ "${MOCK_DOWNLOAD_FAIL:-0}" == 0 ]] || exit 22
printf '%s\n' "$url" >>"$MOCK_URL_LOG"
if [[ "${url##*/}" == pangopup-linux-x86_64 && "${MOCK_UNSAFE:-}" == symlink ]]; then
  /bin/ln -s "$MOCK_ASSETS/${url##*/}" "$out"
elif [[ "${url##*/}" == pangopup-linux-x86_64 && "${MOCK_UNSAFE:-}" == hardlink ]]; then
  /bin/ln "$MOCK_ASSETS/${url##*/}" "$out"
else
  /bin/cp "$MOCK_ASSETS/${url##*/}" "$out"
fi
EOF
  chmod +x "$directory/uname" "$directory/$downloader"
}

assets="$root/assets"
mkdir "$assets"
cat >"$assets/pangopup-linux-x86_64" <<EOF
#!/usr/bin/env bash
[[ "\${1:-}" == --version ]] && printf 'pangopup $version\n'
EOF
chmod +x "$assets/pangopup-linux-x86_64"
digest=$(sha256sum "$assets/pangopup-linux-x86_64" | awk '{print $1}')
printf '%s  pangopup-linux-x86_64\n' "$digest" >"$assets/pangopup-linux-x86_64.sha256"

expect_installer_failure 'version must be latest or MAJOR.MINOR.PATCH' --version "v$version"
expect_installer_failure 'install directory must be an absolute path' --install-dir relative
expect_installer_failure 'unknown argument' --unknown
expect_installer_failure 'may be supplied only once' --version latest --version "$version"

mock="$root/mock"
make_path "$mock"
ln -s "$(command -v sha256sum)" "$mock/sha256sum"
out="$root/bin"
log="$root/urls"
MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" \
  "$repo/install.sh" --install-dir "$out" >"$root/latest.out"
[[ "$($out/pangopup --version)" == "pangopup $version" ]]
grep -Fxq 'https://github.com/genomoncology/pangopup/releases/latest/download/pangopup-linux-x86_64' "$log"
grep -Fq 'Next: pangopup sync' "$root/latest.out"
grep -Fq 'Then: pangopup status' "$root/latest.out"
grep -Fq "releases/download/v$version/LICENSE" "$root/latest.out"
grep -Fq "export PATH=$out:\"\$PATH\"" "$root/latest.out"

printf 'old executable\n' >"$out/pangopup"
printf '%064d  pangopup-linux-x86_64\n' 0 >"$assets/pangopup-linux-x86_64.sha256"
if MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" \
  "$repo/install.sh" --version "$version" --install-dir "$out" >"$root/fail.out" 2>"$root/fail.err"; then
  fail 'checksum mismatch unexpectedly succeeded'
fi
[[ "$(cat "$out/pangopup")" == 'old executable' ]]
grep -Fq 'checksum does not match' "$root/fail.err"

cat >"$assets/pangopup-linux-x86_64" <<'EOF'
#!/usr/bin/env bash
[[ "${1:-}" == --version ]] && printf 'pangopup 9.9.9\n'
EOF
chmod +x "$assets/pangopup-linux-x86_64"
bad_digest=$(sha256sum "$assets/pangopup-linux-x86_64" | awk '{print $1}')
printf '%s  pangopup-linux-x86_64\n' "$bad_digest" >"$assets/pangopup-linux-x86_64.sha256"
if MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" \
  "$repo/install.sh" --version "$version" --install-dir "$out" >/dev/null 2>"$root/version.err"; then
  fail 'wrong executable version unexpectedly succeeded'
fi
[[ "$(cat "$out/pangopup")" == 'old executable' ]]
cat >"$assets/pangopup-linux-x86_64" <<EOF
#!/usr/bin/env bash
[[ "\${1:-}" == --version ]] && printf 'pangopup $version\n'
EOF
chmod +x "$assets/pangopup-linux-x86_64"
digest=$(sha256sum "$assets/pangopup-linux-x86_64" | awk '{print $1}')

if MOCK_OS=Darwin PATH="$mock" "$repo/install.sh" --install-dir "$root/os" >/dev/null 2>"$root/os.err"; then fail 'unsupported OS unexpectedly succeeded'; fi
grep -Fq 'only Linux is supported' "$root/os.err"
if MOCK_ARCH=aarch64 PATH="$mock" "$repo/install.sh" --install-dir "$root/arch" >/dev/null 2>"$root/arch.err"; then fail 'unsupported architecture unexpectedly succeeded'; fi
grep -Fq 'only Linux x86_64 is supported' "$root/arch.err"

no_downloader="$root/mock-no-downloader"
make_path "$no_downloader" none
ln -s "$(command -v sha256sum)" "$no_downloader/sha256sum"
if PATH="$no_downloader" "$repo/install.sh" --install-dir "$root/no-downloader" >/dev/null 2>"$root/no-downloader.err"; then fail 'missing downloader unexpectedly succeeded'; fi
grep -Fq 'curl or wget is required' "$root/no-downloader.err"

no_checksum="$root/mock-no-checksum"
make_path "$no_checksum"
if PATH="$no_checksum" "$repo/install.sh" --install-dir "$root/no-checksum" >/dev/null 2>"$root/no-checksum.err"; then fail 'missing checksum tool unexpectedly succeeded'; fi
grep -Fq 'sha256sum, shasum, or openssl is required' "$root/no-checksum.err"

if MOCK_DOWNLOAD_FAIL=1 MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --install-dir "$root/download-fail" >/dev/null 2>"$root/download-fail.err"; then fail 'downloader failure unexpectedly succeeded'; fi

for malformed in empty multiple wrong-name; do
  case "$malformed" in
    empty) : >"$assets/pangopup-linux-x86_64.sha256" ;;
    multiple) printf '%s\n%s\n' "$digest" "$digest" >"$assets/pangopup-linux-x86_64.sha256" ;;
    wrong-name) printf '%s  wrong\n' "$digest" >"$assets/pangopup-linux-x86_64.sha256" ;;
  esac
  if MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --install-dir "$root/checksum-$malformed" >/dev/null 2>"$root/checksum-$malformed.err"; then fail "malformed checksum unexpectedly succeeded: $malformed"; fi
done
printf '%s  pangopup-linux-x86_64\n' "$digest" >"$assets/pangopup-linux-x86_64.sha256"

if MOCK_UNSAFE=symlink MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --install-dir "$root/unsafe" >/dev/null 2>"$root/unsafe.err"; then fail 'symlinked download unexpectedly succeeded'; fi
grep -Fq 'downloaded executable is not a regular file' "$root/unsafe.err"
if MOCK_UNSAFE=hardlink MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --install-dir "$root/hardlink" >/dev/null 2>"$root/hardlink.err"; then fail 'multiply linked download unexpectedly succeeded'; fi
grep -Fq 'downloaded executable must have one hard link' "$root/hardlink.err"

regular_install="$root/regular-install"
mkdir "$regular_install"
printf 'old\n' >"$regular_install/pangopup"
MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --version "$version" --install-dir "$regular_install" >/dev/null
[[ "$("$regular_install/pangopup" --version)" == "pangopup $version" ]]

destination_target="$root/destination-target"
mkdir "$destination_target"
ln -s "$destination_target" "$root/destination-link"
if MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --install-dir "$root/destination-link" >/dev/null 2>"$root/destination-link.err"; then fail 'symlink install directory unexpectedly succeeded'; fi
grep -Fq 'install directory must be a real directory' "$root/destination-link.err"

victim="$root/victim"
symlink_install="$root/symlink-install"
mkdir "$victim" "$symlink_install"
ln -s "$victim" "$symlink_install/pangopup"
MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --version "$version" --install-dir "$symlink_install" >/dev/null
[[ -f "$symlink_install/pangopup" && ! -L "$symlink_install/pangopup" ]]
[[ ! -e "$victim/pangopup" ]]

directory_install="$root/directory-install"
mkdir -p "$directory_install/pangopup"
printf 'preserve\n' >"$directory_install/pangopup/owner"
if MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --version "$version" --install-dir "$directory_install" >/dev/null 2>"$root/directory.err"; then fail 'directory target unexpectedly succeeded'; fi
[[ "$(cat "$directory_install/pangopup/owner")" == preserve ]]

special_install="$root/space * \$(touch SHOULD_NOT_EXIST)"
MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --version "$version" --install-dir "$special_install" >"$root/special.out"
[[ "$("$special_install/pangopup" --version)" == "pangopup $version" ]]
[[ ! -e "$root/SHOULD_NOT_EXIST" ]]
grep -Fq '\$\(touch\ SHOULD_NOT_EXIST\)' "$root/special.out"
guidance=$(grep -F 'Add Pangopup to PATH: ' "$root/special.out")
guidance=${guidance#Add Pangopup to PATH: }
(cd "$root" && EXPECTED_PATH="$special_install:/usr/bin" PATH=/usr/bin bash -c "$guidance; [[ \"\$PATH\" == \"\$EXPECTED_PATH\" ]]")
[[ ! -e "$root/SHOULD_NOT_EXIST" ]]

path_present="$root/path-present"
MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$path_present:$mock" "$repo/install.sh" --version "$version" --install-dir "$path_present" >"$root/path-present.out"
! grep -Fq 'Add Pangopup to PATH' "$root/path-present.out"
grep -Fq "Release: https://github.com/genomoncology/pangopup/releases/tag/v$version" "$root/path-present.out"
grep -Fq "Source: https://github.com/genomoncology/pangopup/tree/v$version" "$root/path-present.out"
grep -Fq "License: https://github.com/genomoncology/pangopup/releases/download/v$version/LICENSE" "$root/path-present.out"
grep -Fq "Notice: https://github.com/genomoncology/pangopup/releases/download/v$version/NOTICE" "$root/path-present.out"

non_directory="$root/not-a-directory"
printf 'owner\n' >"$non_directory"
if MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$mock" "$repo/install.sh" --install-dir "$non_directory" >/dev/null 2>"$root/not-a-directory.err"; then fail 'non-directory destination unexpectedly succeeded'; fi
[[ "$(cat "$non_directory")" == owner ]]

printf '%s *pangopup-linux-x86_64\n' "$digest" >"$assets/pangopup-linux-x86_64.sha256"
for tool in shasum openssl; do
  path="$root/mock-$tool"
  make_path "$path"
  ln -s "$(command -v "$tool")" "$path/$tool"
  MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$path" \
    "$repo/install.sh" --version "$version" --install-dir "$root/bin-$tool" >/dev/null
done

wget_path="$root/mock-wget"
make_path "$wget_path" wget
ln -s "$(command -v sha256sum)" "$wget_path/sha256sum"
MOCK_ASSETS="$assets" MOCK_URL_LOG="$log" PATH="$wget_path" \
  "$repo/install.sh" --version "$version" --install-dir "$root/bin-wget" >/dev/null

fixture="$root/repository"
mkdir "$fixture"
cp "$repo/LICENSE" "$repo/NOTICE" "$fixture/"
printf '[workspace.package]\nversion = "%s"\n' "$version" >"$fixture/Cargo.toml"
git -C "$fixture" init -q
git -C "$fixture" config user.name test
git -C "$fixture" config user.email test@example.invalid
git -C "$fixture" add Cargo.toml LICENSE NOTICE
git -C "$fixture" commit -qm fixture
commit=$(git -C "$fixture" rev-parse HEAD)
cat >"$root/fake.c" <<EOF
#include <stdio.h>
int main(int argc, char **argv) {
    if (argc == 2 && argv[1][0] == '-' && argv[1][1] == '-') {
        puts("pangopup $version");
        return 0;
    }
    return 2;
}
EOF
cc "$root/fake.c" -o "$root/input-pangopup"
printf '{"bomFormat":"CycloneDX","specVersion":"1.5","serialNumber":"urn:uuid:00000000-0000-0000-0000-000000000000","version":1}\n' >"$root/input.cdx.json"
binary_before=$(sha256sum "$root/input-pangopup" | awk '{print $1}')
sbom_before=$(sha256sum "$root/input.cdx.json" | awk '{print $1}')
for round in one two; do
  "$repo/target/debug/pangopup-build" executable-release prepare \
    --executable "$root/input-pangopup" --sbom "$root/input.cdx.json" \
    --version "$version" --target-commit "$commit" --repository "$fixture" \
    --output "$root/release-$round" >/dev/null
done
diff -r "$root/release-one" "$root/release-two"
[[ "$binary_before" == "$(sha256sum "$root/input-pangopup" | awk '{print $1}')" ]]
[[ "$sbom_before" == "$(sha256sum "$root/input.cdx.json" | awk '{print $1}')" ]]
[[ "$(find "$root/release-one" -mindepth 1 -maxdepth 1 -type f | wc -l)" == 6 ]]
(cd "$root/release-one" && sha256sum --check --strict pangopup-linux-x86_64.sha256 >/dev/null)
"$repo/scripts/qualify-linux-release.sh" "$root/release-one" "$version" "$commit"

expect_qualification_failure() {
  local label=$1 directory=$2
  if "$repo/scripts/qualify-linux-release.sh" "$directory" "$version" "$commit" >/dev/null 2>"$root/qualify-$label.err"; then
    fail "release qualification unexpectedly succeeded: $label"
  fi
}

cp -a "$root/release-one" "$root/release-newer-glibc"
uv run --no-project --python "$(command -v python3)" python - \
  "$root/release-newer-glibc/pangopup-linux-x86_64" <<'PY'
import sys

path = sys.argv[1]
with open(path, "rb") as stream:
    original = stream.read()
modified = original.replace(b"GLIBC_2.34\0", b"GLIBC_2.40\0")
assert modified != original
with open(path, "wb") as stream:
    stream.write(modified)
PY
expect_qualification_failure newer-glibc "$root/release-newer-glibc"
grep -Fq 'release binary exceeds GLIBC 2.39' "$root/qualify-newer-glibc.err"

cp -a "$root/release-one" "$root/release-extra"
printf 'extra\n' >"$root/release-extra/extra"
expect_qualification_failure extra "$root/release-extra"
cp -a "$root/release-one" "$root/release-symlink"
rm "$root/release-symlink/NOTICE"
ln -s "$repo/NOTICE" "$root/release-symlink/NOTICE"
expect_qualification_failure symlink "$root/release-symlink"
cp -a "$root/release-one" "$root/release-directory"
rm "$root/release-directory/NOTICE"
mkdir "$root/release-directory/NOTICE"
expect_qualification_failure directory "$root/release-directory"

for field in schema version target_commit target rust_toolchain member_name member_size member_sha256; do
  rebound="$root/release-rebound-$field"
  cp -a "$root/release-one" "$rebound"
  uv run --no-project --python "$(command -v python3)" python - "$rebound/release-manifest.json" "$field" <<'PY'
import json, sys
path, field = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    manifest = json.load(stream)
if field == "member_name": manifest["members"][0]["name"] = "FOREIGN"
elif field == "member_size": manifest["members"][0]["size"] += 1
elif field == "member_sha256": manifest["members"][0]["sha256"] = "0" * 64
else: manifest[field] = "foreign"
with open(path, "w", encoding="utf-8") as stream:
    json.dump(manifest, stream, separators=(",", ":"))
PY
  expect_qualification_failure "rebound-$field" "$rebound"
done

grep -Eq '^permissions:$' "$repo/.github/workflows/package-linux.yml"
grep -Eq '^  contents: read$' "$repo/.github/workflows/package-linux.yml"
! grep -Eq 'contents: write|attest|release create|release upload' "$repo/.github/workflows/package-linux.yml"
while IFS= read -r action; do
  [[ "$action" =~ @[0-9a-f]{40}$ ]] || fail "workflow action is not pinned: $action"
done < <(sed -nE 's/^[[:space:]]*- uses: ([^ #]+).*/\1/p' "$repo/.github/workflows/package-linux.yml")
grep -Fq 'git merge-base --is-ancestor "$EXACT_COMMIT" origin/main' "$repo/.github/workflows/package-linux.yml"
grep -Fq 'git diff --cached --quiet --' "$repo/.github/workflows/package-linux.yml"
for stage in \
  'Authenticate exact checkout' \
  'Install pinned Rust toolchain' \
  'Build release executables and SBOM tool' \
  'Generate deterministic untouched SBOM' \
  'Prepare exact release files' \
  'Qualify final executable and inventory' \
  'Smoke in pinned clean container'; do
  grep -Fq -- "- name: $stage" "$repo/.github/workflows/package-linux.yml"
done
grep -Fq 'astral-sh/setup-uv@08807647e7069bb48b6ef5acd8ec9567f424441b' "$repo/.github/workflows/package-linux.yml"
grep -Fq 'actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02' "$repo/.github/workflows/package-linux.yml"
grep -Fq 'cargo build --locked --release --package pangopup-cli' "$repo/.github/workflows/package-linux.yml"
grep -Fq 'cargo install --locked --version 0.5.9 cargo-cyclonedx' "$repo/.github/workflows/package-linux.yml"
grep -Fxq '          cargo fetch --locked' "$repo/.github/workflows/package-linux.yml"
[[ "$(grep -Fc 'cargo fetch' "$repo/.github/workflows/package-linux.yml")" == 1 ]]
fetch_line=$(grep -nF 'cargo fetch --locked' "$repo/.github/workflows/package-linux.yml" | cut -d: -f1)
offline_line=$(grep -nF 'CARGO_NET_OFFLINE=true cargo cyclonedx --manifest-path' "$repo/.github/workflows/package-linux.yml" | cut -d: -f1)
[[ -n "$fetch_line" && -n "$offline_line" && "$fetch_line" -lt "$offline_line" ]]
[[ "$(grep -Fc 'CARGO_NET_OFFLINE=true cargo cyclonedx --manifest-path' "$repo/.github/workflows/package-linux.yml")" == 1 ]]
[[ "$(grep -Fc 'cargo cyclonedx --manifest-path' "$repo/.github/workflows/package-linux.yml")" == 1 ]]
grep -Fq 'for round in one two' "$repo/.github/workflows/package-linux.yml"
grep -Fq 'scripts/qualify-linux-release.sh "$release" "$version" "$EXACT_COMMIT"' "$repo/.github/workflows/package-linux.yml"
grep -Fq 'ld-linux-x86-64\.so\.2' "$repo/scripts/qualify-linux-release.sh"
grep -Fq 'release inventory must contain exactly six entries' "$repo/scripts/qualify-linux-release.sh"
grep -Fq '/tmp/pangopup-cyclonedx-source-v1' "$repo/.github/workflows/package-linux.yml"
grep -Eq '^    runs-on: ubuntu-24[.]04$' "$repo/.github/workflows/package-linux.yml"
grep -Fq 'ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90' "$repo/.github/workflows/package-linux.yml"
grep -Fq '"$maximum" 2.39' "$repo/scripts/qualify-linux-release.sh"
smoke_invocation='              /source/scripts/smoke-linux-release.sh /release/pangopup-linux-x86_64 /source /tmp/data /tmp/pangopup-smoke-cache'
grep -Fxq "$smoke_invocation" "$repo/.github/workflows/package-linux.yml"
[[ "$(grep -Fc '/source/scripts/smoke-linux-release.sh' "$repo/.github/workflows/package-linux.yml")" == 1 ]]
! grep -Fq 'GRCh38:chr12:6801301:G:A' "$repo/.github/workflows/package-linux.yml"
! grep -Fq 'GRCh38:chr1:5051:A:AC' "$repo/.github/workflows/package-linux.yml"
! grep -Eq 'make (lint|test|spec)' "$repo/.github/workflows/package-linux.yml"
! grep -Eq 'Install gate prerequisites|mustmatch|cargo-deny|ripgrep' "$repo/.github/workflows/package-linux.yml"
! grep -Eq '^    runs-on: ubuntu-22[.]04$' "$repo/.github/workflows/package-linux.yml"
! grep -Fq '"$maximum" 2.35' "$repo/scripts/qualify-linux-release.sh"

release_notes="$repo/planning/artifacts/054-release-notes.md"
publication_record="$repo/planning/artifacts/055-public-v0.3.0.md"
grep -Fq 'raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh' "$release_notes"
grep -Fq 'ghcr.io/genomoncology/pangopup:0.3.0' "$release_notes"
if grep -Eqi 'prepared v0[.]3[.]0 candidate|publication (is )?pending' "$repo/README.md" "$release_notes"; then
  fail 'tagged v0.3.0 documents retain candidate or pending-publication language'
fi
grep -Fq 'State: **COMPLETE — immutable v0.3.0 executable and native container are public and qualified.**' "$publication_record"
grep -Fq 'release ID `365425336`' "$publication_record"
grep -Fq 'sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0' "$publication_record"
grep -Fq 'readonly PREVIOUS_RELEASE_ID=364960381' "$publication_record"
grep -Fq 'readonly PREVIOUS_INDEX=sha256:ad1aa8c27cc61d107310f609cd63f8fcbaf591a4f9760db475384a0a71049de4' "$publication_record"
! grep -Fq 'orgs/genomoncology/packages/container/pangopup' "$publication_record"
grep -Fq 'PUBLIC_DOCKER=$(mktemp -d)' "$publication_record"
grep -Fq 'readonly PRIVATE PUBLIC_DOCKER' "$publication_record"
grep -Fq 'jq -e '\''((.auths // {}) | length) == 0'\'' "$PUBLIC_DOCKER/config.json"' "$publication_record"
grep -Fq -- '--data-urlencode scope=repository:genomoncology/pangopup:pull' "$publication_record"
grep -Fq -- '--oauth2-bearer "$anonymous_token"' "$publication_record"
grep -Fq 'anonymous_digest latest "$PREVIOUS_INDEX" latest' "$publication_record"
grep -Fq 'anonymous_digest "$amd64" "$amd64" staged-amd64' "$publication_record"
grep -Fq 'anonymous_digest "$arm64" "$arm64" staged-arm64' "$publication_record"
grep -Fq 'planning/artifacts/054-release-notes.md' "$publication_record"
grep -Fq 'scripts/qualify-linux-release.sh "$RELEASE_DIR" 0.3.0 "$COMMIT"' "$publication_record"
grep -Fq 'uninstall --full --yes' "$publication_record"
final_tag_check=$(grep -nF 'test "$(gh api "repos/$REPO/git/matching-refs/tags/$TAG" --jq length)" -eq 0' "$publication_record" | tail -1 | cut -d: -f1)
final_draft_check=$(grep -nF 'gh api "repos/$REPO/releases/$RELEASE_ID" >"$PRIVATE/prepublish.json"' "$publication_record" | cut -d: -f1)
publish_request=$(grep -nF 'gh api --method PATCH "repos/$REPO/releases/$RELEASE_ID" \' "$publication_record" | cut -d: -f1)
[[ -n "$final_tag_check" && -n "$final_draft_check" && -n "$publish_request" ]]
[[ "$final_tag_check" -lt "$final_draft_check" && "$final_draft_check" -lt "$publish_request" ]]
if grep -Eqi '(authorization:[[:space:]]|bearer[[:space:]]+[a-z0-9]|ghp_[a-z0-9]|github_pat_[a-z0-9]|signed[_ -]?url)' "$publication_record"; then
  fail 'v0.3.0 publication record contains credential material'
fi

v040_release_notes="$repo/planning/artifacts/057-release-notes.md"
v040_publication_record="$repo/planning/artifacts/058-public-v0.4.0.md"
check_v040_partial_record() {
  local record=$1
  grep -Fxq 'State: **PARTIAL — immutable v0.4.0 executable public; v0.4.0 container aliases absent.**' "$record" || return 1
  grep -Fq 'GitHub release ID `383614742` is immutable, Latest, non-draft, and non-prerelease.' "$record" || return 1
  grep -Fq 'It has tag and title `v0.4.0` and `PangoPup v0.4.0`.' "$record" || return 1
  grep -Fq 'The release and direct `refs/tags/v0.4.0` both resolve to `ea4438e50762e32f09052b364060c89201ed78bc`.' "$record" || return 1
  grep -Fq 'Publication completed at `2026-09-06T14:33:59Z`.' "$record" || return 1
  [[ "$(grep -Fc 'ea4438e50762e32f09052b364060c89201ed78bc' "$record")" == 2 ]] || return 1
  grep -Fq 'planning/artifacts/057-release-notes.md`, whose SHA-256 is `729fa6ed9ddb641501f2abdf5e63cd2fd9861154a46f02967bea7ff408ce4aa9`' "$record" || return 1
  grep -Fq $'LICENSE\t35149\tsha256:3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986' "$record" || return 1
  grep -Fq $'NOTICE\t2365\tsha256:516f3c44d00eb2840a1a1a7e1127b027bb47190d3fe36a4c918572f39d7ad1c1' "$record" || return 1
  grep -Fq $'pangopup-linux-x86_64\t29034216\tsha256:6340f95f55b122f4d4f1b914cb59817239664ae36ee9d23924a9db3d8bb929c5' "$record" || return 1
  grep -Fq $'pangopup-linux-x86_64.cdx.json\t201980\tsha256:6b83db1c68d99354f4ea21ebcfc37debaaeaedcc9389a4461eef9c697939ffeb' "$record" || return 1
  grep -Fq $'pangopup-linux-x86_64.sha256\t88\tsha256:0fb7f8b3f559e9b516578a1172986effeaae34828ae034b669d96cee2c802a51' "$record" || return 1
  grep -Fq $'release-manifest.json\t950\tsha256:34e889cbc8427e299e0bc87f0c3c76f3fc8b7e0d88fe7ffeeb8fb1dbc061e521' "$record" || return 1
  grep -Fq 'stage run `34039157332`' "$record" || return 1
  grep -Fq 'AMD64 leaf `sha256:8350078aebf6542e976ad0219cf130cd6228b13b984867dc9dc36605f50e7d96`' "$record" || return 1
  grep -Fq 'ARM64 leaf `sha256:484b71f80b46f94080a13caa64c44b7a2d99de059a3ba92115cc3dae89e1deec`' "$record" || return 1
  grep -Fq 'The tagged installer produced `pangopup 0.4.0`.' "$record" || return 1
  grep -Fq 'Offline synchronization, status, lookup, model, cache, HTTP, and code-only uninstall checks passed against disposable copies of the retained production profile.' "$record" || return 1
  grep -Fq 'UNINSTALL_IO' "$record" || return 1
  grep -Fq 'remove entry receipt.json: Permission denied (os error 13)' "$record" || return 1
  grep -Fq 'Publication stopped before container finalization. No v0.4.0 container index was created.' "$record" || return 1
  grep -Fq 'canonical `MANIFEST_UNKNOWN` responses for GHCR `0.4.0` and `v0.4.0`' "$record" || return 1
  grep -Fq 'GHCR `latest`, `0.3.0`, and `v0.3.0` remained OCI index `sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`' "$record" || return 1
  if grep -Eqi '(authorization:[[:space:]]|bearer[[:space:]]+[a-z0-9]|ghp_[a-z0-9]|github_pat_[a-z0-9]|signed[_ -]?url)' "$record"; then
    return 1
  fi
}
[[ "$(sha256sum "$v040_release_notes" | cut -d' ' -f1)" == 729fa6ed9ddb641501f2abdf5e63cd2fd9861154a46f02967bea7ff408ce4aa9 ]]
check_v040_partial_record "$v040_publication_record" || fail 'v0.4.0 partial publication record is incomplete'
mutated_v040_record="$root/v0.4.0-publication-record-without-partial-state.md"
sed '/^State: \*\*PARTIAL /d' "$v040_publication_record" >"$mutated_v040_record"
if check_v040_partial_record "$mutated_v040_record"; then
  fail 'v0.4.0 publication-record check accepted a removed partial state'
fi
mutated_v040_inventory="$root/v0.4.0-publication-record-without-one-member.md"
sed '/^release-manifest[.]json[[:space:]]/d' "$v040_publication_record" >"$mutated_v040_inventory"
if check_v040_partial_record "$mutated_v040_inventory"; then
  fail 'v0.4.0 publication-record check accepted incomplete executable evidence'
fi
mutated_v040_tag="$root/v0.4.0-publication-record-without-direct-tag-binding.md"
sed 's/ and direct `refs\/tags\/v0[.]4[.]0` both/ and release target both/' \
  "$v040_publication_record" >"$mutated_v040_tag"
if check_v040_partial_record "$mutated_v040_tag"; then
  fail 'v0.4.0 publication-record check accepted missing direct-tag evidence'
fi
mutated_v040_index="$root/v0.4.0-publication-record-without-index-outcome.md"
sed 's/ Publication stopped before container finalization[.] No v0[.]4[.]0 container index was created[.]//' \
  "$v040_publication_record" >"$mutated_v040_index"
if check_v040_partial_record "$mutated_v040_index"; then
  fail 'v0.4.0 publication-record check accepted missing container stop outcome'
fi

candidate_release_notes="$repo/planning/artifacts/059-release-notes.md"
candidate_publication_record="$repo/planning/artifacts/060-public-v0.4.1.md"
grep -Fxq '# PangoPup v0.4.1 release notes' "$candidate_release_notes"
grep -Fq 'The HTTP, JSON, command-line, and scoring contracts do not change from v0.4.0 except for the reported software version and its derived scoring identity.' "$candidate_release_notes"
grep -Fq 'Scoring assets do not change.' "$candidate_release_notes"
grep -Fq 'pangopup uninstall --full --yes' "$candidate_release_notes"
grep -Fq 'measures warmed-query allocations only on the measured thread' "$candidate_release_notes"

check_v041_publication_record() {
  local record=$1
  grep -Fxq 'State: **PREPARED — no v0.4.1 tag, release, or container alias exists.**' "$record" || return 1
  grep -Fq 'release ID `383614742`, tag `v0.4.0`' "$record" || return 1
  grep -Fq 'ea4438e50762e32f09052b364060c89201ed78bc' "$record" || return 1
  grep -Fq 'GHCR `latest`, `0.3.0`, and `v0.3.0` to remain OCI index `sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0`' "$record" || return 1
  grep -Fq 'Require the v0.4.1 GitHub release and Git tag to be absent.' "$record" || return 1
  grep -Fq 'GHCR `0.4.1` and `v0.4.1` to return the canonical anonymous `MANIFEST_UNKNOWN` response.' "$record" || return 1
  grep -Fq 'release body: planning/artifacts/059-release-notes.md' "$record" || return 1
  grep -Fq 'publication date (UTC): 2026-09-06' "$record" || return 1
  grep -Fq 'Ian Maurer explicitly authorized building, deploying, and pushing PangoPup v0.4.1 on 2026-09-06.' "$record" || return 1
  grep -Fq 'The final publication commit cannot name itself inside this committed runbook.' "$record" || return 1
  grep -Fq 'obtain and retain an out-of-commit authorization receipt or user message from Ian Maurer' "$record" || return 1
  grep -Fq 'It must name the exact 40-character `origin/main` hash, authorization date, v0.4.1, and both executable and container publication.' "$record" || return 1
  grep -Fq 'A replacement commit requires a new receipt before any public effect.' "$record" || return 1
  grep -Fq 'The COMPLETE record must name the exact authorized commit and retained authorization evidence after publication.' "$record" || return 1
  ! grep -Fq 'REPLACE_WITH_EXACT_40_CHARACTER_ORIGIN_MAIN_COMMIT' "$record" || return 1
  grep -Fq 'Require every user-facing v0.4.1 container alias to remain absent.' "$record" || return 1
  grep -Fq 'Run `scripts/qualify-linux-release.sh <release-directory> 0.4.1 <publication-commit>`.' "$record" || return 1
  grep -Fq 'compare every remote asset name, size, and SHA-256 with the held local inventory' "$record" || return 1
  grep -Fq 'Before executable publication, require GitHub Latest to remain immutable release ID `383614742`' "$record" || return 1
  grep -Fq 'After executable publication, container finalization requires GitHub Latest to be the immutable v0.4.1 release at the selected publication commit.' "$record" || return 1
  grep -Fq 'PRE-EXECUTABLE-PUBLISH GATE: Immediately repeat the GitHub v0.4.0 Latest identity and direct-tag checks' "$record" || return 1
  grep -Fq 'private draft target/title/body/six-member inventory checks, and the UTC/CITATION date check.' "$record" || return 1
  grep -Fq 'PRE-INDEX-CREATION GATE: Immediately recheck the exact retained receipt, GitHub Latest v0.4.1 and its direct tag at the publication commit' "$record" || return 1
  grep -Fq 'the unchanged GHCR v0.3.0 predecessor under `latest`, and absence of GHCR `0.4.1` and `v0.4.1`.' "$record" || return 1
  grep -Fq 'require one immutable, Latest, non-draft, non-prerelease v0.4.1 release' "$record" || return 1
  grep -Fq 'pangopup uninstall --full --yes' "$record" || return 1
  grep -Fq 'authenticate the current workflow event, workflow source, and `origin/main` independently from the staged release commit' "$record" || return 1
  grep -Fq 'require `0.4.1`, `v0.4.1`, and `latest` to resolve to the same index digest' "$record" || return 1
  local -a ordered_steps=(
    'Require a clean checkout at the selected exact commit on `origin/main`'
    'Observe public repository visibility, immutable releases, read-only default Actions permissions'
    'Recheck the complete split predecessor state, v0.4.1 release and tag absence'
    'Dispatch `.github/workflows/publish-container.yml` on current `main` with `mode=stage`'
    'Download the unique unexpired receipt artifact through the Actions API'
    'Dispatch `.github/workflows/package-linux.yml` on current `main` with the exact publication commit.'
    'Run `scripts/qualify-linux-release.sh <release-directory> 0.4.1 <publication-commit>`.'
    'Require the manifest version and target commit, checksum, SBOM, executable version, notices, dynamic-library allowlist, and maximum GLIBC 2.39 check to pass.'
    'Create one private draft titled `PangoPup v0.4.1`'
    'If the date changed, preserve the staged leaves from the prior commit.'
    'PRE-EXECUTABLE-PUBLISH GATE:'
    'EXECUTABLE-PUBLISH ACTION:'
    'Use an absent private directory and non-root user in clean Ubuntu 24.04.'
    'Verify code-only and full uninstall in separate disposable trees.'
    'Only after the public executable and installer checks pass, dispatch `.github/workflows/publish-container.yml` with `mode=finalize`'
    'Re-admit the exact retained receipt and repeat anonymous native qualification of both held leaves.'
    'PRE-INDEX-CREATION GATE:'
    'INDEX-CREATION ACTION:'
    'Through fresh anonymous reads, require `0.4.1`, `v0.4.1`, and `latest` to resolve to the same index digest.'
    'If finalization fails after executable publication, preserve the immutable executable release and staged leaves.'
    'After every check passes, replace PREPARED with COMPLETE and append the authorization binding'
    'Update observed-current architecture and planning claims only after both delivery forms are public and qualified.'
  )
  local previous_step=0 step step_line
  for step in "${ordered_steps[@]}"; do
    [[ "$(grep -Fc "$step" "$record")" == 1 ]] || return 1
    step_line=$(grep -nF "$step" "$record" | cut -d: -f1)
    [[ "$step_line" -gt "$previous_step" ]] || return 1
    previous_step=$step_line
  done
  local stage package draft prepublish publish installer finalize preindex index_create evidence
  stage=$(grep -nF '## 2. Stage and admit native container leaves' "$record" | cut -d: -f1)
  package=$(grep -nF '## 3. Build and admit the executable' "$record" | cut -d: -f1)
  draft=$(grep -nF '## 4. Create, verify, and publish the executable release' "$record" | cut -d: -f1)
  prepublish=$(grep -nF 'PRE-EXECUTABLE-PUBLISH GATE:' "$record" | cut -d: -f1)
  publish=$(grep -nF 'EXECUTABLE-PUBLISH ACTION:' "$record" | cut -d: -f1)
  installer=$(grep -nF '## 5. Qualify the public installer and uninstall' "$record" | cut -d: -f1)
  finalize=$(grep -nF '## 6. Finalize and verify the container index' "$record" | cut -d: -f1)
  preindex=$(grep -nF 'PRE-INDEX-CREATION GATE:' "$record" | cut -d: -f1)
  index_create=$(grep -nF 'INDEX-CREATION ACTION:' "$record" | cut -d: -f1)
  evidence=$(grep -nF '## 7. Record final evidence' "$record" | cut -d: -f1)
  [[ "$stage" -lt "$package" && "$package" -lt "$draft" && "$draft" -lt "$prepublish" && "$prepublish" -lt "$publish" && "$publish" -lt "$installer" && "$installer" -lt "$finalize" && "$finalize" -lt "$preindex" && "$preindex" -lt "$index_create" && "$index_create" -lt "$evidence" ]] || return 1
  [[ "$publish" -eq $((prepublish + 2)) ]] || return 1
  [[ "$index_create" -eq $((preindex + 2)) ]] || return 1
  if grep -Eqi '(authorization:[[:space:]]|bearer[[:space:]]+[a-z0-9]|ghp_[a-z0-9]|github_pat_[a-z0-9]|signed[_ -]?url)' "$record"; then
    return 1
  fi
}
check_v041_publication_record "$candidate_publication_record" || fail 'v0.4.1 publication record is incomplete'
mutated_candidate_record="$root/v0.4.1-publication-record-without-prepared-state.md"
sed '/^State: \*\*PREPARED /d' "$candidate_publication_record" >"$mutated_candidate_record"
if check_v041_publication_record "$mutated_candidate_record"; then
  fail 'v0.4.1 publication-record check accepted a removed PREPARED state'
fi
mutated_candidate_predecessor="$root/v0.4.1-publication-record-with-wrong-container-predecessor.md"
sed 's/sha256:5d00753e9b5019e0408fd33ca39371684c1eebb38b3f559e2b4f953ce062bcc0/sha256:0000000000000000000000000000000000000000000000000000000000000000/g' \
  "$candidate_publication_record" >"$mutated_candidate_predecessor"
if check_v041_publication_record "$mutated_candidate_predecessor"; then
  fail 'v0.4.1 publication-record check accepted the wrong container predecessor'
fi
mutated_candidate_authority="$root/v0.4.1-publication-record-without-authority.md"
sed '/Ian Maurer explicitly authorized building, deploying, and pushing PangoPup v0[.]4[.]1/d' \
  "$candidate_publication_record" >"$mutated_candidate_authority"
if check_v041_publication_record "$mutated_candidate_authority"; then
  fail 'v0.4.1 publication-record check accepted missing authorization'
fi
mutated_candidate_draft="$root/v0.4.1-publication-record-without-private-inventory-check.md"
sed '/compare every remote asset name, size, and SHA-256 with the held local inventory/d' \
  "$candidate_publication_record" >"$mutated_candidate_draft"
if check_v041_publication_record "$mutated_candidate_draft"; then
  fail 'v0.4.1 publication-record check accepted an incomplete private-draft gate'
fi
mutated_candidate_publish_order="$root/v0.4.1-publication-record-with-separated-executable-gate.md"
sed '/^EXECUTABLE-PUBLISH ACTION:/iThe prepublication gate is no longer immediately adjacent.' \
  "$candidate_publication_record" >"$mutated_candidate_publish_order"
if check_v041_publication_record "$mutated_candidate_publish_order"; then
  fail 'v0.4.1 publication-record check accepted a separated executable publication gate'
fi
mutated_candidate_index_order="$root/v0.4.1-publication-record-with-separated-index-gate.md"
sed '/^INDEX-CREATION ACTION:/iThe pre-index gate is no longer immediately adjacent.' \
  "$candidate_publication_record" >"$mutated_candidate_index_order"
if check_v041_publication_record "$mutated_candidate_index_order"; then
  fail 'v0.4.1 publication-record check accepted a separated index creation gate'
fi
mutated_candidate_source_gate="$root/v0.4.1-publication-record-without-source-gate.md"
sed '/^Require a clean checkout at the selected exact commit on `origin\/main`/d' \
  "$candidate_publication_record" >"$mutated_candidate_source_gate"
if check_v041_publication_record "$mutated_candidate_source_gate"; then
  fail 'v0.4.1 publication-record check accepted a removed source and gate authentication step'
fi
mutated_candidate_stage="$root/v0.4.1-publication-record-without-stage-admission.md"
sed '/^Dispatch `[.]github\/workflows\/publish-container[.]yml` on current `main` with `mode=stage`/d' \
  "$candidate_publication_record" >"$mutated_candidate_stage"
if check_v041_publication_record "$mutated_candidate_stage"; then
  fail 'v0.4.1 publication-record check accepted a removed stage dispatch and run admission step'
fi
mutated_candidate_stage_order="$root/v0.4.1-publication-record-with-reordered-stage-and-package.md"
python3 - "$candidate_publication_record" "$mutated_candidate_stage_order" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_text().splitlines()
starts = (
    "Dispatch `.github/workflows/publish-container.yml` on current `main` with `mode=stage`",
    "Dispatch `.github/workflows/package-linux.yml` on current `main` with the exact publication commit.",
)
indices = [next(index for index, line in enumerate(source) if line.startswith(start)) for start in starts]
source[indices[0]], source[indices[1]] = source[indices[1]], source[indices[0]]
Path(sys.argv[2]).write_text("\n".join(source) + "\n")
PY
if check_v041_publication_record "$mutated_candidate_stage_order"; then
  fail 'v0.4.1 publication-record check accepted reordered stage and package actions'
fi
printf 'executable delivery tests passed\n'
