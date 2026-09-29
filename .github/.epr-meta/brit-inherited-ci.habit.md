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
