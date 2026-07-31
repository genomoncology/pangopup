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
  printf 'pangopup 0.1.0\n'
elif [[ "${1:-}" == --help ]]; then
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
[[ "$(wc -l <"$smoke_log")" == 5 ]]
grep -Fq -- "--model-cache $fake_cache_parent/model.sqlite3" "$smoke_log"
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
  "$real_cache_parent/model.sqlite3-shm" "$real_cache_parent/model.sqlite3-wal"
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
grep -Fq 'cargo build --locked --release --package pangopup-cli' "$repo/.github/workflows/package-linux.yml"
grep -Fq 'cargo install --locked --version 0.5.9 cargo-cyclonedx' "$repo/.github/workflows/package-linux.yml"
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
! grep -Eq '^    runs-on: ubuntu-22[.]04$' "$repo/.github/workflows/package-linux.yml"
! grep -Fq '"$maximum" 2.35' "$repo/scripts/qualify-linux-release.sh"
printf 'executable delivery tests passed\n'
