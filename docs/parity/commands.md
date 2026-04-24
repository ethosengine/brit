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
| log | gix log | present | [log.sh](../../tests/journey/parity/log.sh) | 220 green `it` blocks covering the full user-visible git-log flag surface (rev-list-options + diff-options + pretty-options + git-log proper, minus rev-list-only `ifdef::git-rev-list[]` entries and `--max-depth` / `--maximal-only` which git-log itself rejects at runtime despite their unconditional adoc entries). Real implementations: revspec parsing (Include/Range/Merge/IncludeOnlyParents/etc via gix_revision::Spec), pseudo-refs (--all/--branches/--tags/--remotes), parent-count predicate (--merges / --no-merges / --min-parents / --max-parents), iterator adapters (-n/--max-count/--skip/-N), log-specific exit-code remaps (--bogus-flag→128, unborn HEAD → byte-exact git wording + 128, ambiguous-arg → byte-exact 3-line stanza + 128). Remaining flags are clap-accepted and tracked in [SHORTCOMINGS.md](SHORTCOMINGS.md) as compat rows — pretty/format, decorate, diff output, date formatting, pickaxe (-G/-S), cherry/left-right, reflog walk (-g), history simplification, whitespace-diff, diff algorithms (--minimal/--patience/--histogram/--anchored), diff-filter, word-diff, color-moved/color-words, prefix/output, rev-list companions (--reflog/--stdin/--ignore-missing/--since-as-filter/--bisect/--relative-date/--exclude-hidden), combined-diff (--dd/--combined-all-paths/--output-indicator-*/-t), rename/copy (-B/-C/-l/--no-renames/--rename-empty), path control (-O/-R/--skip-to/--rotate-to/--ignore-submodules/--default-prefix/--line-prefix/--ita-invisible-in-index), deprecated notes aliases (--show-notes/--standard-notes), pretty companions (--encoding/--expand-tabs), `-P` / `--basic-regexp` regex modes. One legitimate shortcoming: --merge (git exits 128 without merge state; gix has no precondition check). sha256 blocker: gix-config rejects `extensions.objectFormat=sha256` (`gix/src/config/tree/sections/extensions.rs`), so every repo-opening row is `sha1-only`. Dual rows: `--help`, `(outside a repository)`. |
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
| tag | gix tag | partial | [tag.sh](../../tests/journey/parity/tag.sh) | scaffold only: 37 TODO rows across list/delete/verify/create modes; current gix surface is `Tag(tag::Platform)` → `Subcommands::List` (no flags). Every flag from `vendor/git/builtin/tag.c`'s `options[]` array (~25 entries, listing + creation groups) is represented as a placeholder `it` block; first closeable row is `--help` (already works — no wiring needed). |
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
| cat-file | gix cat | present | [cat-file.sh](../../tests/journey/parity/cat-file.sh) | 59 green `it` sections / 72 `it` blocks across the full git-cat-file(1) flag surface: query modes (-e/-p/-t/-s), `<type> <object>` positional + type-mismatch, mailmap (compat + `--use-mailmap -s` bytes-parity via Commit/Tag signature rewrite through gix_mailmap::Snapshot::resolve_cow), textconv/filters/--path, batch family (--batch / --batch-check / --batch-command + format-atom expansion with --batch-all-objects / --buffer / --unordered / --follow-symlinks / -Z / -z), format atoms (%objectname, %objecttype, %objectsize, %objectsize:disk, %deltabase, %rest; %objectmode emits bad-format 128 in byte-parity with system git 2.47.3), combined-mode + mode+batch usage errors (129), and --allow-unknown-type. Clap surface on `Subcommands::Cat` carries `visible_alias = "cat-file"` so `gix cat-file …` routes to the existing `gix cat` canonical. --follow-symlinks uses a custom tree-walker with dir-stack + ..-escape detection for Resolved / External / Dangling status lines. Deferrals (effect-mode compat, not shortcomings): the 7 `--filter=<spec>` rows because system git 2.47.3 predates cat-file's OPT_PARSE_LIST_OBJECTS_FILTER — rows flip to bytes when the CI git catches up; the ambiguous short-sha row because git's stderr hint text varies across versions. sha1-only: `gix-config` rejects `extensions.objectFormat=sha256`; dual-hash rows (--help, --bogus-flag outside repo, outside-of-repo) pass on both. |
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
