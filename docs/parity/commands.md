# Parity Matrix — git ↔ gix

Top-level command-level status. Per-flag status lives inside each `tests/journey/parity/<cmd>.sh`. See root `CLAUDE.md` for loop structure.

Legend:
- `absent` — no gix implementation
- `partial` — some flags / modes implemented, not all; has a `tests/journey/parity/<cmd>.sh` with TODO rows
- `present` — full parity claimed AND verified by `@gix-steward`
- `deferred` — hard system constraint only; operator-approved

## Porcelain

| git | gix | Status | Parity file | Notes |
|---|---|---|---|---|
| push | gix push | present | [push.sh](../../tests/journey/parity/push.sh) | happy-path + error-path parity across the full documented flag surface (68 green `it` blocks across 48 sections); sha256 remotes, actual lease-mismatch / force-if-includes enforcement, non-default receive-pack programs, actual push-option transmission, push-cert generation, submodule-push, and live-TCP `-4`/`-6` family selection documented in the push.sh header as deferred follow-ups |
| pull | gix pull | absent | — | composes fetch + merge |
| fetch | gix fetch | partial | [fetch.sh](../../tests/journey/parity/fetch.sh) | scaffolded; all rows TODO. `gix fetch` exists but its Clap surface is gix-native, not git-compatible; iterations close rows as the git-fetch flags land |
| clone | gix clone | partial | — | exists as `Clone` subcommand; flag coverage unverified |
| merge | gix merge | partial | — | exists; CLI surface incomplete |
| rebase | — | absent | — | stub crate `gix-rebase` exists |
| reset | — | absent | — | — |
| commit | — | partial | — | `Commit` subcommand exists; hook support missing |
| status | gix status | partial | — | exists; flag coverage unverified |
| log | gix log | partial | — | exists; flag coverage unverified |
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
