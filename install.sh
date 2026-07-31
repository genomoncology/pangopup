#!/usr/bin/env bash
set -euo pipefail

repository="https://github.com/genomoncology/pangopup"
asset="pangopup-linux-x86_64"
version="latest"
install_dir="${PANGOPUP_INSTALL_DIR:-${HOME:-}/.local/bin}"
seen_version=0
seen_dir=0

fail() { printf 'pangopup installer: %s\n' "$*" >&2; exit 1; }

while (($#)); do
  case "$1" in
    --version)
      (( seen_version == 0 )) || fail "--version may be supplied only once"
      (($# >= 2)) || fail "--version requires a value"
      version=$2; seen_version=1; shift 2 ;;
    --install-dir)
      (( seen_dir == 0 )) || fail "--install-dir may be supplied only once"
      (($# >= 2)) || fail "--install-dir requires a value"
      install_dir=$2; seen_dir=1; shift 2 ;;
    *) fail "unknown argument: $1" ;;
  esac
done

[[ "$version" == latest || "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] \
  || fail "version must be latest or MAJOR.MINOR.PATCH"
[[ -n "$install_dir" && "$install_dir" == /* ]] || fail "install directory must be an absolute path"
[[ "$(uname -s)" == Linux ]] || fail "only Linux is supported"
case "$(uname -m)" in x86_64|amd64) ;; *) fail "only Linux x86_64 is supported" ;; esac
command -v mktemp >/dev/null || fail "mktemp is required"
command -v chmod >/dev/null || fail "chmod is required"
command -v stat >/dev/null || fail "stat is required"

if command -v curl >/dev/null; then downloader=curl
elif command -v wget >/dev/null; then downloader=wget
else fail "curl or wget is required"
fi
if command -v sha256sum >/dev/null; then checksum_tool=sha256sum
elif command -v shasum >/dev/null; then checksum_tool=shasum
elif command -v openssl >/dev/null; then checksum_tool=openssl
else fail "sha256sum, shasum, or openssl is required"
fi

if [[ "$version" == latest ]]; then
  base="$repository/releases/latest/download"
else
  base="$repository/releases/download/v$version"
fi

download_dir=$(mktemp -d "${TMPDIR:-/tmp}/pangopup-install.XXXXXXXX")
replacement=""
cleanup() {
  [[ -z "$replacement" || ! -e "$replacement" ]] || rm -f -- "$replacement"
  rm -rf -- "$download_dir"
}
trap cleanup EXIT HUP INT TERM

download() {
  local url=$1 destination=$2
  if [[ "$downloader" == curl ]]; then
    curl --fail --location --proto '=https' --tlsv1.2 --silent --show-error --output "$destination" "$url"
  else
    wget --https-only --secure-protocol=TLSv1_2 --quiet --output-document="$destination" "$url"
  fi
}

download "$base/$asset" "$download_dir/$asset"
download "$base/$asset.sha256" "$download_dir/$asset.sha256"
[[ -f "$download_dir/$asset" && ! -L "$download_dir/$asset" ]] || fail "downloaded executable is not a regular file"
[[ "$(stat -c '%h' "$download_dir/$asset")" == 1 ]] || fail "downloaded executable must have one hard link"

mapfile -t checksum_lines < "$download_dir/$asset.sha256"
(( ${#checksum_lines[@]} == 1 )) || fail "checksum file must contain exactly one record"
record=${checksum_lines[0]}
if [[ "$record" =~ ^([0-9A-Fa-f]{64})$ ]]; then expected=${BASH_REMATCH[1]}
elif [[ "$record" =~ ^([0-9A-Fa-f]{64})(\ \ |\ \*)pangopup-linux-x86_64$ ]]; then expected=${BASH_REMATCH[1]}
else fail "checksum record is malformed or names the wrong asset"
fi
expected=${expected,,}
case "$checksum_tool" in
  sha256sum) actual=$(sha256sum "$download_dir/$asset" | awk '{print tolower($1)}') ;;
  shasum) actual=$(shasum -a 256 "$download_dir/$asset" | awk '{print tolower($1)}') ;;
  openssl) actual=$(openssl dgst -sha256 -r "$download_dir/$asset" | awk '{print tolower($1)}') ;;
esac
[[ "$actual" == "$expected" ]] || fail "downloaded executable checksum does not match"

if [[ -e "$install_dir" || -L "$install_dir" ]]; then
  [[ -d "$install_dir" && ! -L "$install_dir" ]] || fail "install directory must be a real directory, not a symlink"
else
  mkdir -p -m 755 -- "$install_dir"
  [[ -d "$install_dir" && ! -L "$install_dir" ]] || fail "could not create a safe install directory"
fi
replacement=$(mktemp "$install_dir/.pangopup.XXXXXXXX")
cp -- "$download_dir/$asset" "$replacement"
chmod 0755 "$replacement"
observed=$("$replacement" --version) || fail "downloaded executable failed its version check"
[[ "$observed" =~ ^pangopup\ ((0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*))$ ]] \
  || fail "downloaded executable returned an invalid version"
resolved=${BASH_REMATCH[1]}
[[ "$version" == latest || "$resolved" == "$version" ]] || fail "downloaded executable version does not match requested version"
mv -fT -- "$replacement" "$install_dir/pangopup"
replacement=""

printf 'Installed pangopup %s at %s/pangopup\n' "$resolved" "$install_dir"
printf 'Release: %s/releases/tag/v%s\n' "$repository" "$resolved"
printf 'Source: %s/tree/v%s\n' "$repository" "$resolved"
printf 'License: %s/releases/download/v%s/LICENSE\n' "$repository" "$resolved"
printf 'Notice: %s/releases/download/v%s/NOTICE\n' "$repository" "$resolved"
case ":${PATH:-}:" in
  *":$install_dir:"*) ;;
  *)
    printf -v escaped_install_dir '%q' "$install_dir"
    printf 'Add Pangopup to PATH: export PATH=%s:"$PATH"\n' "$escaped_install_dir"
    ;;
esac
printf 'Next: pangopup sync\n'
printf 'Then: pangopup status\n'
