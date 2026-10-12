#!/usr/bin/env bash
# Install an already-extracted Unix release into an explicit private prefix.
# Pins must come from independently accepted release evidence; BUILD-INFO is not
# an authenticity proof. No checkout, Cargo, registry, or user PATH mutation.
set -euo pipefail
if [ "$#" -ne 4 ]; then
  echo 'usage: install-brit-release.sh <extracted-package> <prefix> <source-sha> <binary-sha256>' >&2
  exit 2
fi
package=$1
prefix=$2
source_pin=$3
binary_pin=$4
fail() { echo "Brit installation refused: $*" >&2; exit 2; }
[[ "$source_pin" =~ ^[0-9a-f]{40}$ ]] || fail 'expected a full source revision pin'
[[ "$binary_pin" =~ ^[0-9a-f]{64}$ ]] || fail 'expected an independently supplied binary checksum'
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) host=x86_64-unknown-linux-gnu ;;
  Darwin/x86_64) host=x86_64-apple-darwin ;;
  Darwin/arm64) host=aarch64-apple-darwin ;;
  *) fail 'atomic installation is qualified only for the declared Unix host targets' ;;
esac
sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}
receipt=$package/BUILD-INFO.txt
[ -f "$receipt" ] && [ ! -L "$receipt" ] || fail 'missing regular BUILD-INFO.txt'
[ -f "$package/brit" ] && [ ! -L "$package/brit" ] && [ -x "$package/brit" ] || fail 'missing regular executable brit'
receipt_pin=$(sha256_file "$receipt")
field() {
  awk -F= -v key="$1" '$1 == key { count++; value=substr($0,length(key)+2) } END { if(count != 1) exit 2; print value }' "$receipt"
}
[ "$(field format)" = brit-binary-archive-v1 ] || fail 'unsupported or duplicate receipt format'
[ "$(field package)" = brit-cli ] && [ "$(field binary)" = brit ] || fail 'wrong executable package'
[ "$(field target)" = "$host" ] || fail 'archive does not match this host'
[ "$(field source_commit)" = "$source_pin" ] || fail 'source revision differs from accepted pin'
[ "$(field binary_sha256)" = "$binary_pin" ] || fail 'receipt checksum differs from accepted pin'
version_tag=$(field version_tag)
[[ "$version_tag" =~ ^brit-v[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail 'invalid release version'
[ "$(field source_checkout)" = clean-at-packaging ] || fail 'release does not declare a clean packaging source'
[ "$(sha256_file "$package/brit")" = "$binary_pin" ] || fail 'binary checksum mismatch'
[ ! -L "$prefix" ] || fail 'prefix must not be a symbolic link'
mkdir -p -- "$prefix"
prefix=$(cd -- "$prefix" && pwd -P)
[ ! -L "$prefix/bin" ] && [ ! -L "$prefix/releases" ] || fail 'installation directories must not be symbolic links'
mkdir -p -- "$prefix/bin" "$prefix/releases"
[ ! -e "$prefix/bin/brit" ] || [ -L "$prefix/bin/brit" ] || fail 'refusing to replace an unmanaged executable'
[ ! -d "$prefix/bin/brit" ] || fail 'active executable must not name a directory'
stage=$(mktemp -d "$prefix/releases/$source_pin.XXXXXXXX")
link=$stage/activate
cleanup() {
  # A signal after rename must not remove the newly active release.
  if [ "$(readlink "$prefix/bin/brit" 2>/dev/null || true)" != "$stage/brit" ]; then
    rm -rf -- "$stage"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
cp -- "$package/brit" "$stage/brit"
cp -- "$receipt" "$stage/BUILD-INFO.txt"
[ "$(sha256_file "$stage/BUILD-INFO.txt")" = "$receipt_pin" ] || fail 'receipt changed during installation'
[ "$(sha256_file "$stage/brit")" = "$binary_pin" ] || fail 'copied binary checksum mismatch'
version_output=$(cd -- "$stage" && ./brit --version)
[ "${version_output%%$'\n'*}" = "brit ${version_tag#brit-v}" ] || fail 'executable version differs from receipt'
printf '%s\n' "$version_output" | grep -Fx "source HEAD: $source_pin" >/dev/null || fail 'executable source differs from accepted pin'
(cd -- "$stage" && ./brit --help >/dev/null && ./brit build --help >/dev/null)
ln -s "$stage/brit" "$link"
# Both paths are on the prefix filesystem; replacing a file symlink is atomic.
# Never point at a directory: mv would otherwise follow it on some Unix hosts.
mv -f -- "$link" "$prefix/bin/brit"
printf 'installed_source=%s\ninstalled_binary_sha256=%s\ninstalled_binary=%s/bin/brit\n' "$source_pin" "$binary_pin" "$prefix"
