# Mutation guards

Brit serializes its own writers with advisory `flock` locks. Git and other tools do not take
these locks; they protect Brit writers from each other across the linked worktrees of one
repository. This page is for anyone running Brit concurrently in several worktrees, and for
hook authors.

## The locks

| Lock | File | Taken by |
|---|---|---|
| Repository guard | `<common-dir>/brit-mutation.guard` | Exclusive by writers of shared state (refs, config, worktree registrations, recovery journals). Shared by worktree-local writers and push. |
| Worktree guard | the worktree's own Git directory, locked as a directory | Exclusive by every writer of that worktree's index, HEAD or files. |
| Turnstile | `<common-dir>/brit-mutation.turnstile`, created the first time an exclusive writer has to wait | Held by a waiting exclusive writer until it has the repository guard. New shared acquirers pass through it, so a stream of shared holds cannot starve the waiter. |

Order: a worktree guard is always taken before the repository guard, and nobody waits for a
worktree guard while holding the repository guard.

## Who holds what

| Command | Locks, in order | Held across |
|---|---|---|
| `add`, `status` index refresh | worktree, then repository shared | the whole command (`status` never waits and skips its refresh when contended) |
| `commit` | worktree for the whole command; repository shared only at start and to publish | Released before `pre-commit`, `prepare-commit-msg` and `commit-msg`. Re-taken (patience at least 600 s) to write the tree, the commit and the ref. Every guard is released before `post-commit` and `post-rewrite`. |
| `push` | worktree for the whole command; repository shared for resolution and send-pack | Released across `pre-push`, re-taken (patience at least 600 s) for send-pack. Push never queues at the turnstile. |
| `switch`, `pull`, `rebase` (replay), `restore`, `merge tree --update-head`, `clean --execute` | worktree, then repository exclusive | the whole guarded section, **including their own hooks** (`post-checkout`, `post-merge`) |
| `fetch`, `branch`, `tag`, `config`, `worktree add`, and every other writer | repository exclusive | the whole command, **including fetch's whole transfer** |
| `worktree remove` | repository exclusive, then the target worktree's guard without waiting | refuses at once if that worktree is in use |

The long holds that remain are deliberate: switch, pull and replay journal their work and keep
the repository exclusive until it is durable, and fetch holds it for its transfer.

## Waiting

- By default a writer that finds a lock held refuses at once and names the holder: pid, Brit
  subcommand (never its arguments), role, directory, start time and session.
- `add` and a commit start wait up to 2 seconds by default, so a momentary fetch or branch
  command does not fail them.
- `--wait[=SECONDS]` (before the subcommand, or anywhere for plumbing commands) waits instead:
  bare `--wait` without limit, `--wait=0` refuses at once. `BRIT_MUTATION_WAIT=SECONDS` or
  `forever` sets the default. For `brit context` and `brit integrate`, only a `--wait` before
  the passed-through arguments is Brit's.
- A shared writer that did not ask to wait still waits about 1 second at an engaged turnstile
  before refusing; the refusal names both the queued writer and the holder it waits for.
  A turnstile holder that is stopped (SIGSTOP) is passed.

## Hooks

Every hook and editor Brit runs carries `BRIT_GUARD_HELD_BY`, naming the Brit process, its start
time and the locks it holds. A Brit writer inside the hook trusts it only when that process is
really its ancestor.

- **Same-worktree writers refuse at once.** A `pre-commit` hook cannot run `brit add` in its own
  worktree, nor a `pre-push` hook `brit commit`: the command the hook runs under holds that
  worktree. Stage files from a hook through the index directly, for example with `git add`.
- **Exclusive writers inside a `post-checkout`/`post-merge` hook refuse at once**: switch and pull
  hold the repository exclusively while their hooks run.
- **Other writers work.** A `pre-push` or `pre-commit` hook may create branches, write notes or
  commit in another worktree; writers in another repository are unaffected. `post-commit` and
  `post-rewrite` hold no guard, so they may run any Brit writer, in their own worktree too.
- **Waits under a hook are capped** at `BRIT_HOOK_GUARD_WAIT` seconds (default 30), because the
  holder may itself be waiting for the hook's command. A pre-push gate that commits or writes
  notes while another lane runs a long exclusive operation (a switch, a pull, a fetch) therefore
  fails after the cap rather than waiting it out.

## Recovery journals

`switch`, `pull` and `rebase` journal their work under the worktree's Git directory, and
while a journal is pending every writer in every worktree refuses and names the command that
recovers it. Only that operation's own recovery command, run in the worktree that owns the
journal, removes it: a writer elsewhere holds the repository guard (perhaps only shared) and
its own worktree's guard, never the owning worktree's, so clearing another worktree's journal
would be a write the model reserves for `other_worktree_guard`.

- A `switch` or `pull` stopped after creating its journal directory and before publishing its
  first record leaves `brit-switch` with no `manifest.json`. Both publish that record before
  they take any lock or touch any live file, so nothing was changed. `brit switch --status`
  says so, other writers refuse naming `brit switch --abort`, and `--abort` (`brit pull --abort`
  too) removes it; `--continue` refuses, since nothing was recorded.
- A rebase whose preparation failed, or was killed, is cancelled by `brit rebase --abort`
  without being rolled forward: preparation changes nothing live, so the abort reads only the
  journal's own records and payloads and the pinned HEAD, index and branch, and restores
  nothing. An interrupted cancellation is finished by the next `--abort`. A `prepared`
  journal that already holds the pre-rebase private index and a prepared, never launched
  hook invocation is cancelled the same way, unless a live supervisor still holds that
  invocation's lease; once the hook's start is durable it is not.
- The scratch quota never refuses recovery: opening a journal never applies it, so
  `--status` always answers, and `brit rebase --abort` and the cleanup of a completed or
  aborted rebase are unmetered in every phase. Only forward steps are reserved for. The same
  holds for `switch` and `pull`: `brit switch --status` is never metered, and `switch --abort`,
  `pull --abort`, rolling a durable switch decision forward with `--continue`, and the
  standalone conflict-authoring abort run unmetered too.
- A command killed part-way through publishing a journal record never strands the journal.
  Rebase records are written whole as `state.pending` and then renamed onto `state.json`, with
  nothing live written between the two; a `state.pending` found beside `state.json` never took
  effect, and the next rebase command other than `--status` discards it (a preparation's own
  successor is rolled forward instead). A decision record, a staged proposal or its owner record
  cut short by a kill or a power loss is likewise treated as never written, and an abort that
  was killed holding its transition's Git locks before its decision releases them on the next
  `--abort`. Retained-copy manifests, completion markers and cleanup receipts are published
  whole (written to a scratch spool and renamed into place); in a copy not yet complete, a
  manifest cut short is written again, while one pinning sources that changed since still
  refuses, and a complete copy stays immutable.
- A completed or aborted rebase whose cleanup copy is all that remains (or an abort retaining
  before it changed anything live) can be set aside with `brit rebase --retire`, under the same
  guards as `--abort`: the journal is kept as `brit-rebase-kept-<nonce>`, which no writer
  treats as pending. `--retire` removes no lock and refuses every other phase.
- A rebase journal that does not open as this repository's own (moved or copied repository,
  changed permissions, corrupt checksum) is touched by no command: every writer refuses,
  `brit rebase --status` says what failed and what to do by hand, and warns that a copied
  repository's journal may describe another repository's live rebase. When the repository was
  moved, `--status` names the location the journal records: moving the repository back and
  running `--abort` there restores it exactly; setting the journal aside by hand instead leaves
  the checkout part-way.
- A `.brit-checkout-*` directory in a Git directory is a scratch leftover: a Brit process
  killed while it held one (a journal save, a payload copy, a switch or pull start) leaves it.
  It is not a journal and blocks nothing; `rebase --status` and `switch --status` report it
  and the command that removes it.
