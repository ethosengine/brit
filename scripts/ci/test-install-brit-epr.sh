#!/usr/bin/env bash
# Synthetic fault fixtures, distinct from installed product acceptance.
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) target=x86_64-unknown-linux-gnu ;;
  Darwin/x86_64) target=x86_64-apple-darwin ;;
  Darwin/arm64) target=aarch64-apple-darwin ;;
  *) exit 2 ;;
esac
source_a=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
source_b=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
mkdir "$fixture/package a"
cat > "$fixture/package a/brit" <<SCRIPT
#!/usr/bin/env bash
if [ "\${1:-}" = --version ]; then printf 'brit 0.1.5\\nsource HEAD: $source_a\\n'; fi
SCRIPT
chmod +x "$fixture/package a/brit"
brit_hash=$(sha256_file "$fixture/package a/brit")
cat > "$fixture/package a/BUILD-INFO.txt" <<RECEIPT
format=brit-binary-archive-v1
package=brit-cli
binary=brit
target=$target
source_commit=$source_a
binary_sha256=$brit_hash
version_tag=brit-v0.1.5
source_checkout=clean-at-packaging
RECEIPT
cat > "$fixture/epr a" <<'SCRIPT'
#!/usr/bin/env bash
if [ -n "${PAIR_FIXTURE_BLOCK:-}" ]; then
  touch "$PAIR_FIXTURE_BLOCK/waiting"
  while [ ! -e "$PAIR_FIXTURE_BLOCK/continue" ]; do sleep 0.05; done
fi
if [ "${1:-}" = --help ]; then echo 'ready [--target REF]';
else echo 'integrate --acceptance <CID>'; fi
SCRIPT
cp "$fixture/epr a" "$fixture/epr b"
printf '\n# second EPR candidate\n' >> "$fixture/epr b"
chmod +x "$fixture/epr a" "$fixture/epr b"
epr_a_hash=$(sha256_file "$fixture/epr a")
epr_b_hash=$(sha256_file "$fixture/epr b")
prefix="$fixture/private prefix"
install() { bash "$script_dir/install-brit-epr.sh" "$fixture/package a" "$source_a" "$brit_hash" "$1" "$2" "$3" "$prefix" >/dev/null; }
check_pair() {
  test "$(sha256_file "$prefix/current/bin/brit")" = "$brit_hash"
  test "$(sha256_file "$prefix/current/bin/epr")" = "$1"
}
install "$fixture/epr a" "$source_a" "$epr_a_hash"
check_pair "$epr_a_hash"
old=$(readlink "$prefix/current")
install "$fixture/epr b" "$source_b" "$epr_b_hash"
check_pair "$epr_b_hash"
test -x "$old/bin/epr"
install "$fixture/epr a" "$source_a" "$epr_a_hash"
check_pair "$epr_a_hash"
active=$(readlink "$prefix/current")
retained() { test "$(readlink "$prefix/current")" = "$active"; check_pair "$epr_a_hash"; }
refuse() { if install "$@" > /dev/null 2>&1; then echo 'invalid pair accepted' >&2; exit 1; fi; retained; }
refuse "$fixture/epr b" "$source_b" "$epr_a_hash"
refuse "$fixture/epr b" short "$epr_b_hash"
printf '#!/bin/sh\necho old-interface\n' > "$fixture/old epr"
chmod +x "$fixture/old epr"
refuse "$fixture/old epr" "$source_b" "$(sha256_file "$fixture/old epr")"
printf '#!/bin/sh\necho "ready [--target REF]"\n' > "$fixture/old epr"
refuse "$fixture/old epr" "$source_b" "$(sha256_file "$fixture/old epr")"
cp "$fixture/package a/BUILD-INFO.txt" "$fixture/receipt"
printf 'source_commit=wrong\n' >> "$fixture/package a/BUILD-INFO.txt"
refuse "$fixture/epr b" "$source_b" "$epr_b_hash"
cp "$fixture/receipt" "$fixture/package a/BUILD-INFO.txt"
mkdir "$fixture/block"
PAIR_FIXTURE_BLOCK="$fixture/block" bash "$script_dir/install-brit-epr.sh" \
  "$fixture/package a" "$source_a" "$brit_hash" "$fixture/epr b" "$source_b" "$epr_b_hash" "$prefix" > "$fixture/interruption.log" 2>&1 &
installer_pid=$!
for attempt in $(seq 1 200); do
  [ ! -e "$fixture/block/waiting" ] || break
  sleep 0.05
done
test -e "$fixture/block/waiting"
kill -TERM "$installer_pid"
touch "$fixture/block/continue"
if wait "$installer_pid"; then echo 'interrupted pair install succeeded' >&2; exit 1; fi
retained
mkdir "$fixture/unmanaged"
mkdir "$fixture/foreign prefix"
ln -s "$fixture/unmanaged" "$fixture/foreign prefix/current"
prefix="$fixture/foreign prefix"
if install "$fixture/epr a" "$source_a" "$epr_a_hash" >/dev/null 2>&1; then exit 1; fi
test "$(readlink "$prefix/current")" = "$fixture/unmanaged"
printf 'pair-install-fixtures=passed (pins, rollback, refusal, interruption, spaced paths; synthetic, no human acceptance)\n'
