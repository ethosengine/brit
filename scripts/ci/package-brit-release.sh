#!/usr/bin/env bash
# Package one brit executable with an inspectable, non-authority build receipt.
set -euo pipefail

if [ "$#" -ne 8 ]; then
  echo "usage: $0 <binary> <source-root> <archive-base> <version-tag> <feature> <target> <profile> <source-sha>" >&2
  exit 2
fi

binary=$1
source_root=$2
archive_base=$3
version_tag=$4
feature=$5
target=$6
profile=$7
source_sha=$8

if ! [[ "$version_tag" =~ ^brit-v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "version tag must be brit-v<major>.<minor>.<patch>" >&2
  exit 2
fi
case "$feature" in
  max-pure|small|lean|max) ;;
  *) echo "unsupported frontend feature preset: $feature" >&2; exit 2 ;;
esac
case "$target" in
  x86_64-unknown-linux-gnu|x86_64-apple-darwin|aarch64-apple-darwin|x86_64-pc-windows-msvc) ;;
  *) echo "target is outside the initial host matrix: $target" >&2; exit 2 ;;
esac
if [ "$profile" != release-github ]; then
  echo "profile must be release-github" >&2
  exit 2
fi
expected_archive="brit-$feature-${version_tag#brit-}-$target"
if [ "$archive_base" != "$expected_archive" ]; then
  echo "archive base must be $expected_archive" >&2
  exit 2
fi
case "$source_sha" in
  *[!0-9a-f]*|'') echo "source SHA must be lowercase hex" >&2; exit 2 ;;
esac
if [ "${#source_sha}" -ne 40 ]; then
  echo "source SHA must be a full 40-character Git commit ID" >&2
  exit 2
fi
if [ ! -s "$binary" ] || [ ! -x "$binary" ] || [ ! -f "$source_root/Cargo.lock" ] || \
   [ ! -f "$source_root/README.md" ] || [ ! -f "$source_root/brit-cli/Cargo.toml" ]; then
  echo "missing executable binary or required tracked source input" >&2
  exit 2
fi

# Discard inherited Git redirection from a caller before attesting this checkout.
git_source() {
  env -u GIT_DIR -u GIT_WORK_TREE -u GIT_COMMON_DIR -u GIT_INDEX_FILE \
    -u GIT_OBJECT_DIRECTORY -u GIT_ALTERNATE_OBJECT_DIRECTORIES \
    git -C "$source_root" "$@"
}
source_root_abs=$(cd -- "$source_root" && pwd -P)
actual_root=$(git_source rev-parse --show-toplevel)
actual_root_abs=$(cd -- "$actual_root" && pwd -P)
if [ "$actual_root_abs" != "$source_root_abs" ] || [ "$(git_source rev-parse HEAD)" != "$source_sha" ]; then
  echo "source root or HEAD differs from the release receipt" >&2
  exit 2
fi
git_source diff --quiet
git_source diff --cached --quiet
if [ -n "$(git_source status --porcelain --untracked-files=all)" ]; then
  echo "source checkout is not clean" >&2
  exit 2
fi

case "$target" in
  *-windows-*) expected_name=brit.exe; extension=zip ;;
  *) expected_name=brit; extension=tar.gz ;;
esac
if [ "$(basename -- "$binary")" != "$expected_name" ]; then
  echo "expected $expected_name for target $target" >&2
  exit 2
fi

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -- "$1" | awk '{print $1}'
  else
    echo "no SHA-256 checksum tool available" >&2
    return 1
  fi
}

binary_sha=$(sha256_file "$binary")
lock_sha=$(sha256_file "$source_root/Cargo.lock")
mkdir -- "$archive_base"
cp -- "$binary" "$archive_base/$expected_name"
cp -- "$source_root/README.md" "$archive_base/"
for license in "$source_root"/LICENSE-*; do
  [ -f "$license" ] || continue
  cp -- "$license" "$archive_base/"
done

{
  printf 'format=brit-binary-archive-v1\n'
  printf 'binary=%s\n' "$expected_name"
  printf 'binary_sha256=%s\n' "$binary_sha"
  printf 'package=brit-cli\n'
  printf 'version_tag=%s\n' "$version_tag"
  printf 'source_commit=%s\n' "$source_sha"
  printf 'source_checkout=clean-at-packaging\n'
  printf 'cargo_lock_sha256=%s\n' "$lock_sha"
  printf 'cargo_package=brit-cli\n'
  printf 'cargo_bin=brit\n'
  printf 'cargo_no_default_features=true\n'
  printf 'cargo_feature_preset=%s\n' "$feature"
  printf 'target=%s\n' "$target"
  printf 'profile=%s\n' "$profile"
} > "$archive_base/BUILD-INFO.txt"

if [ "$extension" = zip ]; then
  7z a -tzip "$archive_base.zip" "$archive_base" >/dev/null
else
  tar -czf "$archive_base.tar.gz" -- "$archive_base"
fi
asset="$archive_base.$extension"
asset_sha=$(sha256_file "$asset")
printf '%s  %s\n' "$asset_sha" "$asset" > "$asset.sha256"
printf 'ASSET=%s\nASSET_SUM=%s\n' "$asset" "$asset.sha256"
