#!/usr/bin/env bash
# Real two-artifact Unix upgrade/rollback probe. Callers supply independently
# accepted source/hash pins. This is fixture evidence, never two-human acceptance.
set -euo pipefail
if [ "$#" -ne 7 ]; then
  echo 'usage: verify-brit-upgrade.sh <old-package> <old-source> <old-sha256> <new-package> <new-source> <new-sha256> <private-prefix>' >&2
  exit 2
fi
old_package=$1 old_source=$2 old_hash=$3
new_package=$4 new_source=$5 new_hash=$6 prefix=$7
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
[ "$old_source" != "$new_source" ] && [ "$old_hash" != "$new_hash" ] || { echo 'upgrade requires two distinct source-bound artifacts' >&2; exit 2; }
old_version=$(awk -F= '$1 == "version_tag" { print $2 }' "$old_package/BUILD-INFO.txt")
new_version=$(awk -F= '$1 == "version_tag" { print $2 }' "$new_package/BUILD-INFO.txt")
[ "$old_version" != "$new_version" ] || { echo 'upgrade requires distinct release versions' >&2; exit 2; }
[ ! -e "$prefix" ] || { echo 'upgrade probe requires a fresh private prefix' >&2; exit 2; }
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
for name in ${!GIT_@}; do unset "$name"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME='Brit Upgrade Fixture' GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
mkdir "$fixture/repo"
git -C "$fixture/repo" init -q --initial-branch=main
printf 'original\n' > "$fixture/repo/scoped"
printf 'neighbor\n' > "$fixture/repo/neighbor"
git -C "$fixture/repo" add scoped neighbor
git -C "$fixture/repo" commit -qm baseline
printf 'staged\n' > "$fixture/repo/scoped"
git -C "$fixture/repo" add scoped
printf 'unstaged\n' > "$fixture/repo/scoped"
printf 'untracked\n' > "$fixture/repo/untracked"
snapshot() {
  (cd "$fixture/repo" && find . -type f | LC_ALL=C sort | while IFS= read -r path; do
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$path"; else shasum -a 256 "$path"; fi
  done)
}
snapshot > "$fixture/before"
install() {
  bash "$script_dir/install-brit-release.sh" "$1" "$prefix" "$2" "$3"
  snapshot > "$fixture/after"
  cmp "$fixture/before" "$fixture/after"
}
install "$old_package" "$old_source" "$old_hash"
install "$new_package" "$new_source" "$new_hash"
bash "$script_dir/verify-installed-brit.sh" "$(cd "$prefix" && pwd -P)/bin/brit"
install "$old_package" "$old_source" "$old_hash"
printf 'upgrade-rollback-fixture=passed\nold_source=%s\nnew_source=%s\nold_binary_sha256=%s\nnew_binary_sha256=%s\nhuman_acceptance=not-established\n' \
  "$old_source" "$new_source" "$old_hash" "$new_hash"
