---
title: Safe staged authoring prerequisite
status: completed
date: 2026-09-28
habits:
  - dev-system-equilibrium
---

# Safe staged authoring prerequisite

Bounded sprint under station 4 of the native developer through-line. Baseline
`1017e9464`: ordinary commits refuse; `--allow-empty` copies HEAD's tree rather
than the index; commit hooks do not execute; push accepts but ignores
`--no-verify`. The existing command inventory does not prove authoring readiness.

## Acceptance story and tasks

1. In disposable repositories, a developer commits the index through the public
   Brit binary. Stock Git sees exactly the staged tree, including deletions,
   executable files, symlinks and gitlinks. Unstaged bytes remain untouched.
   Initial commits work; empty and conflicted commits refuse appropriately.
2. Pre-commit, prepare-commit-msg and commit-msg run in order with the resolved
   worktree/index and `core.hooksPath`; rejection leaves HEAD unchanged. Hook
   message edits are consumed. Post-commit runs after success. An explicit
   `--no-verify` skips only pre-commit and commit-msg. Linked worktrees remain
   isolated. Unsupported scoped/pathspec authoring refuses instead of silently
   including unrelated staged files.
3. Until transport exposes its exact resolved updates to the CLI, a configured
   executable pre-push hook causes Brit push to refuse before connecting, naming
   stock Git as the working gate-preserving bridge. Explicit `--no-verify`
   remains an intentional bypass, never the default. This is a safety boundary,
   not a claim that Brit runs pre-push.
4. Wire the disposable authoring journey into existing Brit CI, run focused
   locked tests and feature checks, and record measured evidence here and in the
   parent habit. Keep source edits out of upstream-owned `gix-*` crates.

No new protocol entity, signer, attestation or authority is introduced. Git
objects remain the bridge. Durable proof binding, scoped index transactions,
independent acceptance and native publication remain subsequent station-4 work.
No installation, PATH replacement, publication or parent pin change is part of
this sprint. Preserve the pre-existing through-line plan edits and draft.

Verification: build `brit-cli --bin brit`; run `cli-journey --test authoring`
against that exact binary, plus existing Brit and shared-tree journeys; check
`gitoxide-core` and public CLI feature configurations. Use native empty
RUSTFLAGS, the Cargo pool, one compiler worker and isolated fixture Git helpers.

## Discovered stations

Chain: native developer through-line / between station 3 shared tree → station 4
accountable work / missing node: the public executable commits the persisted
index and refuses skipped repository gates / probe: `cli-journey --test authoring`
/ current state: local green, 19 tests; upstream CI unrun for these changes.

The historical parity inventory conflated accepted flags with operational
semantics. Six initial acceptance tests failed on the baseline executable.
The regression skeleton, implemented in the existing Rust CLI journey, is:

```gherkin
@regression
Scenario: An empty-allowed commit still carries staged changes
  Given staged content differs from HEAD and the worktree has later edits
  When the developer commits with --allow-empty
  Then the commit tree equals Git's write-tree result
  And the unstaged bytes and persisted index remain unchanged
```

Hook execution has its own boundary: a zero-argument hook in a directory with
spaces must remain one executable path. A shell fallback initially broke five
tests because the existing command helper quotes paths only when arguments are
present. Passing the path through argv to a constant shell program fixes that
without modifying upstream-owned code. The regression fixtures also exercise
message arguments and shebang-less scripts. Git byte names are tested directly
in the index; host filename support is not a prerequisite.

Next missing node: between local hook protection → native publication / assertion:
pre-push sees the exact resolved local/remote refs and old/new OIDs that the
transport will send, and a nonzero hook exit prevents every remote update /
probe: a disposable bare remote with create, update, delete and rejection cases /
current state: unwired; the CLI explicitly refuses an executable pre-push hook.
Do not close this node with an empty hook stdin or a second guessed refspec
resolver. Scoped temporary-index authoring and signed, target-bound acceptance
remain separate subsequent stations; no full cutover is claimed.

## Local completion evidence

Completed the bounded local sprint on Linux using Rust toolchain `1.98`, one
compiler worker, empty RUSTFLAGS and the registered external Cargo pool slot.
The public executable built from `1017e9464` plus this working diff has SHA-256
`9af5c78b7f59172f94e2d209db5b506cbcd507183d458f51ee20f8c701accc36`.

| Probe | Result |
|---|---|
| `cargo test --locked -p cli-journey --test authoring` with explicit built `BRIT_BIN` | 19/19 passed, no skips |
| Existing `--test brit --test shared_tree` journeys | 16 + 2 passed |
| `cargo test --locked -p brit-cli -p gitoxide-core` | 21 passed |
| `cargo clippy --locked -p brit-cli -p gitoxide-core -p cli-journey --all-targets -- -D warnings` | Passed |
| Public CLI `cargo check --locked --no-default-features` for `small`, `lean-async`, `max-pure` | All passed |
| Scoped rustfmt, `git diff --check`, `epr check` | Passed |
| Parent habits projection `--check` | Passed after DELTA |

Lint boundary: the workspace retains a removed-Clippy-lint advisory. The narrower
`cargo clippy -p cli-journey --test authoring -- -D warnings` configuration also
fails on an existing `collapsible_if` in upstream-owned
`gix-tempfile/src/registry.rs:36`; the combined invocation above is green.
No upstream source was changed or lint suppressed to conceal that result.

The existing CI journey job now runs `authoring` against its freshly built Brit
executable. These edits remain uncommitted and unpushed; no new remote CI,
registry, release or installation result is asserted. The parent pin and the
pre-existing through-line plan edits/draft are preserved. Parent habit remains
RED. Source inspection used direct shell reads outside the private recall
session; that unmetered inspection reconciled no memory edges.
