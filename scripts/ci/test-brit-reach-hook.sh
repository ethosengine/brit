#!/usr/bin/env bash
# Isolated adapter mechanics; EPR_TEST_BIN adds a real installed native-floor proof.
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
for name in ${!GIT_@}; do unset "$name"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME='Reach Fixture' GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
mkdir "$fixture/bin" "$fixture/repo"
export REACH_TEST_CALLS="$fixture/calls" REACH_TEST_MODE=pass
cat > "$fixture/bin/epr" <<'SH'
#!/bin/sh
printf '%s\n' "$@" >> "$REACH_TEST_CALLS"
case "$REACH_TEST_MODE" in
  fail) exit 7 ;;
  dirty) printf 'concurrent edit\n' >> tracked ;;
  untracked) printf 'concurrent authority\n' > untracked ;;
  head) git reset --soft HEAD^ ;;
  branch) git checkout -qb moved ;;
  baseline) git config branch.main.britReachBase HEAD ;;
  hidden) git update-index --assume-unchanged .epr-meta/manifest.md ;;
esac
SH
chmod +x "$fixture/bin/epr"
export PATH="$fixture/bin:$PATH"
cd "$fixture/repo"
git init -q --initial-branch=main
mkdir .githooks
cp "$repo_root/.githooks/pre-push" .githooks/pre-push
chmod +x .githooks/pre-push
printf 'base\n' > tracked
printf 'private/\n' > .gitignore
printf 'fixture\n' > AGENTS.md
printf 'fixture\n' > CLAUDE.md
mkdir .epr-meta
printf '%s\n' '---' 'epr-meta-version: 1' 'root: true' '---' > .epr-meta/manifest.md
git add .githooks/pre-push tracked .gitignore AGENTS.md CLAUDE.md .epr-meta/manifest.md
git commit -qm base
base=$(git rev-parse HEAD)
printf 'candidate\n' >> tracked
git add tracked
git commit -qm candidate
candidate=$(git rev-parse HEAD)
zero=0000000000000000000000000000000000000000
tuple="refs/heads/main $candidate refs/heads/main $base"
new_tuple="refs/heads/main $candidate refs/heads/main $zero"
hook=$PWD/.githooks/pre-push
run_hook() { printf '%s' "$1" | "$hook" origin "$fixture/remote.git" > "$fixture/out" 2>&1; }
reject() {
  if run_hook "$1"; then echo "unexpected hook acceptance: $2" >&2; exit 1; fi
  echo "passed: $2"
}
run_hook "$tuple"
printf '%s\n' --repo "$PWD" ready --target "$base" > "$fixture/expected"
cmp "$fixture/expected" "$REACH_TEST_CALLS"
echo 'passed: exact installed-evaluator argument binding'
rm "$REACH_TEST_CALLS"
run_hook ''
test ! -e "$REACH_TEST_CALLS"
reject "$tuple"$'\n'"$tuple" 'multiple updates'
reject "refs/heads/main $zero refs/heads/main $base" 'deletion'
reject "refs/heads/other $candidate refs/heads/main $base" 'non-HEAD source'
reject "HEAD $candidate refs/heads/main $base" 'literal HEAD source'
reject "refs/heads/main $candidate refs/tags/tag $base" 'non-branch target'
reject "refs/heads/main short refs/heads/main $base" 'short object ID'
reject "$tuple extra" 'malformed tuple'
reject "refs/heads/main $base refs/heads/main $base" 'stale source ID'
reject "refs/heads/main $candidate refs/heads/main 1111111111111111111111111111111111111111" 'missing baseline object'
reject "$new_tuple" 'first push without baseline'
git config branch.main.britReachBase "$candidate"
reject "$new_tuple" 'first-push empty comparison'
git config branch.main.britReachBase "$base"
run_hook "$new_tuple"
echo 'passed: explicit first-push ancestor baseline'
mkdir private
printf 'ignored private state\n' > private/state
run_hook "$tuple"
printf 'dirty\n' >> tracked
reject "$tuple" 'dirty tracked file'
git restore tracked
printf 'staged\n' >> tracked
git add tracked
reject "$tuple" 'dirty index'
git reset -q HEAD -- tracked
git restore tracked
touch untracked
reject "$tuple" 'untracked authority'
rm untracked
for flag in assume-unchanged skip-worktree; do
  git update-index --"$flag" .epr-meta/manifest.md
  printf '\nconcealed authority edit\n' >> .epr-meta/manifest.md
  cp .epr-meta/manifest.md "$fixture/hidden-expected"
  git ls-files -v .epr-meta/manifest.md > "$fixture/flags-expected"
  rm -f "$REACH_TEST_CALLS"
  reject "$tuple" "hidden authority with $flag"
  test ! -e "$REACH_TEST_CALLS"
  cmp "$fixture/hidden-expected" .epr-meta/manifest.md
  git ls-files -v .epr-meta/manifest.md > "$fixture/flags-actual"
  cmp "$fixture/flags-expected" "$fixture/flags-actual"
  git update-index --no-"$flag" .epr-meta/manifest.md
  git restore .epr-meta/manifest.md
done
other=$(printf 'unrelated\n' | git commit-tree "$(git rev-parse HEAD^{tree})")
reject "refs/heads/main $candidate refs/heads/main $other" 'unrelated baseline'
for mode in fail dirty untracked head branch baseline hidden; do
  export REACH_TEST_MODE=$mode
  reject "$new_tuple" "evaluator $mode refusal or mutation"
  git update-index --no-assume-unchanged .epr-meta/manifest.md
  git checkout -q main
  git reset -q --hard "$candidate"
  rm -f untracked
  git config branch.main.britReachBase "$base"
done
export REACH_TEST_MODE=pass

# An actual push proves evaluator rejection cannot mutate the remote.
git init --bare -q "$fixture/remote.git"
git config core.hooksPath .githooks
export REACH_TEST_MODE=fail
if git push -q "$fixture/remote.git" main > "$fixture/out" 2>&1; then
  echo 'rejecting evaluator allowed remote mutation' >&2; exit 1
fi
test -z "$(git --git-dir="$fixture/remote.git" for-each-ref)"
export REACH_TEST_MODE=pass
git push -q "$fixture/remote.git" main
test "$(git --git-dir="$fixture/remote.git" rev-parse refs/heads/main)" = "$candidate"
echo 'passed: actual push refusal preserves remote, acceptance transfers exact source'

if [[ -n ${EPR_TEST_BIN:-} ]]; then
  test -x "$EPR_TEST_BIN"
  rm "$fixture/bin/epr"
  ln -s "$EPR_TEST_BIN" "$fixture/bin/epr"
  epr setup > "$fixture/setup.json"
  epr doctor > "$fixture/doctor.json"
  epr ready --target "$base" > "$fixture/ready.json"
  printf '%s\n' '---' 'epr-meta-version: 1' 'root: false' '---' > .epr-meta/manifest.md
  git add .epr-meta/manifest.md
  git commit -qm 'invalid native root'
  if git push -q "$fixture/remote.git" main > "$fixture/out" 2>&1; then
    echo 'real native evaluator allowed invalid governance root' >&2; exit 1
  fi
  test "$(git --git-dir="$fixture/remote.git" rev-parse refs/heads/main)" = "$candidate"
  echo 'passed: real installed epr setup/doctor/readiness and invalid-root push refusal'
else
  echo 'not run: real installed epr proof (set EPR_TEST_BIN); fake evaluator proves adapter mechanics only'
fi
