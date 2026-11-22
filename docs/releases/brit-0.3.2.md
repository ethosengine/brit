# Brit 0.3.2

Brit 0.3.2 makes `brit rebase` usable on large repositories, makes brit 0.3.1's crates
publishable, and stops deferred CI legs from gating.

## What 0.3.2 contains

- **`brit rebase` scales with the change, not the history.** Selection, verification, the
  retained copy, the journal and the worktree witness now cover what the rebase can lose: the
  replayed range, its objects above a floor (merge base and upstream), and the touched paths. The
  retained-manifest format is v2 on every new copy and records that floor; `validate_release` stops
  there. Older (v1) copies and journals are still restored and cleaned; an older brit fails closed
  on a v2 copy (steward ruling, 2026-11-22). Before, the closure preflight refused every rebase in
  repositories over about 1 GiB of history; on a fresh monorepo clone a two-commit docs rebase now
  publishes and cleans up in 23.7 s. Unchanged gitlinks are index-only rather than refused; a
  gitlink the rebase moves is refused by path, before anything changes. A `--skip` killed while
  restoring the checkout no longer strands the rebase, and a pending post-abort cleanup names
  `--continue`. Design and residuals: `docs/plans/2026-11-22-rebase-scoped-retention.md`,
  `docs/rebase.md`.
- **`brit-graph` 0.2.4.** 0.3.1 changed `brit-graph` (`from_repo_globs_excluding`, and the
  fingerprint sources and tests around it) without moving it off 0.2.3, which the `elohim`
  registry already held. Main's publish job refused it as an immutable version collision
  (run 44961618503, `brit-graph 0.2.3`: changed `src/fingerprint.rs`, `src/repo_fingerprint.rs`,
  `tests/repo_fingerprint.rs`), so `brit-cli` 0.3.1 was never published. `brit-cli` 0.3.2
  requires `brit-graph` 0.2.4.
- **Deferred legs outside the gate.** `scripts/ci/inherited_ci_policy.py` writes the required
  planned jobs to `matrix` and the deferred ones to a new `deferred` output. `run` executes only
  `matrix` and stays in `Tests pass`; a new `run-deferred` job executes `deferred` with the same
  permissions, environment and steps (shared through YAML anchors) and is not in `Tests pass`.
  An empty `deferred` skips the job. `.epr-meta/readiness.json` declares `run-deferred` deferred,
  and `scripts/ci/test_qualify_readiness.py` checks the split.

## Recorded verification

- Rebase redesign: two independent adversarial reviews (DO NOT MERGE on an unchanged-gitlink
  stranding, fixed in `6cae2b78`; then MERGE WITH RESIDUALS), and a P1 verification of `485cfa2d`:
  `--skip` SIGKILL sweeps at 113 points each, with and without gitlinks, end in a working command
  with an exact restore or a completed skip; abort sweeps restore exactly; copy-soundness proofs over
  nine history shapes find no missing object. Implementer's runs on `96143a31`: faults journeys
  rebase 55, switch 28, pull 44, commit_merge 10, diff_stat 17; replay and authoring suites;
  `memory_replay` 8 passed, 1 ignored; clippy `-D warnings` and fmt.
- Publish rehearsal (`scripts/ci/cargo-publish-brit.sh --check`, from a standalone clone): the
  0.3.1 tree skips 65 crates as equivalent and stops at the `brit-graph 0.2.3` collision; this
  release's result is recorded in its commit message.
- `run-ci/deferred-legs-20261121` (run 44971088719) at the CI change alone: `Tests pass`
  success; `advisories@linux-x86_64` failed as a deferred leg without failing the gate.
- Local: `test_qualify_readiness.py` 12 tests and `test_inherited_ci_policy.py` 14 tests OK.
- The release head's own `Tests pass` run is the release evidence.

## Not qualified

- Rebase residuals, all also present in 0.3.1: a kill at `replay-transition-directory-created`
  leaves an aborted journal that `--retire` (named by `--status`) sets aside; in phase
  `ready-final` and one `retaining-skip` window `--status` advises a command that refuses while
  `--abort` restores; recovery refuses when `GIT_AUTHOR_*`/`GIT_COMMITTER_*` differ from the start
  without naming the variable. Shallow rebase is still refused.
- The wall-clock saving was not shown by the run above: its slowest leg was a required one
  (`tests@windows-aarch64`), so `Tests pass` had no deferred leg left to stop waiting for.
- `advisories@linux-x86_64` and `size@linux-x86_64` stay red, as on 0.3.0 and 0.3.1.
- The hosted `tests@windows-aarch64` runner lost communication after 93 minutes on main's 0.3.1
  run; a re-run of the failed jobs passed.
