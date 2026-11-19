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
- Git's spellings of read-only ref commands, which take no writer guard and point to Brit's own
  verbs in their help:
  - `brit rev-parse` (Brit's `revision resolve`) reads its arguments in one ordered pass, like
    Git: `--verify`, `--short[=N]`, `--abbrev-ref` and `-q` apply to the revisions after them,
    and `--show-toplevel`, `--git-dir`, `--absolute-git-dir`, `--git-common-dir` and
    `--path-format` print where they appear. Failures print Git's diagnostic and exit 128 (1
    for `--verify --quiet`).
  - `brit for-each-ref [--format=…] [--sort=…] [<pattern>…]` with Git's default format, the
    format atoms and sort keys agents use, and Git's pattern matching; other atoms and keys
    are refused by name.
  - `brit ls-remote [--heads|-h] [--tags] [--refs] [--exit-code] [<remote> [<pattern>…]]`
    (Brit's `remote refs`) prints `<oid>\t<refname>` in server order with peeled tags; with
    the remote omitted, `From <url>` goes to stderr once the remote answered, with a password
    shown as `redacted`.
  - `brit check-ref-format [--allow-onelevel] <refname>` answers by its exit code alone.
  - `brit rev-list --count <rev>…` and `--left-right --count A...B` count what
    `brit revision list` walks; listing without `--count` is refused and names Brit's verb.
  - `brit merge-base --is-ancestor <A> <B>` answers through its exit code: 0 when `A` is an
    ancestor of `B` or the same commit, 1 when it is not, 128 for a revision naming no commit.
- Known differences from Git in these spellings: a relative `GIT_DIR` prints as an absolute
  `--git-dir`; short ref names are always strict (as if `core.warnAmbiguousRefs` were true);
  non-UTF-8 ref names are matched and printed lossily; `ls-remote` needs a local repository
  even for an explicit path or URL, and a lone `-h` lists the default remote's branches where
  Git prints usage; `rev-parse` refuses parent shorthands (`A^@`, `A^!`, `A^-`) and flags Git
  echoes or expands (`-1`, `--all`) by name with 128, does not echo a lone leading `--`, loses
  the resolver's diagnostic for an ambiguous prefix, accepts `--path-format relative` as two
  arguments, shows Brit's help for `--help` after a revision, and inside a `.git` directory
  answers only `--show-toplevel` like Git; `rev-list --count` fails with 128 for a revision
  naming no commit (Git counts 0); `merge-base --is-ancestor` with no revision exits 2 where
  Git exits 129. On Windows the `rev-parse` path queries print native `\` separators where Git
  for Windows prints `/`. Brit's `-r <dir>` starts discovery at `<dir>` and may find an ancestor, where
  Git's `--git-dir` needs the directory itself; `GIT_DIR` overrides `-r`. `ls-remote` may wait
  on a configured credential helper; non-interactive callers pass `-c credential.helper=` and
  `GIT_TERMINAL_PROMPT=0`.
- Global `-C <dir>`, repeatable, with Git's meaning: each is entered relative to the previous
  one before the command runs (and before a known subcommand's `--help` or usage error), so
  repository discovery, relative paths, `-r`, and the writer guards all start there; an empty
  value is ignored, and a directory that cannot be entered ends with
  `fatal: cannot change to '<dir>': <reason>` and 128. The `build`, `snapshot`, `context` and
  `integrate` namespaces refuse it, as they refuse other global Git options.
- Test and diagnostic only: `BRIT_HTTP_REQUEST_DEADLINE_SECONDS` replaces the 60 s HTTP
  request deadline of max-pure builds (not supported configuration). The `cli-journey`
  harness runs the `push_transport` journeys against a max-pure `brit` named by
  `BRIT_REQWEST_BIN`, and with `BRIT_REQWEST_REQUIRED` set fails them instead of skipping that
  leg when it is missing; neither variable affects `brit` itself.

### Changed

- **Behaviour change:** a read-only command whose reader closes its output early (`brit log |
  head`, `brit rev-parse … | head`) ends quietly with status 141, like Git killed by `SIGPIPE`,
  instead of reporting the broken pipe. This applies only to the commands listed by exact
  subcommand path in `brit-cli`'s `READ_ONLY_COMMANDS` (the Git spellings above, `log`, `cat`,
  `blame`, `diff`, `diff tree`, `diff file`, `branch list`, `revision resolve`,
  `revision explain`, `worktree list`, `config-tree`, `env`, `--cli-surface-json`), and only
  when the command otherwise succeeded: a command's own failure always keeps its status. So
  `brit rev-parse --verify missing` with a closed stderr exits 128, where Git is killed by
  `SIGPIPE` (141). Every other command, writers included, reports its own outcome.
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
- Build step globs are fingerprinted with the same meaning on every platform: `\` escapes the
  next character, `*` and `?` also match `/`, matching is case-sensitive, and empty alternates
  and unclosed classes are invalid, on Windows as on Linux and macOS (whose fingerprints are
  unchanged). Before, a Windows build read `\` as a path separator. Which steps `brit build
  plan` and `brit build affected` select for a change is still decided by rakia-core's own
  matcher, with globset's per-platform defaults.
- **Behaviour change** on every platform: `brit build fingerprint` and `brit build plan` refuse
  a step glob with a backslash directly beside `**` (such as `src/**\[x\]`), naming the
  pattern and the backslash's position, because such a glob cannot be given one meaning on
  every platform. No pattern without a backslash is affected.
- On non-Unix hosts, `brit build fingerprint` and `brit build plan` refuse a commit whose tree
  holds a path containing a backslash (`tree path '…' contains a backslash; on this platform it
  cannot be matched with the meaning it has elsewhere; fingerprint on a Unix host or rename the
  path`), instead of keying it differently from Unix hosts.
- C dependencies compile at the Rust target's CPU baseline: the `cc` build dependency is
  1.11.1, which passes `-march=x86-64` for x86_64 Linux, and the CPU-baseline build probe
  checks the command line `cc` actually gives dependencies
  ([docs/cpu-compatibility.md](../docs/cpu-compatibility.md)).
- **Behaviour change:** `brit push` (except `--dry-run`) connects to the remote a second time
  once its pre-push stage is over, with or without a hook, and asks for credentials again on
  that connection. It sends only if every ref it names still has the id the hook judged and
  the capabilities it relies on are unchanged; otherwise nothing is sent and the refusal names
  each moved ref with both ids (`remote moved while the before-send callback ran; nothing was
  sent: …`). A ref the remote already has at the requested id is reported up to date and not
  sent; a ref that needed no update when judged and moved meanwhile is left alone and reported
  as not pushed, while the other refs proceed.
- **Behaviour change** (max-pure builds, whose HTTP backend is reqwest): every HTTP request
  except the push upload has a 60 s deadline until the response headers and a 60 s stall
  guard on each read of the body, for the handshakes of fetch, clone, pull and push and for
  fetch's pack request; the receive-pack upload and its answer have none. reqwest's previous
  30 s default cut off any push whose upload or server-side processing took longer. A server
  that computes a fetch for more than 60 s without sending keepalives now fails where Git
  waits. With `http.lowSpeedLimit` and `http.lowSpeedTime` both set, data sent but not
  acknowledged for `lowSpeedTime` seconds aborts the connection on Linux. Default (curl)
  builds keep curl's timeouts.
- An HTTP push body up to `http.postBuffer` bytes (64 MiB when unset) is sent whole with a
  `Content-Length`; a larger one is streamed in chunks.

### Fixed

- `brit rebase --abort` cancels a rebase whose preparation failed or was killed, without
  re-running it: it reads only the journal's own records and the pinned HEAD, index and branch,
  never an object, and reports any live difference instead of refusing. Before, a preparation
  that failed (for example on an uninitialised submodule's gitlink, which it looked up as a local
  object) left a journal that neither `--abort` nor `--continue` could clear, and every Brit
  mutation refused "pending replay". An interrupted cancellation is finished by the next
  `--abort`.
- `brit rebase` refuses a gitlink whose submodule directory is absent by name, before any
  state, instead of failing after creating its journal. Gitlinks remain unsupported.
- `brit rebase` refuses before it starts when its retained copy could not fit the 1 GiB scratch
  limit, naming what it measured (object headers only, stopping once past the limit) and
  pointing to `git rebase`. Before, such a rebase published and then could neither finish its
  cleanup nor be aborted. A retained copy that is still too large now refuses before writing
  any member.
- A `brit switch` or `brit pull` stopped before its first journal record no longer blocks the
  repository: `--status` says it recorded and changed nothing, and `--abort` removes it.
- `brit fetch` refuses a refspec naming one ref the remote does not advertise, checks again
  after reconnecting, and exits non-zero when a ref update is rejected.
- A push that fails anywhere in the receive-pack request reports the cause (an HTTP status
  such as 401 or 413, a reset, a timeout, a refused connection) instead of `sending on a closed
  channel`. Above `http.postBuffer` the cause of a cut upload cannot be read, and the error says
  so: `the transport closed while the pack was being streamed; the cause could not be read`.
- A push no longer goes out on a keep-alive HTTP connection the server dropped while the
  pre-push hook ran.
- After a failed send, `brit push` checks once on a fresh connection whether its refs arrived,
  and never resends. If any is missing it fails with `the push was not visible on the remote
  when checked and may still be applied; nothing was resent: …`, naming the refs that did land.
  **Behaviour change:** if every ref shows its new id, only the report was lost: the push exits
  0 and prints `warning: the server's report was lost (<cause>). A fresh advertisement shows
  every ref this push named at its requested id; it cannot show whether this push applied them,
  whether server-side hooks ran, or what the server printed.`
