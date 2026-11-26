# Brit 0.3.8

Brit 0.3.8 is sprint 6's release ([plan](../plans/2026-11-25-brit-sprint-6.md)), together
with backlog batch 1. It keeps reads working as packs pile up, closes more of the daily
Git fallbacks, makes fetch honest about what it changed, and ships the two helper binaries
the installed release was missing. The CLI versions 0.3.6 and 0.3.7 went to the registry from
`main` without a tag; this release covers them.

## What 0.3.8 contains

- **Pack maintenance.** `brit maintenance` consolidates packs under the reduced-trust cap,
  and `fetch` does the same once more than 24 packs exist, doing bounded work each time. It
  keeps every object that another worktree's HEAD or index, a submodule or an
  `objects/info/alternates` borrower may need, and a kill at any point leaves every object
  readable. While a `push` is running, it holds the object store, and consolidation waits
  until the push finishes. A reader that has a pack mapped keeps reading it after maintenance
  retires it, and refreshes for objects it has not read yet.
- **`brit restore <path>`** restores one tracked path from the index, from `--source <rev>`,
  or with `--staged`, in the main checkout or a linked worktree. The result matches
  `git restore` byte for byte. A path that doesn't exist refuses and changes nothing.
  Directories and pathspec patterns are not supported yet.
- **`brit add -A` with submodules.** It stages every other change and records each
  registered submodule's checked-out commit as its gitlink. An unregistered nested
  repository stays unstaged, with one warning line on stderr.
- **Fetch says what happened.** A named `fetch origin <branch>` never reports success while
  leaving that ref unfetched. A remote lightweight tag that peels to the same commit as a
  local annotated tag is up to date rather than a rejected update. A tag that would move to
  a different commit is still refused and listed.
- **The attribution warning names its epr.** `Brit attribution UNMEASURED` names the `epr` it
  asked, by path and version, and says when a newer one is installed on the device.
- **A feedback-capture gap is reported once per invocation** when `EPR_FEEDBACK_INVOCATION`
  declares one, and otherwise once per session.
- **Ref-lock contention is a bounded refusal.** When another process holds a ref lock,
  brit-epr refuses with a `Concurrent` error after a bounded wait instead of failing at
  random.
- **The release ships `brit-build-ref` and `brit-verify`.** Both are built, archived and
  installed next to `brit`, and each one's sha256 is recorded in `BUILD-INFO.txt`.
- **Help text** describes `pull --ff-only` and moving a single notes ref with `fetch` or
  `push`.
- **CI.** A narrow run builds only the packages that own the selected tests and can repeat a
  test N times. A repeat that selects no tests fails. Selecting the CLI journey tests also
  builds `brit-cli`.
- **Housekeeping.** The stale `vendor/git` submodule declaration is gone. CI checks out
  git/git itself at the version `docs/parity/git-reference.json` names.

## Known limits

- **Quarantined tests.** Three upstream fsmonitor tests and the Windows `gix-notify`
  replacement test are excluded from brit's CI with their diagnosed causes
  (`tests/it/src/qa/native_tests.rs`, `QUARANTINED`). The fsmonitor fixes are on
  `sprint6/fsmonitor-race`, waiting to go upstream to gitoxide. Each entry is dropped once
  an upstream merge brings its fix in.
- **Windows retirement path untested.** On Windows, retiring a pack that a reader still
  has mapped has no test yet.
- **No small object store yet.** Sprint 6 item 6 (blobless clones for thin clients) has not
  started.

## Allocated versions

| Crate | 0.3.5 release | 0.3.8 release |
| --- | --- | --- |
| `brit-cli` | 0.3.5 | 0.3.8 |
| `brit-bridge` | 0.1.9 | 0.1.10 |
| `brit-build-ref` | 0.1.10 | 0.1.12 |
| `brit-epr` | 0.2.4 | 0.2.5 |
| `brit-graph` | 0.2.6 | 0.2.8 |
| `gitoxide` | 0.61.5 | 0.61.8 |
| `gitoxide-core` | 0.64.5 | 0.64.8 |
| `gix-fsmonitor-daemon` | 0.1.0 | 0.1.1 |
| `gix-protocol` | 0.68.2 | 0.68.3 |
| `gix-submodule` | 0.36.6 | 0.36.7 |
| `gix-tix` | 0.6.3 | 0.6.4 |

Each crate in the table declares `rust-version = "1.100"`.
