---
title: Brit sprint 4 — seven fixes from releasing 0.3.2
status: active
serves: brit-native-daily-driver
---
# Brit sprint 4: seven fixes from releasing 0.3.2

This is the whole sprint. It has seven items and it ends when they are done or
skipped. Progress is always "x of 7". The
[backlog](2026-11-15-bounded-memory-backlog.md) is the pool these came from; do
not work from the pool, and do not add to this list. Anything new you find goes
into the pool as one line.

Sprint 2 set the shape; sprint 3 was the trust lanes released in 0.3.0.
Releasing 0.3.2 on 2026-11-22 needed Git where Brit should have served: the tag
push, a branch config write, and rebase recovery after deliberate kills. These
seven items are those pains, ordered by how much wall-clock time each returns
to the people landing and releasing work.

## How to work

1. Take the items in order. Items 1, 2 and 6 touch disjoint files and may run
   in parallel, each in its own worktree and branch. Items 3, 4 and 5 all edit
   `gitoxide-core/src/repository/replay/journal.rs` and run in sequence.
2. For each: write the failing test first, make it pass, run that crate's whole
   test set (`cargo test -p <crate>` with no name filter), `cargo clippy -p
   <crate> --all-targets -- -D warnings`, and the format commands under
   "Dev-loop" in this repository's root `CLAUDE.md`. Tests that run the built
   binary live in `brit-cli/tests/`; workflow ("journey") tests live in
   `tests/cli-journey/tests/`. Keep each worktree's Cargo target separate;
   sharing one runs another item's journey harness.
3. Push the branch to `run-ci/sprint4-<item>`. When `ci.yml`'s `Tests pass` is
   green there, fast-forward `main`.
4. **Any commit that changes a published crate's source must bump that crate's
   version in the same commit**, along with every dependent in the publish set
   whose version the registry already holds. Rehearse with
   `scripts/ci/cargo-publish-brit.sh --check` before landing; main's publish
   job refuses an immutable version collision. Docs-only commits need no bump.
5. Tick the item below in the same commit and append one line to
   `.epr-meta/brit-native-daily-driver.habit.md` in the shape of its last line:
   `DELTA <date> (RED preserved): <what landed, with the test or run that shows
   it>`.

**Stop rule.** If an item is not green on `main` after four hours of active
work on it (waiting on CI does not count), stop. Write what blocked it under
the item, with file and line, mark it `skipped`, and take the next reserve
item. A skipped item is a finished item for counting. Do not widen an item to
make it pass.

**Out of scope:** cherry-pick (a new replay mode), `pull` and `rebase` on a
shallow repository, `log -- <path>`, the bounded-memory matrix rows, and the
`epr bin` items.

## The seven items

Sizes: S is one function with a test; M is a new mode built on code that
exists.

- [ ] **1. The pre-push hook lets a release tag through.** (M) The brit
  repository's own hook refuses any non-branch ref
  (`.githooks/pre-push:19-20`), so brit-v0.3.2 was pushed with
  `git push --no-verify`. The push engine already sends tags
  (`gitoxide-core/src/repository/push.rs:644`). Allow `refs/tags/X` →
  `refs/tags/X` only as a creation, never a move; peel it to a commit and
  require that commit to be reachable from the already reach-checked
  `origin/main`; skip `check_source`'s HEAD equality for tags. Branch pushes
  keep today's checks. *Done when:* `tests/cli-journey/tests/pre_push.rs`
  passes a tag at a reached commit and refuses one at an unreached commit and
  one that moves an existing tag; and the sprint's release tag is pushed by
  `brit push`.
- [ ] **2. `brit config set <key> <value>` and `brit config unset <key>`.**
  (S) `brit config` only reads (`src/plumbing/options/mod.rs:1241`);
  `branch.<b>.britReachBase` had to be set with `git config --local`. Reuse
  `edit_local_config` (`gitoxide-core/src/repository/branch.rs:289`), which
  locks, edits and commits the local config. *Done when:* a journey test shows
  `brit config` reporting the value after `set` and not after `unset`, and a
  held `config.lock` refuses with the config unchanged.
- [ ] **3. `brit rebase --status` advises only what `ready-final` allows.**
  (S) The catch-all at `gitoxide-core/src/repository/replay/journal.rs:4602`
  offers `--abort`, which a final decision cannot take (`:4530`). *Done when:*
  a status test in the `ready-final` state names `--continue` and not
  `--abort`.
- [ ] **4. A kill at `replay-transition-directory-created` heals without
  `--retire`.** (S) The staging directory is created before its `owner` file
  (`journal.rs:3754-3761`); a kill between them leaves an owner-less directory
  that the next run refuses as foreign (`:3746`). Treat an empty, owner-less,
  0700 directory owned by this uid as torn and remove it; anything else still
  refuses. *Done when:* a kill fixture at that checkpoint resumes, and a
  planted non-empty or foreign-owned directory still refuses.
- [ ] **5. A changed author or committer identity does not refuse recovery.**
  (M) The policy digest hashes the whole config snapshot
  (`replay/hooks.rs:10-47`, compared at `journal.rs:704` and `:4340`), which
  includes the identity gix derives from `GIT_AUTHOR_*` and `GIT_COMMITTER_*`.
  Leave the identity keys out of the digest. *Done when:* a resume under a
  different `GIT_COMMITTER_NAME` continues, and a changed hook or other config
  still refuses.
- [ ] **6. `brit submodule update --init <path>`.** (M) `update` and `init`
  are bridged to Git (`brit-cli/src/bridge.rs:25`); options at
  `src/plumbing/options/mod.rs:1994`. *Done when:* in a fresh worktree the
  named submodule directory holds the commit the parent records, and an
  unknown path refuses.
- [x] **7. `brit diff` refused every diff with a moved submodule pin.** (S)
  Reproduced with 0.3.0: `working_output` refused the whole worktree diff
  ("submodule patches are not supported") whenever a gitlink was in the
  index, so in the Elohim checkout it was unusable; blank output for
  staged-only changes matches Git. **Measured 2026-11-22:** gitlinks against
  the working files now print Git's short form (the submodule's checked-out
  commit, `-dirty` for changed tracked files or index, untracked ignored), with
  patch, `--stat`, `--numstat`, `--name-only`, `--name-status` and
  `--exit-code` byte-identical to Git for moved, dirty, staged, uninitialized,
  missing, file-replaced and index-removed submodules, and for gitlink
  additions, removals and type changes between revisions
  (`tests/cli-journey/tests/diff_revisions.rs`, three new tests).
  `diff.submodule=log|diff` (patches), `diff.ignoreSubmodules` and
  `submodule.<name>.ignore` set to `dirty`/`all` refuse by name, and
  `--submodule=` is not an option. Gap: `brit diff -- <submodule path>`
  still refuses as a directory (shared `literal_paths`).

## If an item is skipped

Take one replacement from this reserve, in order, and keep the count at seven:

- `brit diff <revision>` compares a revision with the working tree. Test: for
  one changed file its output matches `brit diff tree` for the same states.
- Stage a gitlink at a named commit without a checked-out submodule (the pin
  move needed `git update-index --cacheinfo`). Test: the index entry holds the
  commit, mode `160000`.
- A kill during child preparation (`recover_preparation`, `journal.rs:2808`)
  resumes. Test: a kill fixture at that point.

## Ending the sprint

When the count reaches 7 of 7 green on main, cut one release from `main`
(`brit-v0.3.3`, matching the CLI version), create the tag with `brit tag -a`,
and push it with `brit push` through the hook from item 1. Tell the
integrator the release commit; the monorepo pin is theirs to move. Then write
three lines here: what shipped, what was skipped and why, and the three things
that hurt most while doing it. Those three start the next list.
