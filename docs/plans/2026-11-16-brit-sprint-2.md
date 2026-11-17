---
title: Brit sprint 2 — seven fixes from daily use
status: complete
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
- [x] **2. `brit push` to a branch completes after a passing hook.** Elohim
  still lands on `dev` with Git. Observed with Brit 0.1.12: the hook passed
  and the push then ended without sending. First step: reproduce with 0.2.1 against
  a scratch branch whose hook takes several minutes; the cause is not known.
  *Done when:* either the reproduction completes on 0.2.1 and a journey test
  pins a push whose hook passes after a delay, or the failure is fixed with
  that test; and one real branch push with a slow hook completes.
  **Measured 2026-11-16:** unchanged Brit 0.2.1 from `0fdbfd4b` pushed
  `dd21cd08` to GitHub branch `qual/sprint2-slow-hook-20261116`, exit 0,
  after the protected hook passed readiness and Rust style and then waited
  180 seconds (184 seconds total hook time). The destination was absent
  during the hook and held the exact commit afterwards. This real probe
  created a branch pointing to already advertised objects;
  `delayed_passing_hook_delivers_branch_creation_and_update` separately
  proves new object delivery for branch creation and fast-forward updates,
  with a delayed passing hook, unchanged local HEAD and remote `fsck`.
  That test passes on the unchanged 0.2.1 binary; no push-engine repair was
  needed. Private receipts are in the existing `sprint2/item2` evidence directory.
  **Verified 2026-11-17:** the full journey suite passes 409 tests (71 existing
  opt-in tests remain ignored), followed by strict all-target Clippy. This run
  uses an isolated Cargo profile and checksum-verified 0.2.1 tools from green
  main CI 43973702902. The discovery fixture now owns a tracked disposable
  manifest instead of depending on this checkout's ancestors. Formatting,
  readiness contracts and the declared Rust-style checker pass.
- [x] **3. The feedback recorder says why it refused.** (S) Every command in
  a fresh worktree prints `brit: feedback capture gap; command result
  unchanged`, and a refusal reads the same as a timeout, because the
  recorder's stderr is discarded (`brit-cli/src/feedback.rs:101`). Usage
  feedback is how the next sprint gets chosen, and worktrees are where most
  work happens. *Done when:* the gap line carries the recorder's first stderr
  line or the word `timeout`, with a test beside
  `brit-cli/tests/feedback.rs:100`. If the cause of the worktree refusal turns
  out to be in `epr` and not in Brit, write that in one line here; do not fix
  `epr` from this repository.
  **Measured 2026-11-17:** the gap reports the recorder's first stderr line
  (at most 4096 bytes, with a truncation marker), exit status when silent,
  or `timeout`. Concurrent draining prevents a verbose refusal from filling
  the pipe; a descendant holding it open cannot extend the two-second wait.
  The command's stdout and exit code remain unchanged. The feedback tests,
  full CLI suite, strict all-target Clippy, formatting and declared style pass.
  Full-suite validation used `RUST_BACKTRACE=0 RUST_LIB_BACKTRACE=0`; its first
  run reproduced the existing diagnostic-test failure addressed by item 4.
  `brit-cli` is 0.2.2. The local EPR refusal was an inherited `EPR_BIN`
  selecting 0.3.2, which lacks `feedback`; the current 0.3.3 supports it.
- [x] **4. A refusal prints no backtrace.** (S) A hook refusal on push is
  followed by `Stack backtrace:` and a column of `<unknown>` frames, because
  errors are printed with the debug format (`brit-cli/src/main.rs:319`).
  *Done when:* the cause chain prints without a backtrace whatever
  `RUST_BACKTRACE` is set to. The existing test
  `git_errors_preserve_anyhow_main_diagnostics` (`main.rs:481`) pins the
  `Caused by:` lines; keep those lines, and if the test has to change, change
  only what it says about the backtrace.
  **Measured 2026-11-17:** explicit cause-chain formatting preserves the
  existing `Caused by:` assertion, numbered causes and multiline indentation
  without formatting the captured backtrace. An isolated regression runs
  that unchanged assertion with both backtrace variables set to `0`, `1`
  and `full`. All 46 CLI tests pass with backtrace capture enabled; strict
  all-target Clippy, formatting and declared Rust style pass. `brit-cli`
  is 0.2.3 for this separate source change.
- [x] **5. A leftover `AUTO_MERGE` file does not block a commit.** (S)
  `brit commit` refuses with "merge metadata exists without MERGE_HEAD" when
  only `AUTO_MERGE` remains in a worktree's git directory
  (`gitoxide-core/src/repository/authoring/merge.rs:64`). *Done when:* a lone
  `AUTO_MERGE` is not treated as merge state, with a case beside
  `tests/cli-journey/tests/commit_merge.rs:95`; the other three files keep
  refusing.
  **Measured 2026-11-17:** a lone `AUTO_MERGE` permits an ordinary
  single-parent commit in both normal and linked worktrees. Lone `MERGE_MSG`,
  `MERGE_MODE` and invalid `MERGE_HEAD` still refuse without changing the
  saved repository state. The regression first failed on the preceding CLI;
  all eight merge-commit journeys, the full journey and affected-crate suites,
  strict all-target Clippy, formatting, readiness contracts and declared Rust
  style now pass. The published dependency closure is `gitoxide-core` 0.63.9,
  `gitoxide` 0.60.9 and `brit-cli` 0.2.4.
- [x] **6. One command restores a path in both the index and the working
  tree.** (S) `brit restore --staged` and `--worktree` conflict, so putting a
  file back to `HEAD` takes two calls
  (`src/plumbing/options/mod.rs:207`). *Done when:* `brit restore --staged
  --worktree -- <path>` does both, with a journey test.
  **Measured 2026-11-17:** combined restoration takes the index lock before
  reading, prepares selected HEAD entries, restores working files from that
  same HEAD snapshot, then publishes the index. Existing per-file worktree
  atomicity is retained. Three added journeys cover Git parity, staged-added
  deletion, untouched neighbors, foreign locks, invalid selections and indexed
  attribute fallback when removing `.gitattributes`. All 17 restore journeys,
  187 core and 54 CLI/root tests pass. The whole journey run found two obsolete
  combined-mode refusal assertions; corrected full `daily_driver` and
  `native_authoring` targets pass all 25 tests, giving 412 passing ordinary
  journeys in aggregate (71 existing opt-in tests). Strict affected-crate
  Clippy, formatting, readiness and declared style pass. Versions are
  `gitoxide-core` 0.63.10, `gitoxide` 0.60.10 and `brit-cli` 0.2.5.
- [x] **7. `brit diff` works in a repository that marks files `-diff`.** (S)
  The whole diff is refused with "diff attributes and text conversion are not
  supported; use git diff" when any path carries a `diff` attribute, set or
  unset (`gitoxide-core/src/repository/diff.rs:379` and `:789`). In the Elohim
  repository that makes `brit diff` unusable. *Done when:* a path marked
  `-diff` or `binary` is reported as a binary change and the rest of the diff
  prints; a real text-conversion driver still refuses, naming the path.
  **Measured 2026-11-17:** unset `diff` (including the `binary` macro)
  forces binary rendering while neighboring text changes still print. The
  mixed-attribute journey matches Git's working/staged patches and statistics,
  checks tree statistics and unchanged repository state, and verifies an
  actual named text-conversion driver still refuses atomically with its path.
  All 17 fault-enabled diff-stat tests, 187 core and 54 CLI/root tests pass.
  The ordinary whole journey run plus the two corrected item-6 target reruns
  gives 413 passing tests (71 existing opt-in tests); strict affected Clippy,
  formatting, readiness and declared style pass. Versions are `gitoxide-core`
  0.63.11, `gitoxide` 0.60.11 and `brit-cli` 0.2.6.

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

When the count reaches 7 of 7 green on main, cut one release from `main`:
`brit-v0.2.6`, matching the CLI version after the required per-source-commit
bumps. No reserve item was taken. The tag cannot be created by
Brit and needs the steward to grant one command; ask once, at the end, with
the exact command. Then write three lines here: what shipped, what was
skipped and why, and the three things that hurt most while doing it. Those
three are the start of the next list. Do not start on them.

CI interpretation for this sprint: `.github/workflows/ci.yml` declares
`Tests pass` as its aggregate merge gate and checks that declaration in
`check-blocking`. Its existing advisory job is explicitly non-blocking.
Run 44116534675 reports the newly observed chrono maintenance and libz-sys
security advisories recorded in the backlog; no advisory exception or workflow
change is part of these seven fixes. Required jobs must still pass on each
exact landing commit, and the advisory findings remain visible at release.

**Delivered:** 7 of 7 on main at `1db7a955197b10bc7672c9aa544768a329bb9008`; all required final-source jobs and `Tests pass` succeeded in [CI 44117920990](https://github.com/ethosengine/brit/actions/runs/44117920990), the protected push passed, and the steward authorized `brit-v0.2.6`; binary publication is measured separately by [release 44145119161](https://github.com/ethosengine/brit/actions/runs/44145119161).
**Skipped:** none; no reserve item was taken, and the existing nonblocking dependency advisories remain in the backlog without exceptions or gate changes.
**Three pains:** hosted runner capacity cancelled jobs before execution; a stale inherited `EPR_BIN` selected a recorder without `feedback`; a shared Cargo profile reused another worktree's journey harness, requiring isolated profiles and frozen matching tools. These are backlog findings, not new work in this sprint.
