---
title: Brit native developer readiness through habit delivery
status: active
serves: brit-feature-reconciliation
---
# Native developer readiness

The entry loop is reconcile → rank → claim/resume → produce → independent review →
accept → habit DELTA. Use installed epr and the same native evaluator from Brit and
the harness. `.epr-meta` declares responsibility; memory helps find current sources
and unresolved work but does not elect acceptance. No new work queue or status ledger.

Start in a standalone checkout:

```sh
epr flow project --root . --recipes .epr-meta/recipes.yaml
brit context docs/plans/2026-09-28-governed-readiness.md --root .
brit context docs/plans/2026-09-28-governed-readiness.md --root . --json
```

Choose an eligible intent from context, then use `epr flow claim --on <intent> --as
<participant> --brief <brief>`. An existing owner's work is a resume/contribution
boundary, never a reason to steal the claim. Produce evidence, obtain independent
review, and fulfill through existing flow verbs. A fulfillment alone does not prove
acceptance. Add the evidence-backed DELTA to the owning habit; do not flip green
because a plan checkbox was ticked.

## First readiness stations

The authored order below is priority within this feature. Each station retains its
finish assertion even when implementation discovers additional prerequisite work.
The dependency prose describes required evidence; it does not pretend recipe edges
are an executable dependency scheduler.

- [ ] Standalone reconciliation: installed epr reads Brit's declared repository identity, local habits and recipe-projected work; Brit and harness JSON match; unresolved acceptance and missing evidence remain visible. Serves brit-feature-reconciliation.
- [ ] Fast feature entry: measure cold and warm context on the same fixture, with current authority found, eligible action selected and a second developer resuming without full history; preserve bounded recall and disclose stale or truncated context. Requires standalone reconciliation; serves brit-feature-reconciliation.

- [ ] Bounded memory bootstrap: orient from repository-local habit declarations and covenant order within the existing recall budget; prove standalone behavior without the legacy `genesis/manifests/habits.yaml` bootstrap input. Native context is already shared with the harness; the separate bootstrap reader still requires its governed memory contract and generated register. Serves brit-feature-reconciliation.

The dependent work is owned by the per-habit plans:

- [Inherited CI](2026-09-28-inherited-ci.md)
- [Native daily driver](2026-09-28-daily-driver.md)
- [Git comparison horizon](2026-09-28-git-compatibility.md)

## Compatibility horizon

The capability-family map is `docs/parity/capability-boundaries.md`. Work below stays
coarse until readiness blockers are discharged. Git v2.54.0 is a comparison boundary,
not a promise that every legacy behavior will be implemented.


## Proof distinctions

The readiness qualifier consumes captured Actions evidence for one completed exact
run; it does not fetch, attest authenticity or turn a local JSON file into remote
standing. Required skipped/missing/cancelled/wrong-revision results refuse. The normal
forge aggregate remains an integration signal with known exclusions. Current native
daily-driver and compatibility contracts remain unwired and cannot qualify.

The parent consumes pinned evidence through its attested gate; it does not import
Brit's habits as duplicate authority. Standalone habit attention does not expand the
operator's shared two-active-habit budget.

## Backlog assessment — 2026-10-12 upstream integration

The declaration order in `.epr-meta/habits-covenant.md` remains the priority.
Implementation progress and station acceptance must be reconciled separately:

1. **Feature reconciliation:** the nine standalone fixture checks and measured
   0.278s first entry / 0.272s warm median are recorded in the owning habit.
   Finish the repository-local bounded memory bootstrap and independent operational
   acceptance, then measure a second developer resuming from installed interfaces.
   Do not repeat the already implemented Brit-to-epr dispatch.
2. **Inherited CI:** baseline `e3de7c0c046049c4e625b38d28c9e1b634f82dcf`
   qualifies against its existing contract using Actions run `36514749389`,
   attempt 1. This supplies the remote evidence missing from the September ledger;
   it does not qualify the upstream integration revision. Upstream adds the
   `cargo-auditable build` job: it is now required by both the aggregate and exact-run
   qualifier. Re-run inherited and first-party checks on the merged revision.
3. **Daily driver:** ordinary staged commit creation and commit hooks have local
   regression coverage; the README's earlier allow-empty-only description is stale.
   The October 12 sprint in `2026-09-28-daily-driver.md` implements exact resolved
   pre-push updates, including create/update/delete, with hook rejection preventing
   every remote update. Its local evidence and remaining transport limitations are
   recorded there; this does not establish full push qualification. Scoped
   temporary-index authoring, interruption recovery, target-bound independent
   integration, installed upgrade/rollback and two-developer qualification stay open.
4. **Git compatibility:** the pinned census, parser overlay and legacy assertion
   reconciliation are implemented and tested. Unchecked plan stations do not mean
   those implementations are absent. Remaining work is source-bound runtime proof,
   state equivalence beyond exit/output comparison, public conformance selectors
   and governed adoption of intentional differences. Do not use parser coverage as
   a behavior-completion percentage.

The upstream comparison target is `09f85be4e3fda42f3e8fd19c53a63758fdf6fb3d`
(332 commits not previously reachable from Brit). It brings gix 0.89, gix-error
0.5, gix-features replacement by gix-parallel/gix-utils, linked-worktree and
byte-name fixes, configurable HTTP clients and Tix recovery changes. These reduce
engine work but do not establish Brit workflow acceptance or pre-push semantics.
The Git v2.54.0 census comparison horizon remains unchanged.

Historical phase summaries and `docs/plans/README.md` are vision/context; this
plan, the owning habit atoms and their runnable checks are the current work floor.

Native projection on this checkout produces 18 recipe intents. The readiness
context ranks its three unaccepted feature-entry assertions in covenant order:
standalone reconciliation, fast feature entry, bounded memory bootstrap. None has
an owner or independent acceptance record in this local sidecar. Existing fixture
proof should be carried into review rather than treating these as blank implementations.

The feature-disabled engine matrix exposed protocol-specific integration targets
without feature requirements. Their declarations now require `elohim-protocol`;
the inherited journey lane explicitly runs the remaining generic engine suite with
`--no-default-features`. Both configurations must pass; this preserves the generic
engine boundary without silently dropping protocol tests from the default suite.

The merge reach rehearsal at `9e86ed425a58d0fe5299d442782aff9cbf92b002` is
**NOT READY**: the installed evaluator requires a package root and activated Git
hooks, and its package projection verification fails in the standalone checkout.
Reconcile the standalone governance boundary with the native evaluator before
reach. This is an additional qualification prerequisite; local checks and captured
baseline CI do not override it.

Inspection on 2026-10-12 locates those rehearsal failures in the evaluator's
unconditional monorepo assumptions: `doctor.rs` requires the parent package tree,
and `ready.rs` executes a fixed parent projection-verifier path. Brit's root
manifest terminates the parent cascade and expressly avoids copying its hook
implementation. This evaluator result needs a standalone-aware implementation;
it does not diagnose Brit package drift. The repository's declared route is a
locally verified `run-ci/**` push, followed by exact-revision remote CI acceptance
before main integration or parent pin promotion.
