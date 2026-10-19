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
`GIT_EDITOR` can change the pinned configuration and is refused.

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

Publication and cleanup are separate. A message saying **committed/published with
warning** or **cleanup pending** must not be interpreted as an unpublished
failure. Retained copies survive cleanup. Unknown private temporary files can
remain for inspection and prevent cleanup even when abort has restored the
original state. Do not delete an active journal or its retention refs to bypass
a refusal, and do not use `git rebase --continue/--abort` to control Brit's state.

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
