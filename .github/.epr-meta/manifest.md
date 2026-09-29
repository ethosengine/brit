---
epr-meta-version: 1
id: brit-inherited-ci-boundary
covers: subtree
purpose: >
  Preserve gix's platform and feature discipline while adding first-party Brit
  executable and qualification-contract checks at the same integration revision.
---
# Inherited CI discipline

The workflow owns execution, including declared exclusions from Tests pass. The
root `.epr-meta/readiness.json` names the exact evidence required by each milestone.
Qualifier fixture tests prove its refusal contract, not product readiness.
Upstream synchronization remains pristine; fork integration reruns both matrices.
