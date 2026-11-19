# Changelog — brit-cli

All notable changes to the `brit` executable are recorded here; changes to `brit-epr` and the
crates released with it are in [`brit-epr/CHANGELOG.md`](../brit-epr/CHANGELOG.md), and
release narratives in [`docs/releases/`](../docs/releases/). The root `CHANGELOG.md` is
upstream gitoxide's generated file and is not edited here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- `brit log [REVISION…] [--oneline] [-- PATH…]` lists the commits reachable from the given
  revisions instead of `HEAD`, including exclusions and ranges (`^A`, `A..B`, `A...B`, `A^@`,
  `A^!`), ordered like `git rev-list --date-order`, and limits them to commits that change
  literal paths, following Git's limiter and default history simplification; a trailing slash
  matches only directories. `--oneline` prints the `core.abbrev` hash and title.
- `brit diff tree --name-only` lists changed paths like `git diff --name-only --no-renames`;
  `diff tree` also takes `--merge-base OLD NEW` and a lone `A..B` or `A...B`.
- `brit branch --delete`/`-d` (with `--force`), `-D` and `--move`/`-m` follow Git's safety
  rules: a branch checked out, rebased or bisected in any worktree is never deleted, an
  unmerged branch needs `--force`, and the reflog and every `[branch "<name>"]` section
  travel with the branch. A rename repoints `HEAD` in every worktree that has the branch
  checked out, and refuses, naming the holder, while a Brit writer holds one of those
  worktrees (for example a push paused in its pre-push hook).
- `brit tag <name> [<commit-ish>]` creates lightweight or (`-a`/`-m`) annotated tags, `-f`
  replaces one, and `brit tag --delete` removes several atomically.
- `brit fetch [<repository>] [<refspec>…]` takes the remote positionally (a configured remote,
  a URL, or a path to a repository) and updates the configured remote-tracking ref for a branch
  fetched without a destination. A first positional that is neither a remote nor a ref on the
  remote is named in the refusal.
- Global `--wait[=SECONDS]` and `BRIT_MUTATION_WAIT=SECONDS|forever` wait for another Brit
  writer's guard instead of refusing at once; bare `--wait` waits without limit.

### Changed

- Writer guards are scoped ([docs/mutation-guards.md](../docs/mutation-guards.md)). `add` holds
  its worktree's guard and the repository guard shared, so writers in other worktrees keep
  working beside it; shared writes (refs, configuration, worktree registrations, `fetch`'s
  whole transfer) hold the repository guard exclusively; `switch`, `pull`, `rebase`/replay,
  `restore`, `merge tree --update-head` and `clean --execute` hold their worktree's guard and
  the repository guard exclusively, including their own hooks.
- `push` holds its own worktree's guard for its whole run, but the repository guard only
  shared, never across its pre-push hook, and never queues behind an exclusive writer.
- `commit` holds its worktree's guard for its whole run and the repository guard (shared)
  only at its start and to publish the tree, commit and ref; it holds none during
  `pre-commit`, `prepare-commit-msg` and `commit-msg`, and no guard at all during
  `post-commit` and `post-rewrite`.
- A refusal names each holder (pid, program and Brit subcommand path, role, working
  directory, start time, session) from `/proc/locks`; option values, positionals and URLs are
  never shown.
- **Behaviour change:** `brit add`, `brit commit`, `brit switch` and `brit branch -m` of the
  checked-out branch now refuse in a worktree whose push is still running, its pre-push hook
  included.
- **Behaviour change:** with no wait configured, a shared writer gets a 1 s grace behind a
  queued exclusive writer before refusing (naming both), and `add` and a commit start get a 2 s
  start patience behind a momentary exclusive holder; other writers still refuse at once.
- Hooks never wait on their own command: a writer under a Brit hook (`BRIT_GUARD_HELD_BY`,
  trusted only when that process is really its ancestor) refuses at once on a guard its
  command holds, and any other wait under a hook is capped by `BRIT_HOOK_GUARD_WAIT`
  (default 30 s).
- **Behaviour change:** bare `brit log` in a shallow repository now stops at the shallow
  boundary, as Git does, even when the boundary's parents happen to be present (an interrupted
  deepen or a shared object store), so `brit log` equals `brit log HEAD`. Outside shallow
  repositories its output is unchanged.
- `brit pull --ff-only` fast-forwards a shallow repository above its boundary. A refusal of the
  remote-tracking update (non-fast-forward, or needing history below the boundary) changes no
  reference, branch or worktree and leaves no pending pull, so a deepening fetch or a retry can
  run at once; one below the boundary names the deepening fetch.
- A commit whose branch was moved by another writer after its checks names that concurrent
  move and says no reference changed.
- **Behaviour change:** `brit build fingerprint` keys every gitlink a step glob matches or can
  reach by the commit it pins, and every matched or reached symlink (including a glob written
  through a directory link) by its object id, so moving a submodule pin or retargeting a link
  changes the step's fingerprint. Uncommitted files and a symlink's target file (unless a glob
  matches it) are still not keyed.
- C dependencies compile at the Rust target's CPU baseline: the `cc` build dependency is
  1.11.1, which passes `-march=x86-64` for x86_64 Linux, and the CPU-baseline build probe
  checks the command line `cc` actually gives dependencies
  ([docs/cpu-compatibility.md](../docs/cpu-compatibility.md)).

### Fixed

- `brit fetch` refuses a refspec naming one ref the remote does not advertise, checks again
  after reconnecting, and exits non-zero when a ref update is rejected.
