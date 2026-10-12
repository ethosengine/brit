#!/usr/bin/env bash
# Operational proof from an installed binary in a fresh, source-free fixture.
set -euo pipefail
[ "$#" -eq 1 ] || { echo 'usage: verify-installed-brit.sh <absolute-installed-binary>' >&2; exit 2; }
binary=$1
case "$binary" in /*) ;; *) echo 'installed binary must be an absolute path' >&2; exit 2 ;; esac
[ -x "$binary" ] || { echo 'installed binary is not executable' >&2; exit 2; }
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
# All Git overrides belong to this subprocess only. Config and path redirection
# from a parent hook must never select the real checkout as the test fixture.
for name in ${!GIT_@}; do unset "$name"; done
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME='Installed Brit Fixture' GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME" GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
export GIT_TERMINAL_PROMPT=0
mkdir "$fixture/repo"
cd -- "$fixture/repo"
git init -q --initial-branch=main
printf 'staged bytes\n' > scoped.txt
printf 'untouched neighbor\n' > neighbor.txt
git add scoped.txt
expected_tree=$(git write-tree)
printf 'unstaged bytes\n' > scoped.txt
"$binary" commit -m '  installed authoring  ' --cleanup=whitespace >/dev/null
test "$(git rev-parse HEAD^{tree})" = "$expected_tree"
test "$(cat scoped.txt)" = 'unstaged bytes'
test "$(cat neighbor.txt)" = 'untouched neighbor'
printf '# discard\ninstalled cleanup  \n' > "$fixture/message"
"$binary" commit --allow-empty --cleanup=strip -F "$fixture/message" >/dev/null
test "$(git log -1 --format=%B)" = 'installed cleanup'
git init --bare -q "$fixture/remote.git"
git remote add origin "$fixture/remote.git"
printf '#!/bin/sh\ncat > hook-input\nexit 7\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-push
if "$binary" push origin refs/heads/main:refs/heads/main >"$fixture/push.out" 2>"$fixture/push.err"; then
  echo 'installed binary bypassed rejecting pre-push hook' >&2; exit 1
fi
test -s hook-input
test -z "$(git --git-dir="$fixture/remote.git" for-each-ref)"
printf '#!/bin/sh\ncat > hook-input\n' > .git/hooks/pre-push
"$binary" push origin refs/heads/main:refs/heads/main >/dev/null
test "$(git --git-dir="$fixture/remote.git" rev-parse refs/heads/main)" = "$(git rev-parse HEAD)"
printf 'installed-operational-fixture=passed (authoring, cleanup, hook refusal, push; no human acceptance)\n'
