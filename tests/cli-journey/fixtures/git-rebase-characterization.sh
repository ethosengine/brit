#!/usr/bin/env bash
# Stock Git observations only. Invoked by cli-journey with isolated Git environment.
# Every case runs in its own disposable repository; no sleeps or timing races.
set -euo pipefail
trap 'echo "failed line $LINENO: $BASH_COMMAND" >&2' ERR
export GIT_AUTHOR_NAME='Replay Author' GIT_AUTHOR_EMAIL=author@example.invalid
export GIT_COMMITTER_NAME='Replay Committer' GIT_COMMITTER_EMAIL=committer@example.invalid
export GIT_AUTHOR_DATE='2001-01-01T00:00:00 +0000' GIT_COMMITTER_DATE='2002-01-01T00:00:00 +0000'
export GIT_EDITOR=: GIT_SEQUENCE_EDITOR=:
git config core.filemode true
git config core.logAllRefUpdates true
git config advice.skippedCherryPicks false
fail() { if "$@"; then echo "Unexpected success: $*" >&2; exit 1; fi; }
commit() { git add -A; git commit -qm "$1"; }
head() { git rev-parse HEAD; }
subject() { git log -1 --format=%s; }
count() { git rev-list --count main..HEAD; }
# Fixture scripts and sed backups live in private .git, outside commits and collision surfaces.
# An attached backup suffix works with both BSD and GNU sed.
editor() { printf '#!/bin/sh\n%s\n' "$2" > ".git/$1"; chmod +x ".git/$1"; }
linear() {
    echo base > base; echo original > neighbor; commit base
    git switch -qc topic
    echo one > one; echo companion > companion; commit one
    echo two > two; commit two
    original=$(head)
    git switch -q main; echo upstream > upstream; commit upstream
    upstream=$(head); git switch -q topic
}
conflict() {
    echo base > f; commit base
    git switch -qc topic; echo theirs > f; commit topic
    original=$(head)
    git switch -q main; echo ours > f; commit upstream
    upstream=$(head); git switch -q topic
    fail git rebase main
    test "$(git show :1:f)" = base
    test "$(git show :2:f)" = ours
    test "$(git show :3:f)" = theirs
    test "$(git rev-parse topic)" = "$original"
    test "$(head)" = "$upstream"
    test -d .git/rebase-merge
}
case "$1" in
attached|detached)
    linear
    if test "$1" = detached; then git switch -q --detach; fi
    git rebase main
    test "$(git log --format=%s --reverse main..HEAD)" = $'one\ntwo'
    test "$(git show -s --format=%an HEAD)" = 'Replay Author'
    test "$(git show -s --format=%at HEAD)" = 978307200
    test "$(git show -s --format=%ct HEAD)" = 1009843200
    cat .git/logs/HEAD
    grep -q 'rebase' .git/logs/HEAD
    if test "$1" = attached; then test "$(git symbolic-ref HEAD)" = refs/heads/topic
    else fail git symbolic-ref -q HEAD; test "$(git rev-parse topic)" = "$original"; fi
    ;;
empty_and_equivalent)
    linear
    git commit -q --allow-empty -m intentionally-empty
    git switch -q main; git cherry-pick topic~2
    git switch -q topic; git rebase main
    test "$(git log --format=%s --reverse main..HEAD)" = $'two\nintentionally-empty'
    ;;
newly_empty)
    linear
    editor todo 'sed -i.bak "2s/^pick /fixup /" "$1"'
    # Reapplying an exact patch retained explicitly becomes empty and is dropped.
    git switch -q main; git cherry-pick topic~1
    git switch -q topic; git rebase --reapply-cherry-picks main
    test "$(count)" = 1; test "$(subject)" = two
    ;;
merge_history)
    linear; git switch -qc side HEAD~1; echo side > side; commit side
    git switch -q topic; git merge -q --no-ff side -m merged
    git rebase main
    test "$(git rev-list --count --merges main..HEAD)" = 0
    test "$(count)" = 3 # Brit intentionally refuses instead of flattening.
    ;;
reword|squash|fixup)
    linear
    if test "$1" = reword; then action='1s/^pick /reword /'; else action="2s/^pick /$1 /"; fi
    editor todo "sed -i.bak '$action' \"\$1\""
    editor message 'printf "edited message\n" > "$1"'
    GIT_SEQUENCE_EDITOR="$PWD/.git/todo" GIT_EDITOR="$PWD/.git/message" git rebase -i main
    if test "$1" = reword; then test "$(git log -1 --format=%s HEAD~1)" = 'edited message'; test "$(count)" = 2
    elif test "$1" = squash; then test "$(subject)" = 'edited message'; test "$(count)" = 1
    else test "$(subject)" = one; test "$(count)" = 1; fi
    test -f one; test -f two
    ;;
reorder_drop)
    linear
    editor todo '{ sed -n "2p" "$1"; sed -n "1p" "$1"; } > "$1.new"; mv "$1.new" "$1"'
    GIT_SEQUENCE_EDITOR="$PWD/.git/todo" git rebase -i main
    test "$(git log --reverse --format=%s main..HEAD)" = $'two\none'
    editor todo 'sed -i.bak "1s/^pick /drop /" "$1"'
    GIT_SEQUENCE_EDITOR="$PWD/.git/todo" git rebase -i main
    test "$(count)" = 1; test ! -f two
    ;;
edit_amend|edit_split)
    linear
    editor todo 'sed -i.bak "1s/^pick /edit /" "$1"'
    GIT_SEQUENCE_EDITOR="$PWD/.git/todo" git rebase -i main
    test "$(git rev-parse topic)" = "$original"
    if test "$1" = edit_amend; then echo amended >> one; git add one; git commit -q --amend -m amended
    else git reset -q HEAD~1; git add one; git commit -qm part-one; git add companion; git commit -qm part-two; fi
    git rebase --continue
    if test "$1" = edit_amend; then test "$(git log -1 --format=%s HEAD~1)" = amended
    else test "$(count)" = 3; fi
    ;;
break_stop)
    linear; editor todo '{ printf "break\n"; cat "$1"; } > "$1.new"; mv "$1.new" "$1"'
    GIT_SEQUENCE_EDITOR="$PWD/.git/todo" git rebase -i main
    test "$(head)" = "$upstream"; test -d .git/rebase-merge
    git rebase --show-current-patch > .git/patch 2>/dev/null || true
    git rebase --continue; test "$(count)" = 2
    ;;
invalid_todo)
    linear; editor todo 'printf "nonsense bad\n" > "$1"'
    fail env GIT_SEQUENCE_EDITOR="$PWD/.git/todo" git rebase -i main
    test -d .git/rebase-merge
    fail git rebase --continue
    editor repair "printf 'pick $original\\n' > \"\$1\""
    GIT_SEQUENCE_EDITOR="$PWD/.git/repair" git rebase --edit-todo
    git rebase --continue
    test "$(subject)" = two
    ;;
failed_editors)
    linear; fail env GIT_SEQUENCE_EDITOR=false git rebase -i main
    test "$(head)" = "$original"; test ! -d .git/rebase-merge
    editor todo 'sed -i.bak "1s/^pick /reword /" "$1"'
    fail env GIT_SEQUENCE_EDITOR="$PWD/.git/todo" GIT_EDITOR=false git rebase -i main
    test -d .git/rebase-merge; git rebase --abort
    test "$(head)" = "$original"
    ;;
dirty_neighbors)
    linear; echo staged > neighbor; git add neighbor; echo unstaged >> neighbor
    git diff --binary > .git/before-work; git diff --cached --binary > .git/before-index
    fail git rebase main
    git diff --binary > .git/after-work; git diff --cached --binary > .git/after-index
    cmp .git/before-work .git/after-work; cmp .git/before-index .git/after-index
    git reset -q --hard HEAD; git rebase main; test "$(count)" = 2
    ;;
text)
    conflict; git rebase --show-current-patch | grep -q theirs
    echo resolved > f; git add f; git rebase --continue
    test "$(git show HEAD:f)" = resolved; test ! -d .git/rebase-merge
    ;;
add_add|modify_delete|rename|binary|symlink|mode|mode_conflict|directory)
    case "$1" in
    binary) printf 'base\0bytes' > f;;
    symlink) ln -s base f;;
    *) echo base > f;;
    esac
    commit base; git switch -qc topic
    case "$1" in
    add_add) echo theirs > added;;
    modify_delete) echo theirs > f;;
    rename) git mv f theirs;;
    binary) printf 'theirs\0bytes' > f;;
    symlink) rm f; ln -s theirs f;;
    mode) chmod +x f;;
    mode_conflict) rm f; ln -s theirs f;;
    directory) mkdir dir; echo theirs > dir/child;;
    esac
    commit topic; original=$(head); git switch -q main
    case "$1" in
    add_add) echo ours > added;;
    modify_delete) git rm f;;
    rename) git mv f ours;;
    binary) printf 'ours\0bytes' > f;;
    symlink) rm f; ln -s ours f;;
    mode) echo upstream > f;;
    mode_conflict) chmod +x f; echo ours > f;;
    directory) echo ours > dir;;
    esac
    commit upstream; git switch -q topic
    if test "$1" = mode; then git rebase main; test -x f; test "$(cat f)" = upstream
    else fail git rebase main; test -n "$(git ls-files -u)"
        git ls-files -u > .git/stages
        case "$1" in
        add_add) test "$(awk '{print $3}' .git/stages)" = $'2\n3';;
        modify_delete) test "$(awk '{print $3}' .git/stages)" = $'1\n3';;
        binary|symlink) test "$(awk '{print $3}' .git/stages)" = $'1\n2\n3';;
        rename) grep -q $'1\tf' .git/stages; grep -q $'2\tours' .git/stages; grep -q $'3\ttheirs' .git/stages;;
        directory) grep -q $'2\tdir' .git/stages; test -f dir/child;;
        mode_conflict) grep -q '^120000 .* 3' .git/stages; grep -q '^100755 .* 2' .git/stages;;
        esac
        git rebase --abort; test "$(head)" = "$original"; fi
    ;;
skip_abort)
    conflict; echo resolution > f; git add f; git rebase --skip
    test "$(head)" = "$upstream"; test "$(cat f)" = ours
    git reset -q --hard "$original"; fail git rebase main
    echo discarded > f; git add f; git rebase --abort
    test "$(head)" = "$original"; test "$(cat f)" = theirs
    # Stock Git discards this resolution. Brit's private retained copy is a deliberate extension.
    ;;
hooks|hook_failures|hooks_reword|hooks_squash|hooks_conflict)
    linear
    for hook in pre-rebase pre-commit prepare-commit-msg commit-msg post-commit post-rewrite; do
        printf '#!/bin/sh\nprintf "%%s:%%s\\n" "%s" "$*" >> "%s/.git/hooks.log"\n' "$hook" "$PWD" > ".git/hooks/$hook"
        if test "$hook" = post-rewrite; then printf 'cat >> "%s/.git/mapping-$1"\n' "$PWD" >> ".git/hooks/$hook"; fi
        chmod +x ".git/hooks/$hook"
    done
    if test "$1" = hook_failures; then
        echo 'exit 1' >> .git/hooks/pre-rebase; fail git rebase main
        test "$(head)" = "$original"; test ! -d .git/rebase-merge
        sed -i.bak '$d' .git/hooks/pre-rebase; echo 'exit 1' >> .git/hooks/post-rewrite
    fi
    case "$1" in
    hooks_reword|hooks_squash)
        if test "$1" = hooks_reword; then change='1s/^pick /reword /'; else change='2s/^pick /squash /'; fi
        editor todo "sed -i.bak '$change' \"\$1\""
        GIT_SEQUENCE_EDITOR="$PWD/.git/todo" git rebase -i main;;
    hooks_conflict)
        git switch -q main; echo conflict > one; commit conflict
        git switch -q topic; : > .git/hooks.log
        fail git rebase main; echo resolution > one; git add one; git rebase --continue;;
    *) git rebase main;;
    esac
    grep -q '^pre-rebase:main$'  .git/hooks.log
    grep -q '^post-rewrite:rebase$' .git/hooks.log
    test "$(wc -l < .git/mapping-rebase)" = 2
    test "$(awk 'NF != 2 {n++} END {print n+0}' .git/mapping-rebase)" = 0
    test "$(git rev-parse topic)" = "$(head)"
    if test "$1" = hooks_reword; then
        test "$(grep -c '^pre-commit:$' .git/hooks.log)" = 1
        grep -q '^prepare-commit-msg:.git/COMMIT_EDITMSG commit HEAD$' .git/hooks.log
        test "$(grep -c '^commit-msg:.git/COMMIT_EDITMSG$' .git/hooks.log)" = 1
        test "$(grep -c '^post-commit:$' .git/hooks.log)" = 3
        test "$(wc -l < .git/mapping-amend)" = 1
    elif test "$1" = hooks_squash; then
        test "$(grep -c '^commit-msg:' .git/hooks.log || true)" = 0
        test "$(wc -l < .git/mapping-amend)" = 1
        test "$(awk '{print $2}' .git/mapping-rebase | sort -u | wc -l)" = 1
    fi
    if test "$1" != hooks_reword; then
    test "$(grep -c '^pre-commit:' .git/hooks.log || true)" = 0
    test "$(grep -c '^prepare-commit-msg:.git/COMMIT_EDITMSG message$' .git/hooks.log)" = 2
    test "$(grep -c '^post-commit:$' .git/hooks.log)" = 2
    test "$(grep -c '^commit-msg:' .git/hooks.log || true)" = 0
    fi
    cat .git/hooks.log
    # post-rewrite failure is ignored after publication; never call this a rollback.
    ;;
interruption_editor|interruption_hook)
    linear
    if test "$1" = interruption_editor; then
        editor interrupt 'printf reached > .git/interrupt-marker; kill -KILL "$PPID"; exit 1'
        fail env GIT_SEQUENCE_EDITOR="$PWD/.git/interrupt" git rebase -i main
    else
        editor todo 'sed -i.bak "1s/^pick /reword /" "$1"'
        printf '#!/bin/sh\nprintf reached > .git/interrupt-marker\nkill -KILL "$PPID"\nexit 1\n' > .git/hooks/prepare-commit-msg
        chmod +x .git/hooks/prepare-commit-msg
        fail env GIT_SEQUENCE_EDITOR="$PWD/.git/todo" git rebase -i main
    fi
    test -f .git/interrupt-marker; test -d .git/rebase-merge
    test "$(git rev-parse topic)" = "$original"
    # An editor/hook entry marker is not an exactly-once outcome receipt.
    ;;
external_mutations)
    conflict
    echo external > f; git add f
    echo neighboring > extra; git add extra
    git rebase --continue
    test "$(git show HEAD:extra)" = neighboring # Brit must restrict this consumption.
    git reset -q --hard "$original"; fail git rebase main
    git update-ref refs/heads/topic "$upstream" "$original"
    echo resolved > f; git add f
    fail git rebase --continue # final branch CAS detects unexpected ref movement.
    test "$(git rev-parse topic)" = "$upstream"
    ;;
linked_worktree)
    linear; git switch -q main
    git worktree add -q .git/linked topic
    (cd .git/linked; git rebase main; test "$(git symbolic-ref HEAD)" = refs/heads/topic)
    test "$(git symbolic-ref HEAD)" = refs/heads/main
    test "$(head)" = "$upstream"
    ;;
untracked_collision)
    linear; echo private > two-private; echo two-private > .git/info/exclude
    echo untracked > loose; git rebase main
    test "$(cat two-private)" = private; test "$(cat loose)" = untracked
    git reset -q --hard "$original"; git switch -q main
    echo upstream > collision; commit collision; git switch -q topic
    echo local > collision; fail git rebase main; test "$(cat collision)" = local
    ;;
editor_precedence)
    linear; unset GIT_SEQUENCE_EDITOR
    editor seq 'printf sequence > .git/editor-choice'
    editor msg 'printf message > .git/editor-choice'
    git config sequence.editor "$PWD/.git/seq"
    GIT_EDITOR="$PWD/.git/msg" git rebase -i main
    test "$(cat .git/editor-choice)" = sequence
    git reset -q --hard "$original"
    GIT_SEQUENCE_EDITOR="$PWD/.git/msg" git rebase -i main
    test "$(cat .git/editor-choice)" = message
    git reset -q --hard "$original"; git config --unset sequence.editor
    GIT_EDITOR="$PWD/.git/msg" git rebase -i main
    test "$(cat .git/editor-choice)" = message
    git reset -q --hard "$original"; git config core.editor "$PWD/.git/seq"
    env -u GIT_EDITOR git rebase -i main
    test "$(cat .git/editor-choice)" = sequence
    ;;
external_index_and_worktree)
    conflict; cp .git/index .git/index-snapshot
    printf invalid > .git/index; fail git rebase --continue
    cmp .git/index <(printf invalid)
    cp .git/index-snapshot .git/index
    echo unstaged > f; fail git rebase --continue; test "$(cat f)" = unstaged
    git add f; git rebase --continue; test "$(git show HEAD:f)" = unstaged
    ;;
ignored_collision)
    linear; git switch -q main; echo upstream > collision; commit collision
    git switch -q topic; echo collision > .git/info/exclude; echo private > collision
    git rebase main
    test "$(cat collision)" = upstream # Intentional Brit difference: refuse even ignored overwrite.
    ;;
failed_prepare_hook)
    linear
    printf '#!/bin/sh\nexit 1\n' > .git/hooks/prepare-commit-msg; chmod +x .git/hooks/prepare-commit-msg
    fail git rebase main
    test -d .git/rebase-merge; test "$(git rev-parse topic)" = "$original"
    git rebase --abort; test "$(head)" = "$original"
    ;;
noop_fastforward)
    echo base > base; commit base; base=$(head); git branch topic
    git rebase main; test "$(head)" = "$base"
    echo upstream > upstream; commit upstream; upstream=$(head)
    git switch -q topic; git rebase main; test "$(head)" = "$upstream"
    ;;
*) echo "unknown case $1" >&2; exit 2;;
esac
printf 'OBSERVED stock Git: %s\n' "$1"
