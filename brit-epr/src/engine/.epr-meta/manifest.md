---
epr-meta-version: 1
id: brit-epr-engine-generic
purpose: >
  Protocol-neutral covenant types and traits that compile with the Elohim
  protocol feature disabled.
rules:
  - id: elohim-schema-stays-in-adapter
    class: inject
    when:
      write: "*.rs"
      contains-any: ["ElohimProtocolSchema", "PillarTrailers", "BuildAttestationContentNode"]
    route-to: { dest: "brit-epr/src/elohim/" }
    why: >
      These concrete Elohim schema types belong in the feature-gated adapter.
      Keep engine traits and CID/Git primitives protocol-neutral, then verify
      `brit-epr` with default features disabled. The matcher flags only the named
      symbols; it does not prove the full feature boundary.
    retire-when: >
      when a feature-matrix test enforces the engine's protocol independence
      before every accepted change
---

# Engine guide star

Prefer the existing `AppSchema` and object-store interfaces before adding a
parallel abstraction. The source code and feature-matrix build decide the real
boundary; this manifest's advisory sees only the matched symbols in a proposed
write. Do not move generic content addressing into a Holochain-specific module.
