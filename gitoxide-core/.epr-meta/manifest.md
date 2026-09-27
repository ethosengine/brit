---
epr-meta-version: 1
id: gitoxide-core-git-bridge
purpose: >
  Reusable Git operations behind the public CLI, retaining Git object, index,
  ref and interoperability semantics without Elohim authority.
---

# Git bridge seam

Keep Git operations reusable by the root `src/plumbing/` dispatcher and
`brit-cli`. The root `src/plumbing/` is where parsed Git command dispatch is
extracted as a library interface; it is not moved into `gitoxide-core`.
Existing `gitoxide-core` code is real Git functionality and `ein` remains a
consumer. Commitment, review and acceptance belong to `epr`
flow in the Elohim workspace or its published interface. Channel standing and
election belong to content governance. This manifest
records those boundaries; no filename predicate can decide Git versus authority
semantics reliably, so it declares no edit-time rule.
