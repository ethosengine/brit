#!/usr/bin/env bash
# Execute the followed-release workflow's shell steps with local build/forge fixtures.
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
workflow="$script_dir/../../.github/workflows/brit-build-artifacts.yml"
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
for name in ${!GIT_@}; do unset "$name"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME='Brit Fixture' GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
mkdir -p "$fixture/bin" "$fixture/source"
git -C "$fixture/source" init -q --initial-branch=main
git -C "$fixture/source" commit -qm fixture --allow-empty
export SOURCE_SHA
SOURCE_SHA=$(git -C "$fixture/source" rev-parse HEAD)
export CARGO_TARGET_DIR="$fixture/target" GITHUB_RUN_ID=1 GITHUB_RUN_ATTEMPT=1
export FIXTURE_ROOT="$fixture"
cat > "$fixture/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
mkdir -p "$CARGO_TARGET_DIR/release-github"
while [ "$#" -gt 0 ]; do
  if [ "$1" = --bin ]; then
    shift
    printf '#!/bin/sh\nprintf "source HEAD: %%s\\n" "$SOURCE_SHA"\n' > "$CARGO_TARGET_DIR/release-github/$1"
    chmod +x "$CARGO_TARGET_DIR/release-github/$1"
  fi
  shift
done
SH
cat > "$fixture/bin/gh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
case "$1 $2" in
  'release create')
    for arg in "$@"; do
      case "$arg" in artifacts/*) test -f "$arg"; echo "$arg" >> "$FIXTURE_ROOT/uploaded" ;; esac
    done ;;
  'release view')
    [ "$#" -gt 3 ] || exit 1
    case "$*" in *isDraft*) echo true ;; *) wc -l < "$FIXTURE_ROOT/uploaded" ;; esac ;;
  'release edit') test -s "$FIXTURE_ROOT/uploaded" ;;
  *) exit 1 ;;
esac
SH
chmod +x "$fixture/bin/"*
export PATH="$fixture/bin:$PATH"
step() {
  awk -v wanted="$1" '
    /^      - name:/ { active = index($0, wanted) > 0; body = 0 }
    active && /^        run: \|/ { body = 1; next }
    body { sub(/^          /, ""); print }
  ' "$workflow" > "$fixture/step.sh"
  test -s "$fixture/step.sh"
  bash "$fixture/step.sh"
}
cd "$fixture/source"
step 'Build the followed executables'
step 'Verify and checksum the source-pinned bytes'
step 'Stage and publish the source build prerelease'
printf '%s\n' artifacts/SHA256SUMS artifacts/brit artifacts/brit-build-ref artifacts/brit-rebase-retained artifacts/brit-verify > "$fixture/expected"
sort "$fixture/uploaded" > "$fixture/actual"
cmp "$fixture/expected" "$fixture/actual"
printf corrupt >> artifacts/brit-rebase-retained
if (cd artifacts && sha256sum -c SHA256SUMS) > /dev/null 2>&1; then
  echo 'corrupt retained helper accepted' >&2; exit 1
fi
echo 'followed release: four built, checksummed and uploaded executables; corruption refused'
