---
epr-meta-version: 1
id: brit-epr-feature-boundary
purpose: >
  Brit's EPR adapter: a generic covenant engine and a feature-gated Elohim
  protocol schema, each with a distinct dependency direction.
---

# Generic engine and protocol adapter

`src/engine/` remains usable with `elohim-protocol` disabled. Elohim pillar
vocabulary and concrete schema handling live in the feature-gated `src/elohim/`
module. Keep the generic engine's real CID/Git behavior working without the
Elohim feature. EPR flow in the Elohim workspace remains the owner of intent and review decisions; this
crate represents and validates its Git-facing material. The child engine
manifest gives a narrow advisory for a concrete feature-boundary crossing.
