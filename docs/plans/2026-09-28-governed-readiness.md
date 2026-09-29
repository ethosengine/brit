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
