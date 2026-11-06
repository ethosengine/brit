#!/usr/bin/env bash
# Disposable adapter fixtures; --native additionally exercises the installed engine.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
native=
if [[ $# -gt 0 ]]; then
  [[ $# -eq 2 && $1 == --native && $2 == /* && -x $2 ]] || exit 2
  native=$2
fi
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
for name in ${!GIT_@}; do unset "$name"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME='Rust Style Fixture' GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
mkdir -p "$fixture/repo/scripts/ci" "$fixture/install/bin" "$fixture/path"
cp "$root/scripts/ci/rust-style.sh" "$fixture/repo/scripts/ci/rust-style.sh"
cat > "$fixture/repo/scripts/ci/install-rust-style.sh" <<'SH'
#!/usr/bin/env bash
[[ $1 == --verify ]] || exit 7
[[ ${STYLE_INSTALL_FAIL:-0} == 0 ]] || exit 7
printf '%s/bin/epr-rust-style\n' "$2"
SH
cat > "$fixture/install/bin/epr-rust-style" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$STYLE_ARGS"
exit "${STYLE_STATUS:-0}"
SH
cat > "$fixture/path/epr-rust-style" <<'SH'
#!/usr/bin/env bash
echo 'arbitrary PATH checker invoked' >&2
exit 99
SH
chmod +x "$fixture/install/bin/epr-rust-style" "$fixture/path/epr-rust-style"
export BRIT_RUST_STYLE_PREFIX="$fixture/install" STYLE_ARGS="$fixture/args"
export PATH="$fixture/path:$PATH"
adapter="$fixture/repo/scripts/ci/rust-style.sh"
cd "$fixture/repo"
git init -q --initial-branch=main
mkdir -p .epr-meta/rust-style/objects
printf '{"archive":".epr-meta/rust-style/objects/bafkr4iaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.tar.gz"}\n' > .epr-meta/rust-style/source.json
printf 'fixture holding\n' > .epr-meta/rust-style/objects/bafkr4iaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.tar.gz
git add scripts .epr-meta
git commit -qm baseline
base=$(git rev-parse HEAD)
git commit -qm candidate --allow-empty
head=$(git rev-parse HEAD)
bash "$adapter" --base "$base" --head "$head" --json
printf '%s\n' --root "$fixture/repo" --base "$base" --head "$head" --json > "$fixture/expected"
cmp "$fixture/expected" "$STYLE_ARGS"
expect_exit() {
  local expected=$1 actual=0
  shift
  "$@" > "$fixture/out" 2>&1 || actual=$?
  [[ $actual == "$expected" ]] || { cat "$fixture/out" >&2; echo "expected $expected, got $actual" >&2; exit 1; }
}
expect_exit 2 bash "$adapter" --head "$head"
expect_exit 2 bash "$adapter" --base main --head "$head"
expect_exit 2 bash "$adapter" --base "$base" --head HEAD
expect_exit 2 bash "$adapter" --base "$head" --head "$head"
expect_exit 2 bash "$adapter" --base 0000000000000000000000000000000000000000 --head "$head"
expect_exit 2 bash "$adapter" --scope gix
expect_exit 2 env BRIT_RUST_STYLE_PREFIX=relative bash "$adapter"
expect_exit 2 env STYLE_INSTALL_FAIL=1 bash "$adapter"
printf '\n' >> .epr-meta/rust-style/source.json
expect_exit 2 bash "$adapter" --base "$base" --head "$head"
git restore .epr-meta/rust-style/source.json
printf 'dirty archive\n' >> .epr-meta/rust-style/objects/bafkr4iaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.tar.gz
expect_exit 2 bash "$adapter" --base "$base" --head "$head"
git restore .epr-meta/rust-style/objects/bafkr4iaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.tar.gz
expect_exit 1 env STYLE_STATUS=1 bash "$adapter"
expect_exit 2 env STYLE_STATUS=2 bash "$adapter"
bash "$adapter"
printf '%s\n' --root "$fixture/repo" --base origin/main > "$fixture/expected"
cmp "$fixture/expected" "$STYLE_ARGS"
rm "$fixture/install/bin/epr-rust-style"
expect_exit 2 bash "$adapter"
echo 'passed: explicit installation, exact committed inputs and distribution holdings, native refusal propagation, no PATH fallback'

if [[ -z "$native" ]]; then
  echo 'not run: native trial proof (use --native with the installed checker)'
  exit 0
fi
cd "$fixture"
mkdir native-repo
cd native-repo
mkdir -p .epr-meta brit-build-ref/src
git init -q --initial-branch=main
printf '[package]\nname="trial-fixture"\nversion="0.1.0"\nedition="2021"\n' > brit-build-ref/Cargo.toml
printf '#[allow(dead_code)]\npub fn inherited_debt() {}\npub fn fixture() {}\n' > brit-build-ref/src/lib.rs
git add brit-build-ref
git commit -qm baseline
base=$(git rev-parse HEAD)
cp "$root/.epr-meta/rust-style-policy.json" .epr-meta/
cp -R "$root/.epr-meta/rust-style" .epr-meta/
guide_path=$(jq -er '.discipline.guide.path' .epr-meta/rust-style-policy.json)
[[ $guide_path == docs/rust-style-guide.md ]] || { echo 'unexpected guide projection path' >&2; exit 1; }
mkdir -p docs
cp "$root/$guide_path" "$guide_path"
git add .epr-meta "$guide_path"
git commit -qm 'explicit local trial enrollment'
enrollment=$(git rev-parse HEAD)
"$native" --root "$PWD" --base "$base" --head "$enrollment"
printf '#[allow(dead_code)]\npub fn fixture() {}\n' > brit-build-ref/src/lib.rs
git add brit-build-ref/src/lib.rs
git commit -qm 'unexplained suppression'
bad=$(git rev-parse HEAD)
expect_exit 1 "$native" --root "$PWD" --base "$enrollment" --head "$bad"
printf '#[allow(dead_code, reason = "RS-GOV-04: exported fixture entry point is called by an external harness")]\npub fn fixture() {}\n' > brit-build-ref/src/lib.rs
expect_exit 1 "$native" --root "$PWD" --base "$enrollment" --head "$bad"
git add brit-build-ref/src/lib.rs
git commit -qm 'explained suppression'
good=$(git rev-parse HEAD)
"$native" --root "$PWD" --base "$enrollment" --head "$good"
printf '\ncorrupt guide bytes\n' >> "$guide_path"
"$native" --root "$PWD" --base "$enrollment" --head "$good"
expect_exit 2 "$native" --root "$PWD" --base "$enrollment"
git add "$guide_path"
git commit -qm 'corrupt addressed guide'
corrupt=$(git rev-parse HEAD)
expect_exit 2 "$native" --root "$PWD" --base "$enrollment" --head "$corrupt"
echo 'passed: native enrollment retains inherited debt, new unexplained replacement refuses, dirty repair cannot mask committed failure, explained exception with paydown, corrupt guide refusal and committed closure isolation'
