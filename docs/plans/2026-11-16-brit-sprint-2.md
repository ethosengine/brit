---
title: Brit sprint 2 — seven fixes from daily use
status: active
serves: brit-native-daily-driver
---
# Brit sprint 2: seven fixes from daily use

This is the whole sprint. It has seven items and it ends when they are done or
skipped. Progress is always "x of 7". The 82-item
[backlog](2026-11-15-bounded-memory-backlog.md) is the pool these came from; do
not work from the pool, and do not add to this list. Anything new you find goes
into the pool as one line.

The previous sprint tried to close a coverage matrix that had no fixed size and
ran two days without closing a row. On 2026-11-15 the steward ruled that a Brit
release is what has been measured, and that the next work is chosen from what
hurt while using Brit. Brit 0.2.0 was released on that basis and `brit-cli` is
now 0.2.1. These seven items are what hurt most across two devices on
2026-11-15 and 16, ordered by how much wall-clock time each returns to the
people landing work on the Elohim `dev` branch.

## How to work

1. Take the items in order, one at a time.
2. For each: write the failing test first, make it pass, run that crate's whole
   test set (`cargo test -p <crate>` with no name filter), `cargo clippy -p
   <crate> --all-targets -- -D warnings`, and the format commands under
   "Dev-loop" in this repository's root `CLAUDE.md`. Tests that run the built
   binary live in `brit-cli/tests/`; workflow ("journey") tests live in
   `tests/cli-journey/tests/`.
3. Push the branch to `run-ci/<item-name>`. When `ci.yml` is green there,
   fast-forward `main`.
4. **Any commit that changes a published crate's source must bump that crate's
   version in the same commit** (`brit-cli/Cargo.toml` and `Cargo.lock`). A
   published version cannot change its contents: main CI run 43756399241
   passed its tests and then refused to publish with "immutable version
   collision for brit-cli 0.2.0". Docs-only commits need no bump.
5. Tick the item below in the same commit and append one line to
   `.epr-meta/brit-native-daily-driver.habit.md`, the evidence ledger for this
   work, in the shape of its last line: `DELTA <date> (RED preserved): <what
   landed, with the test or run that shows it>`.

**Stop rule.** If an item is not green on `main` after four hours of active
work on it (waiting on CI does not count), stop work on it. Write what blocked it under the item, with file and line, mark
it `skipped`, and take the next one. A skipped item is a finished item for
counting. Do not widen an item to make it pass.

**Out of scope for this sprint**, however tempting: the bounded-memory matrix
rows and scratch measurement in the pool; cross-platform qualification; and
these, which were sized as new engine work: `pull` and `rebase` on a shallow
repository (one shared guard, `validate_local_history`), cherry-pick, text
conversion in `diff`, and `log -- <path>`.

## The seven items

Sizes: S is one function with a test; M is a new mode built on code that
exists. Items 1 and 2 were not sized from source, so each begins with a
reproduction and is capped by the stop rule like the rest.

- [x] **1. Build notes written at epoch 2 decode.** Reported 2026-11-14 from
  the Elohim gate: notes shaped `{body, epoch: 2, key}` under
  `refs/notes/brit/build/*` could not be decoded by the pinned
  `brit-build-ref` crate (the Elohim repository pins Brit by submodule), and
  each note it could not read stopped one device from reusing another device's
  passing gate for five projects. Where reuse works it was measured that day
  at 31 seconds against 1177 seconds. The reader's handling of a note's
  `epoch` starts at `brit-build-ref/src/build_cmd.rs:132`; a note it cannot
  verify is reported on stderr as `unverifiable:` or held. First step: in the
  Elohim checkout, list the notes under `refs/notes/brit/build/*`, find one
  whose JSON carries `"epoch": 2`, and run the 0.2.1 reader over it. Record
  whether it is read, held or refused. *Done when:* a test in
  `brit-build-ref` reads a fixture copied from that note without an
  `unverifiable` or held line, and the same real note reads cleanly. If 0.2.1
  already reads it, the fixture test alone finishes the item; do not go
  looking for other epochs.
  **Measured 2026-11-16:** the reader built from main `0fdbfd4b` (Brit 0.2.1)
  reads Elohim's `gate:orchestrator` note on
  `293da567277d5b8153c0f5eea3bcad1148dc25b9` through both `build get` and
  `build find`, exit 0 and empty stderr. The exact note blob
  `1bdf5cbe7498f185b9cfad86b79646271dbd30ff` is retained in
  `brit-build-ref/tests/fixtures/epoch2-orchestrator-note.json` and exercised by
  `real_epoch_two_gate_note_reads_without_diagnostics`. No decoder change was
  needed. The full crate's 32 tests, strict all-target Clippy, both prescribed
  formatting checks and the declared Rust-style checker pass; `brit-build-ref`
  is bumped to 0.1.8 for its changed package contents.
- [ ] **2. `brit push` to a branch completes after a passing hook.** Elohim
  still lands on `dev` with Git. Observed with Brit 0.1.12: the hook passed
  and the push then ended without sending. First step: reproduce with 0.2.1 against
  a scratch branch whose hook takes several minutes; the cause is not known.
  *Done when:* either the reproduction completes on 0.2.1 and a journey test
  pins a push whose hook passes after a delay, or the failure is fixed with
  that test; and one real branch push with a slow hook completes.
- [ ] **3. The feedback recorder says why it refused.** (S) Every command in
  a fresh worktree prints `brit: feedback capture gap; command result
  unchanged`, and a refusal reads the same as a timeout, because the
  recorder's stderr is discarded (`brit-cli/src/feedback.rs:101`). Usage
  feedback is how the next sprint gets chosen, and worktrees are where most
  work happens. *Done when:* the gap line carries the recorder's first stderr
  line or the word `timeout`, with a test beside
  `brit-cli/tests/feedback.rs:100`. If the cause of the worktree refusal turns
  out to be in `epr` and not in Brit, write that in one line here; do not fix
  `epr` from this repository.
- [ ] **4. A refusal prints no backtrace.** (S) A hook refusal on push is
  followed by `Stack backtrace:` and a column of `<unknown>` frames, because
  errors are printed with the debug format (`brit-cli/src/main.rs:319`).
  *Done when:* the cause chain prints without a backtrace whatever
  `RUST_BACKTRACE` is set to. The existing test
  `git_errors_preserve_anyhow_main_diagnostics` (`main.rs:481`) pins the
  `Caused by:` lines; keep those lines, and if the test has to change, change
  only what it says about the backtrace.
- [ ] **5. A leftover `AUTO_MERGE` file does not block a commit.** (S)
  `brit commit` refuses with "merge metadata exists without MERGE_HEAD" when
  only `AUTO_MERGE` remains in a worktree's git directory
  (`gitoxide-core/src/repository/authoring/merge.rs:64`). *Done when:* a lone
  `AUTO_MERGE` is not treated as merge state, with a case beside
  `tests/cli-journey/tests/commit_merge.rs:95`; the other three files keep
  refusing.
- [ ] **6. One command restores a path in both the index and the working
  tree.** (S) `brit restore --staged` and `--worktree` conflict, so putting a
  file back to `HEAD` takes two calls
  (`src/plumbing/options/mod.rs:207`). *Done when:* `brit restore --staged
  --worktree -- <path>` does both, with a journey test.
- [ ] **7. `brit diff` works in a repository that marks files `-diff`.** (S)
  The whole diff is refused with "diff attributes and text conversion are not
  supported; use git diff" when any path carries a `diff` attribute, set or
  unset (`gitoxide-core/src/repository/diff.rs:379` and `:789`). In the Elohim
  repository that makes `brit diff` unusable. *Done when:* a path marked
  `-diff` or `binary` is reported as a binary change and the rest of the diff
  prints; a real text-conversion driver still refuses, naming the path.

## If an item is skipped

Take one replacement from this reserve, in order, and keep the count at seven.
Each is sized M and is done when the named journey test passes:

- `brit submodule update --init <path>` checks a submodule out at the commit
  the parent records (`src/plumbing/options/mod.rs:1474`). Test: a fresh
  worktree's submodule directory holds that commit afterwards.
- `brit config set <key> <value>` and `unset <key>` write the repository's
  local config (`:754`). Test: `brit config` shows the value, then does not.
- `brit diff <revision>` compares a revision with the working tree (`:693`).
  Test: its output for one changed file matches `brit diff tree` for the same
  two states.

`brit remote refs` already lists a remote's references
(`src/plumbing/options/mod.rs:988`), so nothing is needed there.

## Ending the sprint

When the count reaches 7 of 7, cut one release from `main`: `brit-v0.2.2`, or
`brit-v0.3.0` if a reserve item added a command. The tag cannot be created by
Brit and needs the steward to grant one command; ask once, at the end, with
the exact command. Then write three lines here: what shipped, what was
skipped and why, and the three things that hurt most while doing it. Those
three are the start of the next list. Do not start on them.
