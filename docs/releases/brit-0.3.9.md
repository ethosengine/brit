# Brit 0.3.9

Brit 0.3.9 is backlog batch 2. It closes the fallbacks that peer sessions reported most
often, because each one costs a permission prompt in every session. It also fixes the macOS
installer, whose failure stopped the `brit-v0.3.8` release before anything was published.
That tag has no release assets. 0.3.9 carries everything in
[0.3.8](brit-0.3.8.md) as well.

## What 0.3.9 contains

- **`brit cherry-pick` takes ranges and lists.** `cherry-pick A..B` applies the commits oldest
  first. `cherry-pick c1 c2 c3` applies them in the order given. Both run as one replay that
  keeps author, date, message and trailers byte for byte. An empty range or a merge commit
  refuses before anything is applied. On a conflict, brit names the commit and the paths,
  leaves the conflict stages in the index and keeps the picks already made. After `brit add`,
  `--continue` carries on. `--skip` drops the conflicting commit. `--abort` restores HEAD, the
  index and the worktree exactly. `--status` reports where the sequence stands. These forms run
  natively on Linux. Other hosts refuse and name the `git cherry-pick` command, as the
  single-commit form does.
- **Status output for scripts.** `status --porcelain` (v1) and `--porcelain=v2`
  (`-f porcelain-v2`), with `-z` and the `--branch` headers, match Git byte for byte. This
  covers renames, unmerged paths and submodules.
- **`log --format=<fmt>` and `--pretty=format:<fmt>`.** They cover the common placeholders
  (`%H %h %s %b %B %an %ae %ad %aI %at %cn %ce %cI %ct %P %p %d %D %n %%`) and the
  `--date` styles `iso`, `iso-strict`, `unix` and `short`. An unsupported placeholder refuses
  and names itself. `log --stat` matches Git's stat block.
- **`show :<n>:<path>`** prints the blob staged at stage n. A path containing `..` is read as
  a path. A missing stage names the path and the stage.
- **`ls-tree`** (with `-r`, `--name-only`, `-z`, path filters and gitlinks) and **`cat-file`**
  (with `-p`, `-t`, `-s`, `-e`) match Git, including exit codes.
- **`diff` on a submodule path** shows the gitlink change (`Subproject commit …`, with
  `-dirty`) in both the worktree and revision forms, and with `--stat`. It used to refuse with
  "Directory staging is not supported".
- **`worktree add` waits for the repository guard.** On Linux, `worktree add` and the other
  writers that take only the repository guard wait up to 10 seconds. They name the holder when
  they start waiting, and refuse naming it if the guard is still held at the limit.
- **The installer migrates a legacy link through a symbolic parent.** On macOS every
  temporary path lives under `/var`, which is a link to `/private/var`. The installer now
  compares the release directory physically.

## Allocated versions

| Crate | 0.3.8 release | 0.3.9 release |
| --- | --- | --- |
| `brit-cli` | 0.3.8 | 0.3.9 |
| `gitoxide` | 0.61.8 | 0.61.9 |
| `gitoxide-core` | 0.64.8 | 0.64.9 |

Each crate in the table declares `rust-version = "1.100"`.
