# Changelog — brit-cli

All notable changes to the `brit` executable are recorded here; changes to `brit-epr` and the
crates released with it are in [`brit-epr/CHANGELOG.md`](../brit-epr/CHANGELOG.md), and
release narratives in [`docs/releases/`](../docs/releases/). The root `CHANGELOG.md` is
upstream gitoxide's generated file and is not edited here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- `brit log [REV] [--oneline] [-- PATH…]` starts the walk from any revision (default `HEAD`)
  and limits it to commits that change literal paths, following Git's default history
  simplification; a trailing slash matches only directories. `--oneline` prints the
  `core.abbrev` hash and title.
- `brit branch --delete`/`-d` (with `--force`), `-D` and `--move`/`-m` follow Git's safety
  rules: a branch checked out, rebased or bisected in any worktree is never deleted, an
  unmerged branch needs `--force`, and the reflog and every `[branch "<name>"]` section
  travel with the branch. A rename repoints `HEAD` in every worktree that has the branch
  checked out, and refuses, naming the holder, while a Brit writer holds one of those
  worktrees (for example a push paused in its pre-push hook).
- `brit tag <name> [<commit-ish>]` creates lightweight or (`-a`/`-m`) annotated tags, `-f`
  replaces one, and `brit tag --delete` removes several atomically.
- `brit fetch [<remote>] [<refspec>…]` takes the remote positionally and updates the
  configured remote-tracking ref for a branch fetched without a destination.
- Global `--wait[=SECONDS]` and `BRIT_MUTATION_WAIT=SECONDS|forever` wait for another Brit
  writer's guard instead of refusing at once; bare `--wait` waits without limit.

### Changed

- Writer guards are scoped. `add` and `commit` hold their worktree's guard and the repository
  guard shared, so writers in other worktrees keep working beside them; shared writes
  (refs, configuration, worktree registrations) hold the repository guard exclusively;
  `switch`, `pull`, `rebase`/replay, `restore`, `merge tree --update-head` and
  `clean --execute` hold their worktree's guard and the repository guard. `push` keeps its
  worktree's guard from before its pre-push hook until after send-pack and releases only the
  repository guard across the hook, so other worktrees stay writable while it runs.
- A refusal names each holder (pid, program and Brit subcommand path, working directory,
  start time, session) from `/proc/locks`; option values, positionals and URLs are never
  shown.
- **Behaviour change:** `brit add`, `brit commit`, `brit switch` and `brit branch -m` of the
  checked-out branch now refuse in a worktree whose push is still in its pre-push hook.
- **Behaviour change:** once an exclusive writer is queued, a new shared writer without
  `--wait` refuses instead of overtaking it, so overlapping worktree writers cannot starve it.
- A hook or editor never waits for the command it runs under: a writer started from a Brit
  hook (`BRIT_GUARD_HELD_BY`, or the process ancestry) refuses at once, saying so.
- `brit pull --ff-only` fast-forwards a shallow repository above its boundary, and a refusal
  that needs history below the boundary leaves nothing pending and names the deepening fetch.
- A commit whose branch was moved by another writer after its checks names that concurrent
  move and says no reference changed.

### Fixed

- `brit fetch` refuses a refspec naming one ref the remote does not advertise, and exits
  non-zero when a ref update is rejected.
