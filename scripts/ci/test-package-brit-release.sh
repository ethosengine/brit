#!/usr/bin/env bash
# Local archive-shape proof; creates no release and needs no Cargo build.
set -euo pipefail
unset BRIT_REBASE_RETAINED_BIN

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
fixture_dir=$(mktemp -d)
trap 'rm -r -- "$fixture_dir"' EXIT

mkdir -p -- "$fixture_dir/source/brit-cli" "$fixture_dir/output"
printf 'fixture executable\n' > "$fixture_dir/brit"
chmod +x -- "$fixture_dir/brit"
cp -- "$fixture_dir/brit" "$fixture_dir/brit.exe"
for tool in brit-build-ref brit-verify; do
  printf '%s executable\n' "$tool" > "$fixture_dir/$tool"
  chmod +x -- "$fixture_dir/$tool"
  cp -- "$fixture_dir/$tool" "$fixture_dir/$tool.exe"
done
printf 'fixture readme\n' > "$fixture_dir/source/README.md"
printf 'fixture license\n' > "$fixture_dir/source/LICENSE-MIT"
printf 'fixture lockfile\n' > "$fixture_dir/source/Cargo.lock"
printf '[package]\nname = "brit-cli"\nversion = "0.1.1"\n' > "$fixture_dir/source/brit-cli/Cargo.toml"

# The fixture is an isolated Git checkout with no inherited Git path/config
# override; the source SHA is an independently committed value.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES
git -C "$fixture_dir/source" -c init.defaultBranch=main init -q
git -C "$fixture_dir/source" add Cargo.lock README.md LICENSE-MIT brit-cli/Cargo.toml
git -C "$fixture_dir/source" -c user.name='Brit Fixture' -c user.email='fixture@example.invalid' \
  commit -qm 'fixture source'

source_sha=$(git -C "$fixture_dir/source" rev-parse HEAD)
cd -- "$fixture_dir/output"
bash "$script_dir/package-brit-release.sh" \
  "$fixture_dir/brit" "$fixture_dir/source" \
  brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu \
  brit-v0.1.1 max-pure x86_64-unknown-linux-gnu release-github "$source_sha" \
  > receipt.env

test -s brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu.tar.gz
sha256sum -c brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -tzf brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu.tar.gz | sort > members.txt
printf '%s\n' \
  'brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/' \
  'brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/BUILD-INFO.txt' \
  'brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/LICENSE-MIT' \
  'brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/README.md' \
  'brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/brit' \
  'brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/brit-build-ref' \
  'brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/brit-verify' | sort > expected-members.txt
diff -u expected-members.txt members.txt

receipt=$(tar -xOzf brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu.tar.gz \
  brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/BUILD-INFO.txt)
printf '%s\n' "$receipt" | grep -Fx "source_commit=$source_sha"
printf '%s\n' "$receipt" | grep -Fx 'cargo_feature_preset=max-pure'
printf '%s\n' "$receipt" | grep -Fx "binary_sha256=$(sha256sum "$fixture_dir/brit" | awk '{print $1}')"

release_tools_match_receipt() {
  local package=$1 suffix=${2:-} tool filename key
  for tool in brit-build-ref brit-verify; do
    filename=$tool$suffix
    key=${tool//-/_}
    test -f "$package/$filename"
    test ! -L "$package/$filename"
    test -x "$package/$filename"
    cmp "$fixture_dir/$filename" "$package/$filename"
    grep -Fx "${key}_binary=$filename" "$package/BUILD-INFO.txt"
    grep -Fx "${key}_sha256=$(sha256sum "$fixture_dir/$filename" | awk '{print $1}')" "$package/BUILD-INFO.txt"
  done
}
mkdir extracted
tar -xzf brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu.tar.gz -C extracted
release_tools_match_receipt extracted/brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu
for target in x86_64-apple-darwin aarch64-apple-darwin; do
  archive=brit-max-pure-v0.1.1-$target
  bash "$script_dir/package-brit-release.sh" "$fixture_dir/brit" "$fixture_dir/source" "$archive" \
    brit-v0.1.1 max-pure "$target" release-github "$source_sha" >/dev/null
  tar -xzf "$archive.tar.gz" -C extracted
  release_tools_match_receipt "extracted/$archive"
done

if command -v 7z >/dev/null 2>&1 && command -v unzip >/dev/null 2>&1; then
  bash "$script_dir/package-brit-release.sh" \
    "$fixture_dir/brit.exe" "$fixture_dir/source" \
    brit-max-pure-v0.1.1-x86_64-pc-windows-msvc \
    brit-v0.1.1 max-pure x86_64-pc-windows-msvc release-github "$source_sha" \
    > windows-receipt.env
  sha256sum -c brit-max-pure-v0.1.1-x86_64-pc-windows-msvc.zip.sha256
  unzip -Z1 brit-max-pure-v0.1.1-x86_64-pc-windows-msvc.zip | sort > windows-members.txt
  printf '%s\n' \
    'brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/' \
    'brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/BUILD-INFO.txt' \
    'brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/LICENSE-MIT' \
    'brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/README.md' \
    'brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/brit.exe' \
    'brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/brit-build-ref.exe' \
    'brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/brit-verify.exe' | sort > windows-expected-members.txt
  diff -u windows-expected-members.txt windows-members.txt
  unzip -p brit-max-pure-v0.1.1-x86_64-pc-windows-msvc.zip \
    brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/BUILD-INFO.txt | \
    grep -Fx "source_commit=$source_sha"
  unzip -q brit-max-pure-v0.1.1-x86_64-pc-windows-msvc.zip -d extracted
  release_tools_match_receipt extracted/brit-max-pure-v0.1.1-x86_64-pc-windows-msvc .exe
else
  echo "ZIP fixture skipped: 7z or unzip unavailable" >&2
fi

release_package_rejects_invalid_tool() {
  if bash "$script_dir/package-brit-release.sh" "$fixture_dir/invalid/brit" "$fixture_dir/source" \
    brit-lean-v0.1.1-x86_64-unknown-linux-gnu \
    brit-v0.1.1 lean x86_64-unknown-linux-gnu release-github "$source_sha" >invalid-tool.out 2>&1; then
    echo "invalid brit-verify accepted" >&2
    exit 1
  fi
  grep -F 'regular executable brit-verify' invalid-tool.out
}
mkdir "$fixture_dir/invalid"
cp "$fixture_dir/brit" "$fixture_dir/brit-build-ref" "$fixture_dir/invalid/"
release_package_rejects_invalid_tool
ln -s "$fixture_dir/brit-verify" "$fixture_dir/invalid/brit-verify"
release_package_rejects_invalid_tool
rm "$fixture_dir/invalid/brit-verify"
printf 'not executable\n' > "$fixture_dir/invalid/brit-verify"
release_package_rejects_invalid_tool

if bash "$script_dir/package-brit-release.sh" \
  "$fixture_dir/brit" "$fixture_dir/source" invalid-name \
  brit-v0.1.1 max-pure x86_64-unknown-linux-gnu release-github "$source_sha" \
  >/dev/null 2>&1; then
  echo "invalid archive name was accepted" >&2
  exit 1
fi
if bash "$script_dir/package-brit-release.sh" \
  "$fixture_dir/brit" "$fixture_dir/source" brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu \
  brit-v0.1.1 max-pure x86_64-unknown-linux-gnu release-github 0000000000000000000000000000000000000000 \
  >/dev/null 2>&1; then
  echo "wrong source SHA was accepted" >&2
  exit 1
fi
if bash "$script_dir/package-brit-release.sh" \
  "$fixture_dir/brit" "$fixture_dir/source" brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu \
  $'brit-v0.1.1\nsource_commit=forged' max-pure x86_64-unknown-linux-gnu release-github "$source_sha" \
  >/dev/null 2>&1; then
  echo "receipt field injection was accepted" >&2
  exit 1
fi
# Optional helper packaging preserves the legacy archive when omitted.
mkdir "$fixture_dir/helper-output"
printf 'retained-copy helper fixture\n' > "$fixture_dir/brit-rebase-retained"
chmod +x "$fixture_dir/brit-rebase-retained"
(
  cd "$fixture_dir/helper-output"
  BRIT_REBASE_RETAINED_BIN="$fixture_dir/brit-rebase-retained" bash "$script_dir/package-brit-release.sh" \
    "$fixture_dir/brit" "$fixture_dir/source" brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu \
    brit-v0.1.1 max-pure x86_64-unknown-linux-gnu release-github "$source_sha" > receipt.env
  sha256sum -c brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu.tar.gz.sha256
  tar -xzf brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu.tar.gz
  helper_package=brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu
  test -f "$helper_package/brit-rebase-retained"
  test ! -L "$helper_package/brit-rebase-retained"
  test -x "$helper_package/brit-rebase-retained"
  cmp "$fixture_dir/brit-rebase-retained" "$helper_package/brit-rebase-retained"
  grep -Fx 'retained_helper_binary=brit-rebase-retained' "$helper_package/BUILD-INFO.txt"
  grep -Fx "retained_helper_sha256=$(sha256sum "$fixture_dir/brit-rebase-retained" | awk '{print $1}')" "$helper_package/BUILD-INFO.txt"
)
mkdir "$fixture_dir/linked-helper"
ln -s "$fixture_dir/brit-rebase-retained" "$fixture_dir/linked-helper/brit-rebase-retained"
if BRIT_REBASE_RETAINED_BIN="$fixture_dir/linked-helper/brit-rebase-retained" bash "$script_dir/package-brit-release.sh" \
  "$fixture_dir/brit" "$fixture_dir/source" brit-small-v0.1.1-x86_64-unknown-linux-gnu \
  brit-v0.1.1 small x86_64-unknown-linux-gnu release-github "$source_sha" >/dev/null 2>&1; then
  echo "symlink helper was accepted" >&2
  exit 1
fi
if BRIT_REBASE_RETAINED_BIN="$fixture_dir/brit-rebase-retained" bash "$script_dir/package-brit-release.sh" \
  "$fixture_dir/brit" "$fixture_dir/source" brit-small-v0.1.1-x86_64-apple-darwin \
  brit-v0.1.1 small x86_64-apple-darwin release-github "$source_sha" >/dev/null 2>&1; then
  echo "unqualified platform helper was accepted" >&2
  exit 1
fi

printf 'dirty bytes\n' >> "$fixture_dir/source/README.md"
if bash "$script_dir/package-brit-release.sh" \
  "$fixture_dir/brit" "$fixture_dir/source" brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu \
  brit-v0.1.1 max-pure x86_64-unknown-linux-gnu release-github "$source_sha" \
  >/dev/null 2>&1; then
  echo "dirty source was accepted" >&2
  exit 1
fi
echo "brit archive helper fixture passed"
