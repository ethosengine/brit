# Changelog — brit-cli

All notable changes to the `brit` executable are recorded here; changes to `brit-epr` and the
crates released with it are in [`brit-epr/CHANGELOG.md`](../brit-epr/CHANGELOG.md), and
release narratives in [`docs/releases/`](../docs/releases/). The root `CHANGELOG.md` is
upstream gitoxide's generated file and is not edited here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.3.2 — 2026-11-22

### Fixed

- brit 0.3.1's crates can be published: `brit-graph` moves to 0.2.4, because 0.3.1 changed it
  (`ContentFingerprint::from_repo_globs_excluding`) at the 0.2.3 the registry already held, so
  main's publish refused it as an immutable version collision and `brit-cli` 0.3.1 never reached
  the registry. `brit-cli` requires `brit-graph` 0.2.4. No behaviour change from 0.3.1.
- `brit rebase` works on large repositories. What a rebase reads, verifies and keeps now scales
  with its change instead of the repository's history: selection walks only the replayed range
  above the merge base; verification covers the replayed commits, their tree deltas over their
  parents and the upstream's and base's trees along the touched paths; the journal and the
  retained copy hold those commits and what they introduce over a *floor* (the merge base and
  the upstream, which the user's own refs keep reachable), not the history beneath it. A rebase
  whose history alone exceeded the 1 GiB scratch limit (the monorepo, a full clone of brit) was
  refused before starting; it is now admitted when its journal fits.
- `brit rebase` no longer refuses a repository because its index holds gitlinks (submodules):
  one the rebase leaves unchanged is index-only, never looked up as an object and its directory
  never read or written. A gitlink the rebase would change, or one staged differently from
  `HEAD`, is still refused by name before anything changes.
- The worktree witness of a rebase and of its conflict and edit stops no longer reads untracked
  or ignored files outside the touched paths: build output or a new file elsewhere no longer
  refuses `--continue` ("worktree mutation outside resolution scope" / "worktree changed outside
  replay scope"), and a large `target/` or `node_modules/` is never hashed.

### Changed

- Retained-copy manifests are version 2 for every new copy, recording the floor at which the
  release proof stops (a tree entry equal to its parent's same-path entry counts as inherited).
  This Brit still reads, restores from and cleans version 1 copies and the journals Brit 0.2.6 to
  0.3.1 wrote; an older Brit refuses a version 2 copy, or a journal with a floor, without touching
  it.
- The rebase journal stores its preservation record (the original index and its entries) once, as
  a payload, instead of in every state record: on the monorepo's 21,667-entry index each record
  was 56 MB, about three per decision, so a two-commit rebase projected a 1.48 GB journal and was
  refused at admission.
- The scratch limit meters the rebase journal only. A retained copy, a `--skip`'s included, is
  durable data under the Git directory and is never metered; the preflight's refusal of a
  history whose objects exceed the limit, and admission's test of the copy, are gone.
- CI: the planned jobs `.epr-meta/readiness.json` declares deferred run in their own job,
  `run-deferred`, which `Tests pass` does not need, so a deferred leg can no longer hold up or
  fail the gate. Required legs run in `run` as before.

## 0.3.1 — 2026-11-21

### Added

- `brit build affected --inputs sources|all` keeps a step's two input classes apart. With the
  flag omitted, the planner and the JSON are as before. With it, each `affected_by` entry
  carries `inputClass` (`sources`, `buildProcess` or `upstream`), and a step lists every class
  that matched, each with its own first matching path. `sources` selects from a step's sources
  alone, minus its own `buildProcess`, so a step selected only by its pipeline file is not
  selected, and neither are its dependents.
- `brit build fingerprint --inputs sources|all`. Omitted: the same CID and JSON as before.
  `sources` leaves the pipeline file's bytes out of the step's fingerprint, including where a
  source glob covers it. `all` hashes the sources and the buildProcess files the planner
  matches (a `File@fn` entry counts as `File`).

### Changed

- Requires rakia-core and rakia-brit 0.1.2 (`inputs`, `plan_from_changes_classified`).

## 0.3.0 — 2026-11-20

### Added

- `brit push --atomic` asks the remote to apply every update or none (the `atomic`
  capability). A remote that does not offer it refuses the push before anything is sent with
  Git's `fatal: the receiving end does not support --atomic push` and status 128. Before, the
  flag was accepted and ignored.
- `brit log [REVISION…] [--oneline] [-- PATH…]` lists the commits reachable from the given
  revisions instead of `HEAD`, including exclusions and ranges (`^A`, `A..B`, `A...B`, `A^@`,
  `A^!`), ordered like `git rev-list --date-order`, and limits them to commits that change
  literal paths, following Git's limiter and default history simplification; a trailing slash
  matches only directories. `--oneline` prints the `core.abbrev` hash and title.
- `brit diff tree --name-only` lists changed paths like `git diff --name-only --no-renames`;
  `diff tree` also takes `--merge-base OLD NEW` and a lone `A..B` or `A...B`.
- `brit diff [<revision>…] [-- <path>…]` compares like `git diff --no-renames`: no revision
  (working files against the index, or with `--cached` the index against HEAD), one revision
  (against the working files, or with `--cached` the index), two revisions, `A..B`, or `A...B`
  (the merge base of A and B against B), with `--merge-base`, in every output form: patch,
  `--stat`, `--name-only`, `--name-status`, `--numstat`, `--exit-code` and `--quiet` (status
  1 when there are differences). A name that is both a revision and a file is ambiguous, as in
  Git; `<name> --` reads it as a revision and `-- <name>` as a path. Unknown revisions are named
  with Git's first line (`ambiguous argument '…': unknown revision or path not in the working
  tree.`), followed by a shorter second line, and exit 1 where Git exits 128.
- `brit show [<revision>…] [-- <path>…]` is Brit's own verb (it was a Git bridge operation):
  commits with Git's medium header and their patch against the first parent, merges (header
  only, or their first-parent summary with `--stat`/`--numstat`), annotated tags, blobs
  (`REV:PATH`) and trees, like `git show --no-renames`, with `--stat`, `--name-only`,
  `--name-status`, `--numstat`, `-s`/`--no-patch` and `--oneline`. A commit message is re-encoded
  to UTF-8 from its declared `encoding`.
- Measured limits of `diff` and `show`, each a refusal rather than different output unless
  said otherwise: a path with a `diff=<driver>` attribute (even a built-in driver with nothing
  configured) refuses the patch and count forms; intent-to-add, skip-worktree and
  assume-unchanged entries, sparse and split indexes and a conflicted index refuse the forms
  that read the index; a submodule refuses comparisons with the working files; a merge that
  changes a path against every parent (Git's combined diff) refuses `show`'s patch,
  `--oneline`, `--name-only` and `--name-status`; an encoding label Brit cannot decode exactly
  as Git, and non-ASCII header or message text when the output encoding
  (`i18n.logOutputEncoding`, else `i18n.commitEncoding`) is not UTF-8, refuse; `show` refuses a
  range. Line counts can differ from Git's on very large, heavily changed files (different
  Myers heuristics): three of 26,673 files on the monorepo's `origin/main...origin/dev`, by two
  to five lines, with `--name-status` identical.
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
  Git exits 129. The relative answers of the `rev-parse` path queries print `/` on every
  platform, as Git does: `--show-toplevel`, `--git-dir` and `--git-common-dir` after
  `--path-format=relative`, and `--git-common-dir` without `--path-format` when it answers
  relative to the current directory (this worktree's own `.git`, such as `../.git`). The
  absolute answers (`--show-toplevel` and `--absolute-git-dir`, `--git-dir` and
  `--git-common-dir` when they name a path absolutely, and every query after
  `--path-format=absolute`) still print native `\` separators on Windows where Git for Windows
  prints `/`. Brit's `-r <dir>` starts discovery at `<dir>` and may find an ancestor, where
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
- `brit rebase --retire`: sets aside a completed or aborted rebase whose retained copy is all
  that remains, or an abort or a skip retaining before it changed anything live, keeping the
  journal as `brit-rebase-kept-<nonce>` with its retention refs and retained copies and printing
  `Retired the rebase; nothing was deleted and nothing live was changed.` and how to remove
  each piece (details under Fixed). `brit rebase --status` then lists it as
  `Kept by --retire (blocks nothing)`.
- Test builds only (`switch-test-faults`, never in a shipped build): `BRIT_SWITCH_CHECKPOINT`
  accepts `<name>#<N>` to pause the Nth time a process reaches a checkpoint, every journal save
  passes `replay-state-file-writing` before it writes `state.pending`, and
  `BRIT_TEST_SCRATCH_LIMIT` lowers the scratch limit.

### Changed

- **Behaviour change:** `brit push` refuses to push ungated when the `pre-push` hook is
  expected but cannot run: `core.hooksPath` names something that is not a directory, or the
  hook is not an executable file (a dangling link included). The refusal names the path and
  `--no-verify`, the explicit way to push without the hook; nothing is sent. Git pushes without
  the hook and without a word in both cases.
- `brit push` fast-forwards any ref but a tag after checking the remote's commit is an ancestor,
  as Git does, so a branch and its notes (`refs/notes/…`) update together in one push; before,
  only `refs/heads/*` could be updated without `--force`. An existing tag moves only with
  `--force` ("refusing to update existing tag …").
- `brit push` handshakes once when no `pre-push` hook ran (none installed, or `--no-verify`);
  only after a hook ran does it connect again and re-validate the refs it judged.
- Merged onto the gitoxide 3e89f3b56 integration. `brit log` keeps upstream's `--sort` and
  `--first-parent` next to `--oneline` and paths; a path filter takes only the default date
  order over all parents and refuses the others. Without paths, the default order still lists
  exactly the commits the excluded revisions cannot reach: upstream's topological walk now
  limits hidden ancestry by commit date, as Git does, so `brit log` paints the excluded
  ancestry itself and prunes the walk there. `brit show` decodes a declared commit encoding
  with `gix-encoding`'s iconv names and tables instead of `encoding_rs`; `windows-31j` now
  decodes as CP932, as Git's iconv does, where it was refused before.

- **Behaviour change:** `brit rebase` and `brit switch`/`brit pull` recovery follows five
  rules ([docs/rebase.md](../docs/rebase.md)): admission refuses before anything changes
  (`rebase refused before it started: …`, with `git rebase` named as the fallback); a forward
  step (`--continue`, `--stage`, `--skip`, an edit) may be refused before it writes, and the
  refusal names `--abort`; `--abort` and cleanup are never refused by the scratch limit, and
  neither are `switch --abort`, `pull --abort`, `switch --status` or rolling a durable switch
  decision forward; `--retire` applies only to this repository's own journal in `completed`,
  `aborted`, or `retaining-abort` or `retaining-skip` with no decision taken (no command
  sequence reaches the last of these: every stop a skip starts from is a decision); and a
  journal that does not open as this repository's own is reported by `--status` and never
  touched. Before, a failed or
  killed preparation, a quota refusal at a large conflict resolution, or a killed command's
  unrenamed record could leave a journal that `--abort` and `--continue` both refused, and
  every Brit mutation then refused "pending replay". New `--status` lines: `Cancelled
  preparation not yet removed: …; brit rebase --abort finishes it`, `Scratch left by a killed
  Brit process (blocks nothing): …`, `Kept by --retire (blocks nothing): …`, `A rebase journal
  of this repository exists but could not be read in full: …`, `A rebase preparation stopped
  before it recorded anything and changed nothing live; brit rebase --abort removes its
  directory …`, and, for a switch or pull, `Switch or pull start stopped before its first
  record: it recorded nothing and changed nothing; …`. These rules come from gitoxide-core's
  replay and switch modules; its upstream-generated CHANGELOG.md is left byte-identical.

- **Behaviour change:** `brit diff -- <path>` (and `brit show … -- <path>`) whose paths select
  nothing exits 0 with no output, as Git does; `diff` exited 1 before. `brit show` is no longer
  answered with the bridge line and exit 2.
- **Behaviour change:** a read-only command whose reader closes its output early (`brit log |
  head`, `brit rev-parse … | head`) ends quietly with status 141, like Git killed by `SIGPIPE`,
  instead of reporting the broken pipe. This applies only to the commands listed by exact
  subcommand path in `brit-cli`'s `READ_ONLY_COMMANDS` (the Git spellings above, `log`, `cat`,
  `blame`, `diff`, `diff tree`, `diff file`, `branch list`, `revision resolve`,
  `revision explain`, `worktree list`, `config-tree`, `env`, `--cli-surface-json`), and only
  when the command otherwise succeeded: a command's own failure always keeps its status. So
  `brit rev-parse --verify missing` with a closed stderr exits 128, where Git is killed by
  `SIGPIPE` (141). Every other command, writers included, reports its own outcome.
- **Behaviour change:** 28 more exact paths end quietly with 141 on a closed reader, each admitted
  after reading its code for writes and unwatched output: `show`, `tag list`, `remote url`,
  `remote refs`, `remote ref-map`, `revision previous-branches`, `verify`, `fsck`,
  `tree entries`, `tree info`, `commit-graph list`, `commit-graph verify`, `mailmap entries`,
  `mailmap check`, `attributes query`, `exclude query`, bare `submodule`, bare `config`,
  `config show`, `config list`, `index entries`, `dirwalk`, and `build graph discover`,
  `build graph show`, `build affected`, `build plan`, `build fingerprint`,
  `build baseline read`. The whole list, with the reason beside each command left out, is
  `READ_ONLY_COMMANDS` in `brit-cli/src/main.rs`. Left out: bare `tag` (`tag <name>` creates a
  tag on the same path), `revision list` (`--svg` writes and opens a file), `commit verify` (its
  verifier writes to an unwatched stderr), `commit describe` and `submodule list`
  (`--dirty-suffix` runs the status engine, which can run a configured clean filter),
  `is-clean` and `is-changed` (predicates whose answer is their exit status),
  `attributes validate-baseline` (it reports a failure with a write that a closed reader would
  replace), and the writers (`build run`, `build baseline write` and `migrate`, `config fmt`,
  `index from-tree`, `tag delete`, `commit sign`). Measured with stdout closed: `brit show HEAD`
  exited 1 with `Error: Broken pipe`, now 141; `brit index entries` exited 0, now 141; a short
  blob without a newline (`brit show HEAD:file`) exited 0, now 141, because a read-only
  command's stdout is flushed before its status is chosen. A command's own failure still keeps
  its status: `brit tree info no-such-revision` with stderr closed exits 1, because it now
  resolves the revision before writing its format note. Caveats: `ls-remote`, `remote refs` and
  `remote ref-map` may run a credential helper, which stores or erases a credential after
  authenticating, and output a helper or prompt writes does not pass the watched writer, so on
  that path a closed reader may go unseen. `--trace` and the verbose progress renderer write to
  stderr outside the watched writer. A streaming query (`exclude query`, `attributes query`) stops at its first failed write, even when a later input would have failed,
  as Git killed by `SIGPIPE` does.
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
- **Behaviour change:** command-scope `safe.directory` (`-c safe.directory=<path>` and
  `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_<n>`/`GIT_CONFIG_VALUE_<n>`) now grants ownership trust, as
  in Git, beside the system and global files. Repository configuration, an `includeIf` in the
  command scope, and an environment `include.path` when includes are not permitted never grant
  it.
- **Behaviour change:** a repository opened at reduced trust (owned by someone else and not
  declared with `safe.directory`) sizes its object store from the pack indexes present at open,
  plus headroom for packs written while it is open, instead of a fixed 32 slots; a
  multi-pack-index counts as one. Up to 512 index slots are allowed. A repository needing more
  is refused with one named error and its two cures: ``object database index slots exhausted:
  the repository needs at least <n> index slots and this process may use <m>; its owner can
  consolidate its packs (`git repack -a -d`), or, if this checkout is trusted, declare it with
  `safe.directory` (a global entry, `-c safe.directory=<path>` or GIT_CONFIG_COUNT)``. The same
  error replaces the outer "rev-spec is malformed" wrapper when a store runs out of slots later
  (a pack written while a repository at the ceiling is open). Fully trusted repositories are
  unchanged.
- **Behaviour change:** every brit binary (`brit`, `brit-rebase-retained`, `brit-build-ref`,
  `brit-verify`) prints a failure as `Error: <message>`, then its causes under `Caused by:`
  (numbered when there are several), with the deepest real message last: a gix error class
  such as `NotFound` is folded into the line it classifies (`Reference "x" could not be found
  (NotFound)`) instead of ending the output on its own. `RUST_BACKTRACE` and
  `RUST_LIB_BACKTRACE` no longer make brit print a backtrace; `BRIT_BACKTRACE=1` does, before
  the error, so the error stays the last line. Build command errors keep their one-line
  `error: …` form.
- Known differences from Git in the trust decision: an `includeIf` in a global or system file
  is evaluated against the repository being opened (so do not keep `safe.directory` behind a
  conditional include); `-c include.path=…` is not followed; `GIT_CONFIG_PARAMETERS` (how
  `git -c` reaches Git's children, such as a hook that runs brit) is not read;
  `safe.directory=<R>/*` also trusts `<R>` itself; with `GIT_DIR` and its own `GIT_WORK_TREE`
  set, the ownership check is still made. See
  [docs/plans/2026-11-15-bounded-memory-backlog.md](../docs/plans/2026-11-15-bounded-memory-backlog.md).
- The covenant ref manager resolves revisions with `git rev-parse --end-of-options`, which
  needs Git 2.30 or newer.

### Fixed

- A push that delivered its refs exits 0 even when its report meets a closed stdout or stderr
  (a reader that went away); before, the first failed status line turned the delivered push into
  exit 1. `brit build baseline write` and `brit build baseline migrate` follow the same rule once
  their ref changed. The note that a lost report was verified by a fresh advertisement is now
  printed by the CLI after the per-ref lines, and a ref already at its requested id reads
  `= [up to date]`.
- `brit rebase --abort` cancels a rebase whose preparation failed or was killed, without
  re-running it: it reads only the journal's own records and the pinned HEAD, index and branch,
  never an object, and reports any live difference instead of refusing. Before, a preparation
  that failed (for example on an uninitialised submodule's gitlink, which it looked up as a local
  object) left a journal that neither `--abort` nor `--continue` could clear, and every Brit
  mutation refused "pending replay". An interrupted cancellation is finished by the next
  `--abort`.
- `brit rebase` refuses a gitlink whose submodule directory is absent by name, before any
  state, instead of failing after creating its journal. Gitlinks remain unsupported.
- `brit rebase` no longer starts or continues work whose journal its 1 GiB scratch limit
  could not hold, and the limit never refuses undoing. An estimate from object headers refuses
  far-too-large repositories in about a second, saying brit rebase is not yet available at
  that size (a history over 1 GiB decompressed, or roughly 390,000 objects for a one-commit
  replay) and that `git rebase` is the fallback; once preparation knows its exact records, a
  rebase whose journal could exceed the limit with a 5/4 safety factor, allowing for one stop
  and its resolution, is refused and its preparation cancelled; every later forward step
  (`--continue`, `--stage`, `--skip`, an edit) is reserved for before it writes anything, and a
  refusal names `--abort`; a `--skip` reserves for the retained copy it writes the same way.
  Each refusal says whether the journal or a retained copy refused it. `--abort` and the
  cleanup of a completed or aborted rebase are never refused by the limit, in any phase, so an
  admitted rebase can always be undone or finished; only the disk bounds them, and a write that
  fails on a full disk removes its partial file so the same command resumes. Opening a journal
  never applies the limit, so `--status` always answers. (Earlier changes in this release
  admitted rebases that then stranded, later refused `--abort` itself at a large conflict
  resolution, and then refused rebases that completed under the limit before, such as the
  64 MiB fixtures of `tests/cli-journey/tests/memory_replay.rs`, by reserving for a projected
  retained copy; those complete again.)
- **Behaviour change:** the retained copy an abort or a cleanup writes is not metered, so it
  is no longer projected or reserved for, and it can exceed the scratch limit: it holds every
  object of the rebase's history, the whole journal (which the limit bounds), the original and
  observed index files, and each touched path's before-image, worktree file and up to three
  staged versions, plus a manifest, and can exceed the limit by at most that much. This is the
  consequence of never refusing an abort. Measured on the 64 MiB memory fixtures it reached
  0.67 to 0.81 GB (0.94 GB with a conflict stop and a staged resolution) while the journal
  stayed under 0.34 GB. Only a copy that certainly cannot fit is refused, before anything
  changes: a history whose objects alone exceed the limit, and a copy whose members already
  exceed it at admission. After admission the copy is never tested and can grow past the
  limit with the rebase: a copy that is not metered must not refuse a step. After admission
  only the metered journal, and a `--skip`'s metered copy, can refuse a step.
- Known limits of the journal's admission, each measured at a lowered limit by this lane's
  reviewer: an admitted rebase can still be refused at a later step, before that step writes
  anything, with `--abort` restoring the original state exactly. (1) An `edit` stop:
  admission runs before the sequence editor and counts no edit stop, and accepting an amend
  archives a receipt for every object of the stop's whole tree, so `--continue` after an
  amend of unchanged size can be refused (scaled to the real limit, an edited file of roughly
  138 to 166 MB in a tree holding little else). (2) A conflict under `merge.conflictStyle`
  `diff3` or `zdiff3` whose base is much larger than both sides: the conflicted file holds
  the base too, and the rebase is refused when the conflict is found or at the step after it.
  (3) `--stage --expand-scope` of a large file reserves about half of what it writes and can
  fail part-way at a tight limit, leaving a payload in the journal that `--status` does not
  mention; `--abort` restores `HEAD` and keeps the file. In (1) and (2) the work done at the
  stop is then kept only in the retained copy under `.git/brit-rebase-retained/`.
- `brit rebase --continue` at an edit stop no longer reserves for a state record many times
  larger than the one it writes: the edited file adopted from the authoring session was
  counted at its inline encoding (about twelve times its size, three times over) instead of as
  the payload the record keeps. A 20 MiB edit with `--expand-scope` was refused at
  `--continue` asking for 797,656,311 bytes; it now asks for 42,686,414 and completes.
- `brit rebase --retire` sets aside a completed or aborted rebase whose retained copy is all
  that remains (or an abort, or a skip, retaining before the rebase changed anything live),
  deleting nothing and changing nothing live: it keeps the journal as
  `brit-rebase-kept-<nonce>`, its retention
  refs and retained copies, and prints how to remove each. It refuses every other phase,
  naming `--abort` or `--continue`, and removes no lock.
- A rebase journal that does not open as this repository's own (moved or copied repository,
  changed permissions, corrupt checksum) is touched by no Brit command; `brit rebase
  --status` says what failed, warns that a copied repository's journal may describe another
  repository's live rebase, and names what to remove by hand.
- `brit rebase --abort` declines to cancel a prepared hook invocation whose lease a live
  supervisor still holds.
- A rebase command killed after writing its next journal record and before putting it in
  effect no longer strands the repository: the next `--abort`, `--continue` or `--skip`
  discards that record, which never took effect, instead of refusing "foreign pending replay
  state". A decision record, staged proposal or retained-copy manifest cut short by a kill or a
  power loss is likewise written again, an `--abort` killed while holding its Git locks
  releases them on the next `--abort`, and retained-copy manifests, completion markers and
  cleanup receipts are now published whole. `brit rebase --status` names a cure in every phase
  (apart from the states in the next entry), says what is true of the decision actually
  pending, and, for a repository moved mid-rebase, names its old location: moving it back and
  running `--abort` there restores it.
- States this version can still strand, the first two produced by the reviewer by killing a
  command at the nearest checkpoint and cutting the record to half (a `SIGKILL` cannot
  realistically tear these records; a crash or power loss before the fsync can), the third
  after an ordinary `SIGKILL` with no record torn: a torn `metadata-lock-N.json` in an
  abort transition or in the forward first checkout, and a torn `retention-N.json`, leave
  `--abort`, `--continue`, `--skip` and `--retire` all refusing, and `--status` then names a
  cure that does not work; a torn cleanup receipt left by an older Brit or a power loss leaves
  a published rebase whose `--continue` refuses and blocks every mutation; after an `--abort`
  is killed mid-copy, a stock `git add` of an unrelated dirty file, even undone with
  `git reset -- <file>`, makes `--abort` refuse "foreign replay metadata before abort" and
  `--retire` refuse, with no cure named by `--status` (at an ordinary conflict stop the same
  undo lets `--abort` work). A torn `index.pre-rebase` and a torn `proposal-N.json` are
  cleared by `--abort`. Not produced by anyone: torn `directory-*.json` or
  `payload-license-*.json`, a leftover `*.new` of the authoring child. Established by reading:
  nothing is written between "pending synced" and "renamed" (the window lies inside `save`
  and `save_bytes`, the only callers of `publish_state`; the checkpoint between the sync and
  the rename only checks for an interrupt; the three other renames of `state.pending` only
  read or sync before renaming).
  See [docs/rebase.md](../docs/rebase.md#states-this-version-can-still-strand).
- `brit switch --status` is never refused by the scratch limit, and `switch --abort`,
  `pull --abort` and rolling a durable switch decision forward are not metered.
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
- A command that fails while its stderr reader has closed exits with its own code (for example
  1 for `brit log no-such-rev`, 128 for `brit rev-parse --verify missing`) and is still recorded
  by the optional feedback capture, instead of exiting 101 from a panicking report. The same
  holds for `brit-rebase-retained`, `brit-build-ref` and `brit-verify`.
- The `build` commands' JSON and DOT output goes through the watched writer: with a closed
  stdout, `brit build fingerprint`, `graph discover`, `graph show`, `affected`, `plan` and
  `baseline read` exit 141 where they panicked with 101, and `brit build baseline write` (and
  `migrate`, through the same printer), which are not read-only, exit 1 with
  `error: io error: Broken pipe` after their ref update where they panicked with 101. Output to a live reader is byte-identical.
- A reduced-trust repository with 33 or more pack indexes (one more per unconsolidated fetch)
  opens for reading again; before, every read failed with `too few index slots` or `The rev-spec
  is malformed and misses a ref name`.
