#!/usr/bin/env bash
# Disposable source fixtures and the workflow's publication boundary; no forge writes.
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
for name in ${!GIT_@}; do unset "$name"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME='Brit Delivery Fixture' GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
mkdir -p "$fixture/source/brit-cli"
git -C "$fixture/source" init -q --initial-branch=main
printf '[package]\nname = "brit-cli"\nversion = "0.1.5"\n' > "$fixture/source/brit-cli/Cargo.toml"
git -C "$fixture/source" add brit-cli/Cargo.toml
git -C "$fixture/source" commit -qm baseline
base=$(git -C "$fixture/source" rev-parse HEAD)
git -C "$fixture/source" branch old
printf 'candidate\n' > "$fixture/source/feature"
git -C "$fixture/source" add feature
git -C "$fixture/source" commit -qm candidate
source_sha=$(git -C "$fixture/source" rev-parse HEAD)
git -C "$fixture/source" tag -a brit-v0.1.5 -m release
git -C "$fixture/source" tag brit-v9.9.9
probe() {
  bash "$script_dir/validate-brit-delivery-source.sh" "$fixture/source" "$@"
}
refuse() {
  if probe "$@" > "$fixture/refusal" 2>&1; then
    echo "unexpected acceptance: $*" >&2
    exit 1
  fi
}
probe workflow_dispatch refs/heads/main "$source_sha" > "$fixture/candidate"
printf 'source_sha=%s\nversion_tag=brit-v0.1.5\npublish=false\n' "$source_sha" > "$fixture/expected"
cmp "$fixture/expected" "$fixture/candidate"
probe push refs/tags/brit-v0.1.5 "$source_sha" > "$fixture/release"
printf 'source_sha=%s\nversion_tag=brit-v0.1.5\npublish=true\n' "$source_sha" > "$fixture/expected"
cmp "$fixture/expected" "$fixture/release"
refuse workflow_dispatch refs/tags/brit-v0.1.5 "$source_sha"
refuse push refs/heads/main "$source_sha"
refuse pull_request refs/heads/main "$source_sha"
refuse push refs/tags/brit-v9.9.9 "$source_sha"
refuse workflow_dispatch refs/heads/main "$base"
refuse workflow_dispatch refs/heads/old "$source_sha"
refuse workflow_dispatch refs/heads/main "${source_sha:0:12}"
refuse workflow_dispatch refs/heads/missing "$source_sha"
printf 'dirty\n' >> "$fixture/source/feature"
refuse workflow_dispatch refs/heads/main "$source_sha"
git -C "$fixture/source" checkout -- feature
printf 'untracked\n' > "$fixture/source/untracked"
refuse workflow_dispatch refs/heads/main "$source_sha"
rm "$fixture/source/untracked"
# Check the forge boundary itself, not just helper behavior. No write permission
# or release mutation may escape the push-only publish job. Job names and the
# gate are deliberately checked together so moving a step cannot silently weaken it.
awk '
  /^  [a-z][a-z-]*:$/ { job = $1; sub(/:$/, "", job) }
  /^    if: github.event_name == '\''push'\'' && startsWith\(github.ref, '\''refs\/tags\/brit-v'\''\) && needs.prepare.outputs.publish == '\''true'\''$/ {
    if (job != "publish") exit 1
    guard++
  }
  /contents: write|gh release / {
    if (job != "publish" || guard != 1) exit 1
    boundary++
  }
  /uses: actions\/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a/ {
    if (job != "build") exit 1
    artifact++
  }
  END { if (guard != 1 || boundary != 3 || artifact != 1) exit 1 }
' "$repo_root/.github/workflows/brit-release.yml"
printf 'delivery-source-fixtures=passed (candidate, annotated release tag, 10 refusals)\npublication-boundary=push-only; candidates retain workflow artifacts\n'
