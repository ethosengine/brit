#!/usr/bin/env bash
# Local adapter only; policy resolution and all Rust diagnostics belong to the native checker.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
fail() { printf 'Brit Rust style UNMEASURED: %s\n' "$*" >&2; exit 2; }
base=origin/main
head=
json=false
base_set=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --base) [[ $# -ge 2 && -n "$2" ]] || fail '--base requires a ref'; base=$2; base_set=true; shift 2 ;;
    --head) [[ $# -ge 2 && -n "$2" ]] || fail '--head requires a full commit ID'; head=$2; shift 2 ;;
    --json) json=true; shift ;;
    *) fail 'usage: rust-style.sh [--base REF] [--head FULL-COMMIT-ID] [--json]' ;;
  esac
done
if [[ -n "$head" ]]; then
  [[ "$head" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]] || fail 'committed source must be a full commit ID'
  [[ "$base_set" == true && "$base" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]] || fail 'committed checking requires an explicit full baseline commit ID'
  [[ "$head" != "$base" && ! "$head" =~ ^0+$ && ! "$base" =~ ^0+$ ]] || fail 'source and baseline must be different nonzero commits'
  # The installed source receipt is local, but its declaration must belong to
  # this exact committed candidate. Dirty distribution bytes cannot supply reach.
  committed_holding_matches() {
    local relative=$1 entry committed held
    entry=$(git -C "$root" ls-tree "$head" -- "$relative") || fail 'cannot inspect committed checker holding'
    [[ "$entry" == '100644 blob '* || "$entry" == '100755 blob '* ]] || fail "missing or nonregular committed checker holding: $relative"
    [[ -f "$root/$relative" && ! -L "$root/$relative" ]] || fail "missing or symlinked checker holding: $relative"
    committed=$(git -C "$root" show "$head:$relative" | sha256sum) || fail 'cannot hash committed checker holding'
    held=$(sha256sum "$root/$relative") || fail 'cannot hash local checker holding'
    [[ "${committed%% *}" == "${held%% *}" ]] || fail "checker holding differs from committed source: $relative"
  }
  for directory in .epr-meta .epr-meta/rust-style .epr-meta/rust-style/objects; do
    [[ ! -L "$root/$directory" ]] || fail 'checker holding traverses a symlink'
  done
  declaration=.epr-meta/rust-style/source.json
  committed_holding_matches "$declaration"
  archive=$(jq -er '.archive | select(type == "string")' "$root/$declaration") || fail 'invalid checker archive declaration'
  [[ "$archive" =~ ^\.epr-meta/rust-style/objects/bafkrei[a-z2-7]+\.tar\.gz$ ]] || fail 'checker archive must be an addressed local holding'
  committed_holding_matches "$archive"
fi
prefix=${BRIT_RUST_STYLE_PREFIX:-"$root/.eprfs/tools/rust-style"}
[[ "$prefix" == /* ]] || fail 'BRIT_RUST_STYLE_PREFIX must be absolute'
binary=$(bash "$root/scripts/ci/install-rust-style.sh" --verify "$prefix") || fail 'install or repair the pinned checker with scripts/ci/install-rust-style.sh <absolute-prefix>'
[[ "$binary" == "$prefix/bin/epr-rust-style" && -x "$binary" ]] || fail 'installer returned an unexpected checker path'
args=(--root "$root" --base "$base")
[[ -z "$head" ]] || args+=(--head "$head")
[[ "$json" == false ]] || args+=(--json)
exec "$binary" "${args[@]}"
