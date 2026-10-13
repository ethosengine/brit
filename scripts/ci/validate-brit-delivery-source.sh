#!/usr/bin/env bash
# Resolve one clean, exact-source delivery. Manual dispatch never authorizes publication.
set -euo pipefail
if [ "$#" -ne 4 ]; then
  echo 'usage: validate-brit-delivery-source.sh <source-root> <event> <full-ref> <source-sha>' >&2
  exit 2
fi
source_root=$1 event=$2 source_ref=$3 source_sha=$4
for name in ${!GIT_@}; do unset "$name"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
cd -- "$source_root"
[[ "$source_sha" =~ ^[0-9a-f]{40}$ ]] || { echo 'expected a full source commit ID' >&2; exit 2; }
test "$(git rev-parse HEAD)" = "$source_sha"
test "$(git rev-parse "$source_ref^{commit}")" = "$source_sha"
test -z "$(git status --porcelain --untracked-files=all)"
version=$(awk '/^\[package\]/{inside=1; next} inside && /^version[[:space:]]*=/{gsub(/[" ]/, "", $3); print $3; exit}' brit-cli/Cargo.toml)
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'expected a three-part Brit package version' >&2; exit 2; }
publish=false
case "$event:$source_ref" in
  workflow_dispatch:refs/heads/*) ;;
  push:refs/tags/brit-v*)
    test "$source_ref" = "refs/tags/brit-v$version"
    publish=true
    ;;
  *) echo 'delivery requires manual branch dispatch or a matching brit-v tag push' >&2; exit 2 ;;
esac
printf 'source_sha=%s\nversion_tag=brit-v%s\npublish=%s\n' "$source_sha" "$version" "$publish"
