#!/usr/bin/env bash
# Select a real ancestor for the checked-out CI source, never a self-baseline.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
cd "$root"
fail() { printf 'Brit Rust style CI UNMEASURED: %s\n' "$*" >&2; exit 2; }
head=${GITHUB_SHA:-}
[[ "$head" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]] || fail 'GITHUB_SHA must be a full commit ID'
[[ $(git rev-parse --verify HEAD) == "$head" ]] || fail 'checkout does not match the CI source'
base=${EVENT_BASE:-}
if [[ -z "$base" || "$base" =~ ^0+$ ]]; then
  base=$(git rev-parse --verify origin/main^{commit}) || fail 'fetch the main comparison target'
  if [[ "$base" == "$head" ]]; then
    base=$(git rev-parse --verify "$head^1^{commit}") || fail 'initial source has no ancestor; supply an explicit reviewed baseline'
  fi
fi
[[ "$base" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ && "$base" != "$head" ]] || fail 'baseline must be a distinct full commit ID'
git merge-base --is-ancestor "$base" "$head" || fail 'comparison baseline must exist and be an ancestor of the CI source'
exec bash "$root/scripts/ci/rust-style.sh" --base "$base" --head "$head"
