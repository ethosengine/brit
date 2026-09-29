---
title: Reconcile April parity journeys with Git census and Middot
status: active
serves: brit-git-compatibility
tags: [mishpat, middot, brit, git-compatibility, parity, evidence]
---
# Parity evidence reconciliation

Reuse April's existing command suites and helper semantics. Join executed assertions
to the pinned census as evidence of the actual assertion strength, not full feature
acceptance. Runtime observations are derived C; native recording uses existing
Measure/observation primitives, with no new DHT entity or authority mechanism.

- [ ] Capture source-bound runtime receipts from existing parity helpers, with exact executables/oracle, hash lanes, exit/output comparison strength, compat deferrals, skips and incomplete suites; isolate fixtures and bound execution. Serves brit-git-compatibility.
- [ ] Reconcile suite rows and runtime receipts with census atoms, preserving unmatched/unexecuted/ambiguous cases and method freshness; expose repeatable aggregate and per-command reports with regression tests. Serves brit-git-compatibility.
- [ ] Reconcile executed suite evidence, wire reproducible reconciliation into CI, register observational measures through the existing native interface where supported, and preserve recallable evidence plus independent review. Serves brit-git-compatibility.

A matching option spelling is a candidate association, not an assertion that the
whole option has been tested. Negative-input rows stay distinguishable from successful
operations. `effect` is exit-code comparison; `bytes` currently compares merged,
shell-captured output with trailing newlines normalized. State equivalence requires
separate assertions. Skips, deferred rows, killed/stopped/time-limited suites and
fail-fast tails never count as pass. No existing red behavioral result is silently
admitted into a blocking green policy. Source changes invalidate earlier receipts.
