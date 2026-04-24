# Parity Matrix — git ↔ gix

Top-level command-level status. Per-flag status lives inside each `tests/journey/parity/<cmd>.sh`. See root `CLAUDE.md` for loop structure.

Legend:
- `absent` — no gix implementation
- `partial` — some flags / modes implemented, not all; has a `tests/journey/parity/<cmd>.sh` with TODO rows
- `present` — full parity claimed AND verified by `@gix-steward`
- `deferred` — hard system constraint only; operator-approved

Deferred flag-level rows and compat-only rows live in [SHORTCOMINGS.md](SHORTCOMINGS.md), regenerated from `tests/journey/parity/*.sh` by `etc/parity/shortcomings.sh`.

## Porcelain

| git | gix | Status | Parity file | Notes |
|---|---|---|---|---|
| push | gix push | present | [push.sh](../../tests/journey/parity/push.sh) | 68 green `it` blocks across 48 sections. Deferrals: sha256 remotes, actual lease-mismatch / force-if-includes enforcement, non-default receive-pack programs, actual push-option transmission, push-cert generation, submodule-push, and live-TCP `-4`/`-6` family selection — documented as prose in `tests/journey/parity/push.sh` header; ledger retrofit via `compat_effect`/`shortcoming` markers pending. |
| pull | gix pull | absent | — | composes fetch + merge |
| fetch | gix fetch | present | [fetch.sh](../../tests/journey/parity/fetch.sh) | 89 green `it` blocks across 61 sections. Deferrals: see [SHORTCOMINGS.md#fetch](SHORTCOMINGS.md#fetch). sha256 blocker: gix cannot open sha256 remotes yet (every row `sha1-only`). |
| clone | gix clone | present | [clone.sh](../../tests/journey/parity/clone.sh) | 1 top-level section header + 48 flag-scoped title sections; expect_parity + expect_run assertions, sha256 skipped via only_for_hash. Deferrals: see [SHORTCOMINGS.md#clone](SHORTCOMINGS.md#clone). sha256 blocker: gix cannot open sha256 remotes yet (every row `sha1-only`). |
| merge | gix merge | partial | — | exists; CLI surface incomplete |
| rebase | — | absent | — | stub crate `gix-rebase` exists |
| reset | — | absent | — | — |
| commit | — | partial | — | `Commit` subcommand exists; hook support missing |
| status | gix status | present | [status.sh](../../tests/journey/parity/status.sh) | 51 green `it` blocks across 26 sections (short/long, branch/show-stash, porcelain=v1/v2, verbosity, untracked-files modes, ignore-submodules modes, ignored modes, -z, column, ahead-behind, renames, find-renames, pathspec). Rows flip to `sha1-only` because gix-config rejects `extensions.objectFormat=sha256` (`gix/src/config/tree/sections/extensions.rs`). Deferrals (compat-accept rows: effect-mode parity holds, byte-output pending): `--show-stash`, `-v/--verbose`, `--ignore-submodules=<mode>`, `--column/--no-column`, `--ahead-behind/--no-ahead-behind`, `--renames/--no-renames`, `--find-renames[=<n>]`, porcelain=v2 headers for branch/stash/detached-HEAD/initial-repo — documented as prose in `tests/journey/parity/status.sh` headers; ledger retrofit via `compat_effect` markers pending. |
| log | gix log | present | [log.sh](../../tests/journey/parity/log.sh) | 185 green `it` blocks across 185 sections covering the full user-visible git-log flag surface (rev-list-options + diff-options + pretty-options + git-log proper, minus rev-list-only `ifdef::git-rev-list[]` entries). Real implementations: revspec parsing (Include/Range/Merge/IncludeOnlyParents/etc via gix_revision::Spec), pseudo-refs (--all/--branches/--tags/--remotes), parent-count predicate (--merges / --no-merges / --min-parents / --max-parents), iterator adapters (-n/--max-count/--skip/-N), log-specific exit-code remaps (--bogus-flag→128, unborn HEAD → byte-exact git wording + 128, ambiguous-arg → byte-exact 3-line stanza + 128). Remaining flags are clap-accepted and tracked in [SHORTCOMINGS.md](SHORTCOMINGS.md) as compat rows — pretty/format, decorate, diff output, date formatting, pickaxe (-G/-S), cherry/left-right, reflog walk (-g), history simplification, whitespace-diff, diff algorithms (--minimal/--patience/--histogram), diff-filter, word-diff, color-moved, prefix/output, rev-list companions (--reflog/--stdin/--ignore-missing/--since-as-filter), pretty companions (--encoding/--expand-tabs). One legitimate shortcoming: --merge (git exits 128 without merge state; gix has no precondition check). sha256 blocker: gix-config rejects `extensions.objectFormat=sha256` (`gix/src/config/tree/sections/extensions.rs`), so every repo-opening row is `sha1-only`. Dual rows: `--help`, `(outside a repository)`. |
| diff | gix diff | partial | — | exists; flag coverage unverified |
| show | — | absent | — | — |
| blame | — | partial | — | `Blame` subcommand exists (plumbing) |
| add | — | absent | — | — |
| rm | — | absent | — | — |
| mv | — | absent | — | — |
| checkout | — | absent | — | — |
| switch | — | absent | — | — |
| restore | — | absent | — | — |
| branch | gix branch | partial | — | exists as `Branch` subcommand |
| tag | gix tag | partial | — | exists as `Tag` subcommand |
| stash | — | absent | — | — |
| cherry-pick | — | absent | — | — |
| revert | — | absent | — | — |
| bisect | — | absent | — | — |
| notes | — | absent | — | stub crate `gix-note` exists |
| worktree | gix worktree | partial | — | exists as `Worktree` subcommand |
| submodule | gix submodule | partial | — | exists as `Submodule` subcommand |
| remote | gix remote | partial | — | exists as `Remote` subcommand |
| config | gix config | partial | — | exists as `Config` subcommand |

## Plumbing (selected, not exhaustive)

| git | gix | Status | Parity file | Notes |
|---|---|---|---|---|
| cat-file | gix cat | partial | — | — |
| rev-parse | gix revision | partial | — | — |
| ls-files | — | absent | — | — |
| ls-tree | gix tree | partial | — | — |
| verify-pack | gix free pack verify | partial | — | — |
| pack-objects | — | absent | — | — |
| pack-refs | — | absent | — | — |
| update-ref | — | absent | — | — |
| symbolic-ref | — | absent | — | — |
| hash-object | — | absent | — | — |
| mktree | — | absent | — | — |
| write-tree | — | absent | — | — |
| commit-tree | — | absent | — | — |
| read-tree | — | absent | — | — |

(This table is a seed. Complete enumeration via `etc/parity/enumerate.sh` when we're ready to scale.)

## ein — brit-native workflow tool (no git parity target)

`ein` commands are brit-native; they have no git counterpart and are **out of parity-loop scope**.

| ein command | Purpose |
|---|---|
| ein init | init repository |
| ein tool organize | consolidate git repos into a tidy directory tree |
| ein tool find | locate git repos recursively |
| ein tool estimate-hours | estimate contributor time from commit history |
| ein tool query | object-graph queries |
