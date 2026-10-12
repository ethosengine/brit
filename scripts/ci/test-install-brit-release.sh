#!/usr/bin/env bash
# Synthetic installer fault fixtures, never product or human qualification.
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) target=x86_64-unknown-linux-gnu ;;
  Darwin/x86_64) target=x86_64-apple-darwin ;;
  Darwin/arm64) target=aarch64-apple-darwin ;;
  *) echo 'Unix installer fixtures require a declared Unix host' >&2; exit 2 ;;
esac
sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}
source_a=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
source_b=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
make_package() {
  local directory=$1 source=$2 version=$3
  mkdir "$directory"
  cat > "$directory/brit" <<SCRIPT
#!/usr/bin/env bash
set -eu
if [ "\${1:-}" = --version ]; then
  printf 'brit $version\\nsource HEAD: $source\\n'
elif [ "\${1:-}" = --help ] && [ -n "\${BRIT_FIXTURE_BLOCK_DIR:-}" ]; then
  touch "\$BRIT_FIXTURE_BLOCK_DIR/waiting"
  while [ ! -e "\$BRIT_FIXTURE_BLOCK_DIR/continue" ]; do sleep 0.05; done
fi
SCRIPT
  chmod +x "$directory/brit"
  cat > "$directory/BUILD-INFO.txt" <<RECEIPT
format=brit-binary-archive-v1
package=brit-cli
binary=brit
source_commit=$source
binary_sha256=$(sha256_file "$directory/brit")
version_tag=brit-v$version
source_checkout=clean-at-packaging
target=$target
RECEIPT
}
make_package "$fixture/a" "$source_a" 0.1.0
make_package "$fixture/b" "$source_b" 0.2.0
hash_a=$(sha256_file "$fixture/a/brit")
hash_b=$(sha256_file "$fixture/b/brit")
install() { bash "$script_dir/install-brit-release.sh" "$1" "$fixture/prefix" "$2" "$3" >/dev/null; }
install "$fixture/a" "$source_a" "$hash_a"
test "$(sha256_file "$fixture/prefix/bin/brit")" = "$hash_a"
install "$fixture/b" "$source_b" "$hash_b"
test "$(sha256_file "$fixture/prefix/bin/brit")" = "$hash_b"
install "$fixture/a" "$source_a" "$hash_a"
test "$(sha256_file "$fixture/prefix/bin/brit")" = "$hash_a"
active=$(readlink "$fixture/prefix/bin/brit")
assert_retained() { test "$(readlink "$fixture/prefix/bin/brit")" = "$active"; test "$(sha256_file "$fixture/prefix/bin/brit")" = "$hash_a"; }
refuse() { if install "$@" >/dev/null 2>&1; then echo 'invalid installation accepted' >&2; exit 1; fi; assert_retained; }
refuse "$fixture/b" "$source_a" "$hash_b"
refuse "$fixture/b" "$source_b" "$hash_a"
printf 'tampered bytes\n' >> "$fixture/b/brit"
refuse "$fixture/b" "$source_b" "$hash_b"
rm -rf "$fixture/b"
make_package "$fixture/b" "$source_b" 0.2.0
printf 'source_commit=%s\n' "$source_b" >> "$fixture/b/BUILD-INFO.txt"
refuse "$fixture/b" "$source_b" "$hash_b"
rm -rf "$fixture/b"
make_package "$fixture/b" "$source_b" 0.2.0
# Metadata alone cannot turn identical code into another released version.
sed 's/version_tag=brit-v0.2.0/version_tag=brit-v9.9.9/' "$fixture/b/BUILD-INFO.txt" > "$fixture/false-version"
cp "$fixture/b/BUILD-INFO.txt" "$fixture/original-receipt"
cp "$fixture/false-version" "$fixture/b/BUILD-INFO.txt"
refuse "$fixture/b" "$source_b" "$hash_b"
cp "$fixture/original-receipt" "$fixture/b/BUILD-INFO.txt"
# POSIX mv follows an existing directory destination; refuse that shape rather
# than moving an activation link inside an unrelated directory.
mkdir -p "$fixture/directory-prefix/bin" "$fixture/foreign-directory"
printf 'foreign bytes\n' > "$fixture/foreign-directory/sentinel"
ln -s "$fixture/foreign-directory" "$fixture/directory-prefix/bin/brit"
if bash "$script_dir/install-brit-release.sh" "$fixture/b" "$fixture/directory-prefix" "$source_b" "$hash_b" >/dev/null 2>&1; then
  echo 'active directory symlink was accepted' >&2; exit 1
fi
test "$(readlink "$fixture/directory-prefix/bin/brit")" = "$fixture/foreign-directory"
test "$(cat "$fixture/foreign-directory/sentinel")" = 'foreign bytes'
test "$(find "$fixture/foreign-directory" -type f -o -type l | wc -l | tr -d ' ')" = 1
mkdir "$fixture/block"
BRIT_FIXTURE_BLOCK_DIR="$fixture/block" bash "$script_dir/install-brit-release.sh" \
  "$fixture/b" "$fixture/prefix" "$source_b" "$hash_b" >"$fixture/interrupted.out" 2>&1 &
installer_pid=$!
for attempt in $(seq 1 200); do
  [ ! -e "$fixture/block/waiting" ] || break
  sleep 0.05
done
test -e "$fixture/block/waiting"
kill -TERM "$installer_pid"
touch "$fixture/block/continue"
if wait "$installer_pid"; then echo 'interrupted installation reported success' >&2; exit 1; fi
assert_retained
install "$fixture/b" "$source_b" "$hash_b"
test "$(sha256_file "$fixture/prefix/bin/brit")" = "$hash_b"
printf 'installer-fault-fixtures=passed (synthetic A/B; not released-version or human acceptance)\n'
