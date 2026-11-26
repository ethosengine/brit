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
  Linux/x86_64) host=x86_64-unknown-linux-gnu; rename_option=-T ;;
  Darwin/x86_64) host=x86_64-apple-darwin; rename_option=-h ;;
  Darwin/arm64) host=aarch64-apple-darwin; rename_option=-h ;;
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
tools=()
tool_shas=()
# Older archives predate both tools. Any tool file or receipt entry requires the full pair.
if [ -e "$package/brit-build-ref" ] || [ -L "$package/brit-build-ref" ] || \
   [ -e "$package/brit-verify" ] || [ -L "$package/brit-verify" ] || \
   grep -Eq '^brit_(build_ref|verify)_(binary|sha256)=' "$receipt"; then
  tools=(brit-build-ref brit-verify)
  for tool in "${tools[@]}"; do
    key=${tool//-/_}
    [ "$(field "${key}_binary")" = "$tool" ] || fail "missing or invalid receipt name for $tool"
    tool_sha=$(field "${key}_sha256") || fail "missing or duplicate receipt checksum for $tool"
    [[ "$tool_sha" =~ ^[0-9a-f]{64}$ ]] || fail "invalid receipt checksum for $tool"
    [ -f "$package/$tool" ] && [ ! -L "$package/$tool" ] && [ -s "$package/$tool" ] && [ -x "$package/$tool" ] \
      || fail "missing regular executable $tool"
    [ "$(sha256_file "$package/$tool")" = "$tool_sha" ] || fail "$tool checksum mismatch"
    tool_shas+=("$tool_sha")
  done
fi
[ ! -L "$prefix" ] || fail 'prefix must not be a symbolic link'
mkdir -p -- "$prefix"
prefix=$(cd -- "$prefix" && pwd -P)
[ ! -L "$prefix/bin" ] && [ ! -L "$prefix/releases" ] || fail 'installation directories must not be symbolic links'
mkdir -p -- "$prefix/bin" "$prefix/releases"
[ ! -d "$prefix/bin/brit" ] || fail 'active executable must not name a directory'
current=$prefix/current
release_for_binary() {
  local target=$1 generation dir
  # The prefix is physical (pwd -P). A link written through a symbolic parent, such as
  # macOS's /var -> /private/var, names the same release by another spelling, so compare
  # the release directory physically. The release directory itself must not be a link.
  dir=${target%/brit}
  [ "$dir" != "$target" ] && [ -d "$dir" ] && [ ! -L "$dir" ] || return 1
  dir=$(cd -- "$dir" && pwd -P) || return 1
  target=$dir/brit
  case "$target" in "$prefix/releases/"*) ;; *) return 1 ;; esac
  generation=${target#"$prefix/releases/"}
  [[ "$generation" =~ ^[0-9a-f]{40}\.[a-zA-Z0-9]+/brit$ ]] &&
    [ -f "$target" ] && [ ! -L "$target" ] && [ ! -L "${target%/brit}" ]
}
if [ -L "$current" ]; then
  release_for_binary "$(readlink "$current")/brit" || fail 'current link must name a managed release'
else
  [ ! -e "$current" ] || fail 'refusing to replace an unmanaged current path'
fi
old_brit=$(readlink "$prefix/bin/brit" 2>/dev/null || true)
if [ -n "$old_brit" ] && [ "$old_brit" != ../current/brit ]; then
  release_for_binary "$old_brit" || fail 'active executable must name a managed release'
elif [ -z "$old_brit" ] && [ -e "$prefix/bin/brit" ]; then
  fail 'refusing to replace an unmanaged executable'
fi
for tool in "${tools[@]+"${tools[@]}"}"; do
  if [ -e "$prefix/bin/$tool" ] || [ -L "$prefix/bin/$tool" ]; then
    [ "$(readlink "$prefix/bin/$tool")" = "../current/$tool" ] || fail "refusing to replace unmanaged $tool"
  fi
done
stage=$(mktemp -d "$prefix/releases/$source_pin.XXXXXXXX")
activation_attempted=false
cleanup() {
  # A concurrent activation may already have replaced current after our rename.
  if [ "$activation_attempted" = false ]; then
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
i=0
for tool in "${tools[@]+"${tools[@]}"}"; do
  cp -- "$package/$tool" "$stage/$tool"
  [ -x "$stage/$tool" ] && [ "$(sha256_file "$stage/$tool")" = "${tool_shas[i]}" ] \
    || fail "copied $tool is not executable or its checksum differs"
  i=$((i + 1))
done
version_output=$(cd -- "$stage" && ./brit --version)
[ "${version_output%%$'\n'*}" = "brit ${version_tag#brit-v}" ] || fail 'executable version differs from receipt'
printf '%s\n' "$version_output" | grep -Fx "source HEAD: $source_pin" >/dev/null || fail 'executable source differs from accepted pin'
(cd -- "$stage" && ./brit --help >/dev/null && ./brit build --help >/dev/null)
# Starting --help did not catch the Shem TLS SIGILL. Qualify the copied bytes
# before activation; any missing prerequisite, signal or timeout retains current.
if [ "$host" = x86_64-unknown-linux-gnu ]; then
  script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
  bash "$script_dir/verify-native-https.sh" "$stage/brit" || fail 'native HTTPS qualification failed; current retained'
fi
link_tool() {
  local tool=$1 destination=$prefix/bin/$1 target=../current/$1
  [ "$(readlink "$destination" 2>/dev/null || true)" = "$target" ] ||
    ln -sn "$target" "$destination" ||
    [ "$(readlink "$destination" 2>/dev/null || true)" = "$target" ]
}
# Migrate a direct link from the previous installer through current while it still
# points to the old release. Future activations update every tool with one rename.
if [ -n "$old_brit" ] && [ "$old_brit" != ../current/brit ]; then
  if [ ! -L "$current" ]; then
    ln -sn "${old_brit%/brit}" "$current" || [ -L "$current" ]
  fi
  ln -s ../current/brit "$stage/legacy-brit"
  mv -f "$rename_option" -- "$stage/legacy-brit" "$prefix/bin/brit"
else
  link_tool brit || fail 'could not link brit'
fi
for tool in "${tools[@]+"${tools[@]}"}"; do
  link_tool "$tool" || fail "could not link $tool"
done
ln -s "$stage" "$stage/activate"
activation_attempted=true
mv -f "$rename_option" -- "$stage/activate" "$current"
printf 'installed_source=%s\ninstalled_binary_sha256=%s\ninstalled_binary=%s/bin/brit\n' "$source_pin" "$binary_pin" "$prefix"
