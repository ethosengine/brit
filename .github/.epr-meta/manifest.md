---
epr-meta-version: 1
id: brit-inherited-ci-boundary
covers: subtree
purpose: >
  Preserve gix's platform and feature discipline while adding first-party Brit
  executable and qualification-contract checks at the same integration revision.
---
# Inherited CI discipline

Gitoxide's QA scheduler plans the inherited checks; the workflow runs that plan beside
Brit's first-party jobs. The root `.epr-meta/readiness.json` names the exact evidence
required by each milestone and declares every planned job required, deferred, replaced
or disabled, so exclusions from Tests pass stay written down.
Qualifier fixture tests prove its refusal contract, not product readiness.
Upstream synchronization remains pristine; fork integration reruns both matrices.
