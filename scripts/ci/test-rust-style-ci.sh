#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
for name in ${!GIT_@}; do unset "$name"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME='Style CI Fixture' GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
mkdir -p "$fixture/scripts/ci"
cp "$root/scripts/ci/rust-style-ci.sh" "$fixture/scripts/ci/"
cat > "$fixture/scripts/ci/rust-style.sh" <<'SH'
printf '%s\n' "$@" > "$CI_STYLE_ARGS"
SH
export CI_STYLE_ARGS="$fixture/args"
cd "$fixture"
git init -q --initial-branch=main
git add scripts
git commit -qm initial
initial=$(git rev-parse HEAD)
git update-ref refs/remotes/origin/main "$initial"
reject() {
  if "$@" > "$fixture/out" 2>&1; then echo 'unexpected CI baseline acceptance' >&2; exit 1; fi
}
reject env GITHUB_SHA="$initial" EVENT_BASE= bash scripts/ci/rust-style-ci.sh
git commit -qm candidate --allow-empty
candidate=$(git rev-parse HEAD)
git update-ref refs/remotes/origin/main "$candidate"
for event_base in '' 0000000000000000000000000000000000000000 "$initial"; do
  GITHUB_SHA=$candidate EVENT_BASE=$event_base bash scripts/ci/rust-style-ci.sh
  printf '%s\n' --base "$initial" --head "$candidate" > "$fixture/expected"
  cmp "$fixture/expected" "$CI_STYLE_ARGS"
done
reject env GITHUB_SHA="$candidate" EVENT_BASE="$candidate" bash scripts/ci/rust-style-ci.sh
reject env GITHUB_SHA="$initial" EVENT_BASE="$candidate" bash scripts/ci/rust-style-ci.sh
other=$(printf 'unrelated\n' | git commit-tree "$(git rev-parse HEAD^{tree})")
reject env GITHUB_SHA="$candidate" EVENT_BASE="$other" bash scripts/ci/rust-style-ci.sh
git update-ref refs/remotes/origin/main "$initial"
GITHUB_SHA=$candidate EVENT_BASE= bash scripts/ci/rust-style-ci.sh
cmp "$fixture/expected" "$CI_STYLE_ARGS"
echo 'passed: main/manual/first-push use a real ancestor; self, absent ancestor, unrelated and stale source refuse'
