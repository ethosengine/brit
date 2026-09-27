#!/usr/bin/env bash
# Local archive-shape proof; creates no release and needs no Cargo build.
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
fixture_dir=$(mktemp -d)
trap 'rm -r -- "$fixture_dir"' EXIT

mkdir -p -- "$fixture_dir/source/brit-cli" "$fixture_dir/output"
printf 'fixture executable\n' > "$fixture_dir/brit"
chmod +x -- "$fixture_dir/brit"
cp -- "$fixture_dir/brit" "$fixture_dir/brit.exe"
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
  'brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/brit' | sort > expected-members.txt
diff -u expected-members.txt members.txt

receipt=$(tar -xOzf brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu.tar.gz \
  brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu/BUILD-INFO.txt)
printf '%s\n' "$receipt" | grep -Fx "source_commit=$source_sha"
printf '%s\n' "$receipt" | grep -Fx 'cargo_feature_preset=max-pure'
printf '%s\n' "$receipt" | grep -Fx "binary_sha256=$(sha256sum "$fixture_dir/brit" | awk '{print $1}')"

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
    'brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/brit.exe' | sort > windows-expected-members.txt
  diff -u windows-expected-members.txt windows-members.txt
  unzip -p brit-max-pure-v0.1.1-x86_64-pc-windows-msvc.zip \
    brit-max-pure-v0.1.1-x86_64-pc-windows-msvc/BUILD-INFO.txt | \
    grep -Fx "source_commit=$source_sha"
else
  echo "ZIP fixture skipped: 7z or unzip unavailable" >&2
fi

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
printf 'dirty bytes\n' >> "$fixture_dir/source/README.md"
if bash "$script_dir/package-brit-release.sh" \
  "$fixture_dir/brit" "$fixture_dir/source" brit-max-pure-v0.1.1-x86_64-unknown-linux-gnu \
  brit-v0.1.1 max-pure x86_64-unknown-linux-gnu release-github "$source_sha" \
  >/dev/null 2>&1; then
  echo "dirty source was accepted" >&2
  exit 1
fi
echo "brit archive helper fixture passed"
