---
epr-habit-version: 1
id: brit-inherited-ci
invariant: >
  Brit integration preserves inherited gix checks and qualifies only exact-revision
  required job and step evidence, with matrix exclusions visible as debt.
status: red
active: false
refs:
  - docs/plans/2026-09-28-inherited-ci.md
checks:
  - "python3 scripts/ci/test_qualify_readiness.py"
  - 'python3 scripts/ci/qualify_readiness.py --milestone inherited-ci --sha "${BRIT_READINESS_SHA:?set full source SHA}" --run "${BRIT_READINESS_RUN:?set captured run path}" --jobs "${BRIT_READINESS_JOBS:?set captured job pages path}"'
retire-when: >
  when every required inheritance leg is continuously qualified at integration and
  readiness consumption and this separate migration habit is redundant
---
DELTA 2026-09-28: Qualification machinery and negative fixtures are authored.
No completed remote run at the changed revision is supplied here. Aggregate Tests
pass success, publication success, and local fixture success remain distinct claims.
Native daily-driver and Git compatibility milestones are explicitly unwired.

Set BRIT_READINESS_SHA, BRIT_READINESS_RUN and BRIT_READINESS_JOBS from the exact
captured run described in the inherited-CI plan. Missing inputs refuse immediately.

DELTA 2026-09-28 (qualification verification; RED preserved): Nine qualifier tests
and all 47 CI-helper tests passed locally. Real captured run 36458363380 at
f8a0e14cd7dc55a9b5ddd3d554fd41def8708a11 fails this new contract as expected; it
predates the new required check. New remote integration evidence remains absent.

DELTA 2026-10-12 (upstream reconciliation; RED preserved): Captured Actions run
36514749389 attempt 1 at e3de7c0c046049c4e625b38d28c9e1b634f82dcf qualifies all
18 required jobs under that revision's contract. The 332-commit gitoxide integration
at 09f85be4e3fda42f3e8fd19c53a63758fdf6fb3d adds cargo-auditable to the aggregate
and qualifier. Nine qualifier tests and 47 CI-helper tests pass locally; new
exact-revision remote matrix evidence remains required. Backlog assessment lives
in docs/plans/2026-09-28-governed-readiness.md; no habit acceptance is inferred.

DELTA 2026-10-12 (local integration proof; RED preserved): The built public CLI
passes 39 journeys and CLI/core passes 31 tests. Generic `brit-epr` passes 42
tests without protocol features; the default engine/bridge suites also pass.
Protocol-only test targets now declare their feature requirement, and the journey
qualification requires the feature-disabled engine step. Formatting and all 47
CI-helper tests pass. These local checks do not replace the new revision's remote
matrix or independent workflow acceptance.

DELTA 2026-10-12 (fork transport proof; RED preserved): Twelve send-pack tests,
eight repository push checks and 37 server tests pass against the merged engine.
The server extensions retain their feature declarations and migrate packet decode
errors to gix-error; their existing tests prove both upload-pack versions and daemon
connect parsing. The baseline exact-run evidence remains revision-bound.

DELTA 2026-10-12 (focused lint proof; RED preserved): CLI/core/journey clippy
passes all targets using the inherited CI `--no-deps -D warnings -A unknown-lints`
contract on Rust 1.96.1. Two inherited attribute-baseline iterator expressions were
updated to equivalent `collect` calls. Full workspace formatting and the 69-package
registry dependency closure pass; cross-platform and alternate-feature matrix
qualification remains the exact-revision remote lane.

DELTA 2026-10-12 (reach rehearsal; RED preserved): `epr ready --target origin/main`
at merge 9e86ed425a58d0fe5299d442782aff9cbf92b002 reports NOT READY: missing
`.epr-meta/elohim/packages`, inactive Git hooks, and package projection verification
failure. The standalone boundary must reconcile these evaluator expectations before
reach; local check success is insufficient. No push or parent pin promotion occurred.

DELTA 2026-10-12 (CI-branch reach scope; RED preserved): Source inspection of
`elohim/eprfs/epr-cli/src/{doctor,ready}.rs` in the parent confirms the installed
evaluator unconditionally requires the parent package root and invokes its fixed
`elohim/sdk/domains/elohim-agent/scripts/package-projections.mjs` path. Brit's
root manifest terminates that parent cascade and does not copy its hooks. Brit's
CLAUDE.md requires risky integration to reach `run-ci/**` first; the passing local
checks support that CI-only push. Main integration and parent pin promotion still
require exact-revision `Tests pass` evidence. The generic standalone evaluator
incompatibility remains recorded rather than treated as package drift in Brit.
