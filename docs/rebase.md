# Native rebase and retained-copy recovery

This guide describes Brit's bounded local Linux interface. Qualification results,
source/artifact bindings and remaining daily-driver work are recorded separately
in the [daily-driver plan](plans/2026-09-28-daily-driver.md). A usage example is not
a claim that a particular build passed qualification.

## Start and control

```sh
brit rebase <explicit-local-upstream>
brit rebase -i <explicit-local-upstream>
brit rebase --status
brit rebase --continue
brit rebase --skip
brit rebase --abort
brit rebase --retire
brit rebase --edit-todo
brit rebase --show-current-patch
```

Supply an unambiguous local branch, tag, remote-tracking ref or full commit OID.
Remote-tracking refs are read locally; rebase does not fetch or infer a configured
upstream. Selected history must be linear and provable from local objects. Merge
selections, shallow histories and unsupported object/index/filter arrangements
refuse before live replay. The original branch stays reserved across linked
worktrees until final publication or completed abort.

Interactive todos support `pick`, `reword`, `edit`, `squash`, plain `fixup`,
`drop`, `break`, and their unambiguous one-letter aliases. Every unconsumed
candidate must appear exactly once. Completed entries cannot change. An invalid
edited todo leaves the last accepted todo intact. Squash/fixup combine into the
preceding retained group; a split records the ordered replacement commits.

`--status` and `--show-current-patch` are read-only. The patch uses the pinned
original parent and input commit, including the current squash input; it does
not derive its content from a mutable branch name or the public index. Before a
step or after completion there is no current patch.

Actions are mutually exclusive. Options outside the bounded interface are
rejected: examples include `--onto`, `--root`, `--quit`, autostash, autosquash,
`--exec`, merge-preserving replay, signing and automatic updates of other
branches. Todo `exec`, labels/resets/merge commands and `fixup -c/-C` are likewise
unsupported. Brit does not fall back to a Git subprocess.

## Resolve, amend or split

```sh
brit rebase --stage -- path/to/resolved-file path/to/deleted-file
brit rebase --stage --expand-scope -- newly-authored-file
brit rebase --amend -m 'Replacement message'
brit rebase --amend -F message.txt
brit rebase --amend --no-edit
brit rebase --amend
brit rebase --begin-split
brit rebase --commit -m 'First split part'
brit rebase --commit -F next-message.txt
brit rebase --commit
```

Name literal paths after `--`, including tracked deletions. Scope expansion is
permitted only at an edit stop; it records before-images before accepting newly
authored paths. Amendment and split commits consume the private replay index.
Preserved dirty files never enter rewritten commits. Ordinary commit/reset and
broad staging refuse during replay. External Git staging is imported only after
validation against the active scope and preserved staging distinction.

At an edit stop, amend the replayed commit or begin a split, stage each intended
part explicitly, and create its split commit. Finish with `--continue` once the
active scope is clean and the private index matches the authored tip. A message
argument and message file are mutually exclusive. `--no-edit` is amendment-only;
without a supplied message, amendment or split commit uses the normal message
editor contract and its authoring hooks. Choose the message editor before starting
and keep effective configuration consistent across control commands; changing
`GIT_EDITOR` can change the pinned configuration and is refused. Author and
committer identity is not pinned: each commit records its own, so a control
command run with another `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`,
`GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL`, `EMAIL`,
`user.name` or `user.email` continues. The `GIT_AUTHOR_DATE` and
`GIT_COMMITTER_DATE` overrides stay pinned: a resumed step reuses its proposed
commit exactly. A rebase begun by Brit 0.3.2 or earlier pinned identity too and
keeps doing so until it ends.

## Stops and interruption

- **Conflict:** the command returns nonzero with a durable stop. Stage 2 (ours)
  is the replay/upstream side; stage 3 (theirs) is the original input. Resolve,
  stage the named paths and continue, or skip/abort.
- **Empty input:** the command returns nonzero. `--continue` explicitly keeps an
  empty commit; `--skip` drops it.
- **Edit or break:** the command returns successfully at the stop. It has not
  completed the entire rebase. A break permits todo editing and continuation.
- **Skip:** current work is retained before restoration. A squash/fixup conflict
  skips only its current input; skipping an edit stop discards its entire newly
  authored split group after retaining it.
- **Abort:** original HEAD attachment, work and index distinctions are restored
  while unrelated permitted work remains. Abort remains available after completed
  replay steps until the final publication decision becomes durable. A decided
  final publication must roll forward with `--continue`.

Ctrl-C is forwarded only to the invocation's verified process group. Brit waits
up to five seconds for it to stop; it does not escalate to a force-kill. An
uncooperative group leaves an unknown outcome and blocks recovery until you stop
that group explicitly. The saved invocation `launch.json` receipt identifies its
leader PID (also its process group) and process start identity; inspect that receipt
when a diagnostic does not print the group. This also applies when a hook does not read its
stdin.

After interruption, inspect status and choose an explicit action. `--continue`
recovers a pinned decision or child proposal before further replay. It does not
silently consume a newly recovered conflict, empty or edit stop. Interrupted
skip/abort retain their original intent; continuing them does not restart replay.
A replaced lock, foreign HEAD/index/work state, damaged journal or changed
ownership refuses recovery rather than guessing.

Failed editors retain their output. Inspect or correct the saved file named by
status, then `--continue` explicitly accepts validated saved bytes. An interrupted
editor with an unknown outcome is never automatically relaunched. An unknown
pre-hook permits inspection and abort only. An unknown post-hook follows an
already published commit: explicit continuation acknowledges that uncertainty
without rerunning the hook. Known-not-started post-authoring notifications remain
queued durably and run once. Status distinguishes these cases from a clean stop.

The rebase journal lives under a 1 GiB scratch limit, and the limit decides only
whether work starts or continues. A rebase is admitted only when its journal fits
with a 5/4 safety factor: a cheap estimate refuses far-too-large repositories
before any object is read, and the exact records are checked once preparation
finishes (a refusal there cancels the preparation, so nothing is left behind).
Admission allows for one stop and a resolution as large as the largest replayed
step. Every later forward step (`--continue`, `--stage`, `--skip`, an edit) is
reserved for before it writes anything; a refused step names `--abort`. The limit
meters the journal only: retained copies are durable data under the Git directory
and are never metered, a `--skip`'s included.

Admission does not see everything a rebase will write, so an admitted rebase can
still be refused at a later step. Such a refusal comes before the step writes
anything and names `--abort`, which restores the original state exactly; nothing
strands. Three cases are known, each measured at a lowered limit:

- An `edit` stop. Admission runs before the sequence editor, so it counts no edit
  stop. Accepting an amend archives into the journal a receipt for every object of
  the stop's whole tree, so one edit-and-amend costs about the size of the tree's
  files, not of the edited file, and `--continue` after an amend of unchanged size
  can be refused. Scaled to the real limit that is an edited file of roughly 138
  to 166 MB in a tree holding little else; a repository whose tree approaches the
  limit cannot edit-and-amend under Brit at all.
- A conflict under `merge.conflictStyle` `diff3` or `zdiff3`. The conflicted file
  also holds the base version, which the estimate does not count. Where the base
  is much larger than both sides, the rebase is refused when the conflict is found
  or at the step after it.
- `--stage --expand-scope` of a large file reserves about half of what it then
  writes. At a tight limit it can fail part-way and leave a payload in the
  journal; `--status` does not mention it, and a retry is refused. `--abort`
  restores `HEAD` and keeps the file.

In the first two cases the work done at the stop (the amend, the resolution) is
then kept only in the retained copy `--abort` writes under
`.git/brit-rebase-retained/`. `git rebase` is the fallback for a rebase refused
this way.

`--abort` and cleanup are never refused by the limit, in any phase: the abort's
own decisions, its records and the retained copy it writes, and the cleanup of a
completed or aborted rebase are unmetered. Only real disk space bounds them. A
write that fails on a full disk removes its own partial file, and the command
says so; free space and run the same command again.

### What a rebase reads and keeps

A rebase's cost scales with its change, not with the repository's history. Its
*floor* is the merge base `p` and the upstream `U`: refs the user owns keep both
reachable (the original branch reaches `p`; the user's upstream ref, and once
published the new branch, reach `U`). Nothing below the floor is walked, verified
or copied.

- **Selection** finds `p` with the merge-base walk and the replayed commits by
  following single parents from the tip down to `p`; its cost is proportional to
  the range. Every refusal decided by the index and the trees (refStorage, branch
  occupancy, scope, collisions, external filters, gitlink changes) runs before any
  object is verified, and verification runs before the journal exists.
- **Verification** covers only what the replay reads or writes: the replayed
  commits, for each one the tree entries that differ from its parent's same-path
  entry and the parent trees that comparison reads, and the upstream's and the
  base's trees and blobs along the paths the rebase touches. One pass verifies each
  object once and shares one spool budget.
- **The journal** records the floor and, for each retained root, its *custody*:
  the commits above the floor and their trees' deltas over their parents, as above.
  A stop's tree is held whole. The journal lists no history. The preservation
  record (the original index file and every entry of it, which for a large index
  is most of a record) is written once, as a journal payload the first record
  names, and later records reference it instead of repeating it, so a record's
  size no longer scales with the index once per decision.
- **The retained copy** an abort, a skip or a cleanup writes holds: that custody
  union; the original and observed index files and the blobs they stage beyond
  their commit's tree; for each path the rebase touches, its before-image, its
  worktree file and up to three staged versions; the journal; and a manifest. Its
  manifest is version 2, which records the floor. The release proof that lets
  cleanup drop the `refs/brit-rebase-retain/` refs reads only the copy: it stops at
  floor commits, and takes a tree entry equal to its parent's same-path entry as
  inherited. The copy, and the proof's cost, scale with what the rebase introduced.
- **The worktree witness** that guards a rebase against foreign changes covers the
  touched paths' surroundings only: the files and links on their ancestors and
  beneath any touched directory. A conflict or edit stop watches every tracked path
  and its own scope. Untracked and ignored files elsewhere (build output,
  dependency trees, editor files) are never read and may change freely, as under
  Git.

An upstream named by a raw object id rather than a ref is protected by its
retention ref only while the rebase runs; after cleanup it is as reachable as the
user left it, as with `git rebase`.

Because an abort and a cleanup are never refused, the retained copy they write is
not projected or reserved for. It is bounded by the change: the objects above, the
journal (which the limit bounds), two index files and the touched paths' versions.
On the 64 MiB fixtures of `tests/cli-journey/tests/memory_replay.rs`, whose large
files are the change itself, it is still of the order of those files. After
admission only the metered journal can refuse a step (the cases listed above).
Those fixtures, which completed under the limit before the recovery rules were
added, complete again.

### Formats and older Brit releases

Every new retained copy is manifest version 2. This Brit reads version 1 copies
(Brit 0.2.6 to 0.3.1), restores from them and proves their whole closure as
before; an older Brit refuses a version 2 copy without touching it. A journal an
older Brit wrote records no floor: this Brit aborts it, continues it and cleans it
up under the rules that Brit wrote it with (whole-history closures and a
whole-worktree witness), writing its copies as version 2 with an empty floor, which
means the whole closure. An older Brit refuses a journal this Brit wrote, because
it does not know the floor field, and changes nothing.

A Brit command killed at any point (a `SIGKILL`, which cannot tear a record
Brit writes; a crash or power loss before a record's fsync can, and stock Git
staging after a killed `--abort` can strand the rebase too, see
[States this version can still strand](#states-this-version-can-still-strand)),
at the real limit or a lowered one, leaves a rebase that `--status` describes
with a cure, that `--abort` restores exactly
(unless its final decision was already durable, when `--continue` publishes it),
and that `--continue` carries on. A record the killed command had written but not
yet put in effect (an unrenamed `state.pending`, or a decision, proposal or
retained-copy manifest cut short) is discarded or written again by the next
command; `--status` says so. A preparation killed before it recorded anything has
nothing to continue: `--abort` removes it.

A rebase is refused in about a second, before anything is written, only when its
journal could not fit the limit: when the files it changes, kept several times in
the journal, or a very long replay approach 1 GiB. The size of the history no
longer matters. Use `git rebase` there.

Gitlinks (submodules) the rebase leaves unchanged are index-only: their ids are
never looked up as local objects and their directories, absent, empty or checked
out, are never read or written. A rebase that would change a gitlink, or an index
that stages one differently from `HEAD`, is still refused by name before anything
changes. Shallow repositories are still refused.

`brit rebase --retire` sets aside a rebase whose only remaining work is its
retained copy, which can take long on a large repository or wait for disk space:
a completed or aborted rebase whose cleanup is pending, or an abort that is
retaining before it changed anything live. It also accepts a skip retaining its
copy before the rebase took any decision (phase `retaining-skip` with no decision),
where likewise nothing live has changed, since the first live write of a rebase is
its first decision; no command sequence reaches that state today, because every
stop a skip starts from (`empty`, `authoring`) is itself published as a decision.
It deletes nothing and changes nothing
live: it renames the journal to `brit-rebase-kept-<nonce>` beside it, leaves the
`refs/brit-rebase-retain/<nonce>/*` references and any retained copy in place,
and prints each kept piece, its size and the command that removes it. It refuses
everything else, naming the command that applies: preparation and every phase
with live state to restore (`--abort`), a decision in flight, and an open
conflict-authoring session. A second run reports nothing to retire. Older Brit
releases do not treat `brit-rebase-kept-*` as a journal or a cleanup receipt.

A journal that does not open as this repository's own (the repository was moved
or copied mid-rebase, the journal's permissions changed, or its checksum is
corrupt) is touched by no Brit command. `--status` says what failed, warns that a
copied repository's journal may describe another repository's live rebase, and
names the directory, retention refs and lock files to deal with by hand; every
other Brit mutation refuses until that is done. When the repository was moved and
its old location is gone, `--status` names that location: move the repository
back and run `brit rebase --abort` there, which restores it exactly. The manual
steps remain the alternative, but they restore nothing and leave a checkout the
rebase had begun changing part-way through it.

Publication and cleanup are separate. A message saying **committed/published with
warning** or **cleanup pending** must not be interpreted as an unpublished
failure. Retained copies survive cleanup. Unknown private temporary files can
remain for inspection and prevent cleanup even when abort has restored the
original state. Do not delete an active journal or its retention refs to bypass
a refusal, and do not use `git rebase --continue/--abort` to control Brit's state.

## States this version can still strand

The first two were produced by the lane's reviewer by killing a command at the
nearest checkpoint and cutting the named record to half its length. A `SIGKILL`
cannot realistically tear these records; a crash or power loss before the
record's fsync can. The third tears no record: it follows an ordinary `SIGKILL`.

- A torn `metadata-lock-N.json` in an abort transition or in the forward first
  checkout, and a torn `retention-N.json`, leave `--abort`, `--continue`, `--skip`
  and `--retire` all refusing, and `--status` then names a cure that does not work.
- A torn cleanup receipt, left by an older Brit or by a power loss, leaves a
  published rebase whose `--continue` refuses and which blocks every mutation.
- After an `--abort` is killed mid-copy, a stock `git add` of an unrelated dirty
  file, even when undone with `git reset -- <file>`, makes `--abort` refuse
  "foreign replay metadata before abort" and `--retire` refuse, and `--status`
  names no cure. At an ordinary conflict stop the same undo lets `--abort` work.

A torn `index.pre-rebase` and a torn `proposal-N.json` are cleared by `--abort`.

Not produced by anyone: a torn `directory-*.json` or `payload-license-*.json`, or a
leftover `*.new` of the authoring child.

Established by reading: nothing is written between "pending synced" and
"renamed" (the write-order table in `gitoxide-core/src/repository/replay/journal.rs`).
The window lies entirely inside the journal's `save` and `save_bytes`;
`publish_state`, which performs the rename, has exactly those two callers. Between
the sync and the rename the only code is the `replay-state-file-synced`
checkpoint, which exists in every build: there it only checks for an interrupt
and may return an error, and writes nothing (a fault-injection build also writes
a marker file outside the repository). `state.pending` is renamed onto
`state.json` in three other places, `recover_preparation`, `cancel_preparation`
and the legacy-payload upgrade's roll-forward, each of which only reads or syncs
between finding the record and renaming it.

## Inspect and extract a retained copy

Use the exact copy leaf printed by the operation, usually beneath the owning
private Git directory at `brit-rebase-retained/<operation-nonce>/<sequence>/`.
Inspection works after the active journal has been removed and does not require
repository discovery or a runtime Git executable.

```sh
brit-rebase-retained inspect --copy '/absolute/printed/copy-leaf'
mkdir -m 700 '/absolute/new-empty-export'
brit-rebase-retained extract --copy '/absolute/printed/copy-leaf' \
  --to '/absolute/new-empty-export'
```

The helper validates ownership, schema, completeness and checksums. The
extraction destination must already exist, be empty and safely owned, with no
symlink ancestors. Corrupt copies, unsafe paths and destination replacement
refuse extraction. Interrupted extraction may leave partial output; inspect it
and use a different empty destination for a fresh export.

`manifest.json` maps entries to numeric payload names under `original/`,
`before/`, `staged/`, `worktree/`, `after/`, `messages/`, `invocations/`, `objects/`
and `indexes/`. Git path bytes, stages, modes, object identities and deletion
records remain metadata. Non-UTF-8 names are preserved as byte arrays; inspection
also supplies `copyPathBytes` for a non-UTF-8 enclosing path. Symlinks are exported
as ordinary `.symlink-payload` files containing their target bytes, never as live
links. Directory identities are metadata, not recreated inode identities. Export
is for inspection and deliberate recovery; it does not install files into a
working tree or automatically apply a saved index.

Copies never expire automatically. After verifying the export and deciding the
copy is no longer needed, delete **only its exact leaf**, for example:

```sh
rm -r -- '/absolute/printed/copy-leaf'
```

Do not substitute the operation directory, the entire retained-copy store or the
active journal for that leaf. Extraction itself never deletes the original copy.

## Install the separate Linux helper

Linux archives may include `brit-rebase-retained` alongside `brit`. The existing
`scripts/ci/install-brit-release.sh` installs only `brit`; it remains compatible
with older archives. Install the helper separately from the same accepted,
checksummed archive. The archive's `BUILD-INFO.txt` records
`retained_helper_binary`, `retained_helper_sha256` and `cargo_helper_bin`.
An older archive without those fields does not supply the helper.

The following Bash example uses fresh directories and independently accepted
archive/source pins. Replace every example value before running it. Use the same
accepted source revision when installing the main Brit executable.

```bash
set -euo pipefail
archive='/absolute/brit-max-pure-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz'
archive_sha='ACCEPTED_64_CHARACTER_ARCHIVE_SHA256'
source_pin='ACCEPTED_40_CHARACTER_SOURCE_COMMIT'
unpack='/absolute/new-archive-directory'
helper_prefix='/absolute/new-helper-install'
[[ "$archive_sha" =~ ^[0-9a-f]{64}$ ]]
[[ "$source_pin" =~ ^[0-9a-f]{40}$ ]]
printf '%s  %s\n' "$archive_sha" "$archive" | sha256sum --check -
mkdir -m 700 "$unpack"
tar -xzf "$archive" -C "$unpack"
package="$unpack/$(basename "$archive" .tar.gz)"
receipt="$package/BUILD-INFO.txt"
test -f "$receipt" && test ! -L "$receipt"
field() {
  awk -F= -v key="$1" '$1 == key { count++; value=$2 }
    END { if (count != 1) exit 2; print value }' "$receipt"
}
test "$(field source_commit)" = "$source_pin"
test "$(field target)" = x86_64-unknown-linux-gnu
helper_name=$(field retained_helper_binary)
helper_sha=$(field retained_helper_sha256)
test "$helper_name" = brit-rebase-retained
[[ "$helper_sha" =~ ^[0-9a-f]{64}$ ]]
test -f "$package/$helper_name" && test ! -L "$package/$helper_name"
test -x "$package/$helper_name"
test "$(sha256sum "$package/$helper_name" | awk '{print $1}')" = "$helper_sha"
mkdir -m 700 "$helper_prefix"
mkdir -m 700 "$helper_prefix/bin"
cp "$package/$helper_name" "$helper_prefix/bin/$helper_name"
test "$(sha256sum "$helper_prefix/bin/$helper_name" | awk '{print $1}')" = "$helper_sha"
"$helper_prefix/bin/$helper_name" --help
```

Invoke that absolute helper path or add its `bin` directory to your PATH. For
installed-artifact journey tests, set `BRIT_BIN` to the installed Brit executable
and `BRIT_REBASE_RETAINED_BIN` to this verified helper, then select
`cargo test --locked -p cli-journey --test rebase` with the usual external native
Cargo target and toolchain settings. This is the separate-helper installation
path exercised by the Linux release selector. It does not claim atomic paired
upgrades or rollback of Brit and the helper.

Execution and extraction currently refuse on unsupported platforms. Recovery
qualification is limited to Linux fsync/rename behavior, real process signals
and loss-of-unsynchronized-writes models. It does not establish physical
power-cut behavior or isolation from a hostile concurrent filesystem writer.
