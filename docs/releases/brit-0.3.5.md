# Brit 0.3.5

Brit 0.3.5 is sprint 5's release
([plan](../plans/2026-11-25-brit-sprint-5.md)). It removes the Git fallbacks that elohim's
landings still needed, adds read forms for tracing changes, makes scripted output clean, and
adds narrow CI runs. Each crate it changes now declares Rust 1.100, the pinned toolchain.

## What 0.3.5 contains

- **`brit push` completes a gated `dev` landing.** `push origin HEAD:<ref>` and
  `push origin <sha>:<ref>` both work. A hooks path given per invocation
  (`brit -c core.hooksPath=<dir> push …`) runs that directory's hook. A hooks path that is
  missing or not executable refuses, and nothing is sent. The hook's environment reaches it
  unchanged, and its output streams live. The object the hook judged is the object sent. A
  remote that moves during the hook is refused. Journey tests state the contract landing
  agents rely on: the hook's parent is the pushing `brit`, `BRIT_GUARD_HELD_BY` reads
  `v1 <pid> <start ticks> …` for that process, and that process sends and outlives the remote
  ref's move.
- **`brit cherry-pick <commit>`** applies one commit with rebase's replay engine. It keeps
  author, date, message and every trailer byte for byte, handles a commit that only moves a
  submodule pin, names conflicting paths and changes nothing on a conflict. It runs natively
  on Linux. Elsewhere it refuses and names the `git cherry-pick` command.
- **`brit merge --ff-only <ref>`**, and `brit pull --ff-only` across a submodule pin move.
  The new pin goes into the index, and the submodule's checkout is left alone. Unrelated dirty
  files survive; a dirty path the fast-forward would change refuses.
- **Notes.** `brit notes show` is native. `fetch` and `push` move one named
  `refs/notes/X:refs/notes/X` ref, fast-forward only, and refuse wildcard notes refspecs.
  Other `notes` verbs remain Git bridges.
- **`brit add --gitlink <commit> <path>`** stages a submodule pin without a checked-out
  submodule.
- **`brit grep <pattern> <rev>`** and **`brit log -S <string>`** match Git's output byte for
  byte.
- **Diagnostics leave stdout.** Piped `rev-parse` and `status` print only the answer. The
  feedback-capture notice prints once per session and names its cause. `--quiet` and
  `BRIT_QUIET=1` silence it.
- **Concurrent build-ref writes on macOS.** Scratch notes refs keep no reflog, so concurrent
  writers no longer race on a shared log directory (macOS reported `EINVAL`).
- **Narrow CI runs.** A manual `ci` run takes `mode`, `platforms`, `stage` and `tests` (a
  nextest filter); `scripts/ci/narrow.sh` starts one. A narrowed run reports
  `Narrow checks pass (not a landing proof)`, never `Tests pass`. Native test builds name
  workspace packages unambiguously.

## Allocated versions

| Crate | Previous | Release | Reason |
| --- | --- | --- | --- |
| `brit-cli` | 0.3.4 | 0.3.5 | source |
| `brit-epr` | 0.2.3 | 0.2.4 | source |
| `gitoxide` | 0.61.4 | 0.61.5 | source |
| `gitoxide-core` | 0.64.4 | 0.64.5 | source |
| `gix` | 0.90.3 | 0.90.4 | source |
| `gix-refspec` | 0.47.4 | 0.47.5 | source |

Each crate in the table declares `rust-version = "1.100"`. `gix` carries the workspace's MSRV
check, so the MSRV badge reads 1.100.0.
