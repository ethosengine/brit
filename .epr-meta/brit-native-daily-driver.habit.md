---
epr-habit-version: 1
id: brit-native-daily-driver
invariant: >
  A native developer can safely author, recover, reconcile, independently review
  and integrate scoped work through Brit with enforced gates and stock Git interoperability.
status: unwired
active: false
refs:
  - docs/plans/2026-09-28-daily-driver.md
retire-when: >
  when the full native workflow is the continuously verified release and integration floor
  and this separate cutover marker adds no information
---
DELTA 2026-09-28: Declared, unqualified. Local staged-authoring tests and shared-tree
proof do not establish pre-push enforcement, independent target-bound acceptance,
recovery across interrupted operations, or installed release delivery. Git remains
the explicit bridge for unsupported operations. Qualification must first wire all
stations in docs/plans/2026-09-28-governed-readiness.md and the readiness contract.
DELTA 2026-10-12: UNWIRED preserved; independently rebuilt public binary passed 56 CLI tests (pre_push 8, commit_cleanup 7, authoring 19, existing journeys 22). Exact resolved pre-push input, rejection safety, 2,000-ref early-close handling and noninteractive cleanup now have local proof; unsupported lease protections and unverified overwrites fail closed. Sprint evidence and remaining engine/installation/integration gaps: docs/plans/2026-09-28-daily-driver.md. Remote CI and whole-workflow acceptance remain unproven.
DELTA 2026-10-12: Qualification sprint preserves UNWIRED. Independent rebuild passed 60 public CLI tests and strict scoped Clippy, including scoped interruption/recovery and installed-evaluator forwarding. Installer fault fixtures, 14 consumer tests and 12 readiness tests passed. Native evaluator has 8 standalone governance and 5 integration fixture passes; combined installation, exact-source remote CI, real released-version upgrade and two-human acceptance remain distinct stations in the existing plan.
