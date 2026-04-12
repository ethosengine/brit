# Vendored Elohim Protocol Schemas

This directory contains JSON Schema files vendored from the Elohim Protocol's canonical schema tree. Brit is a **standalone repository** that implements the Elohim Protocol, and therefore carries its own copy of the protocol primitives it consumes rather than depending on the elohim monorepo at build time.

## Versioning

Files in `v1/` track the Elohim Protocol's `v1` schema version. When the upstream protocol releases `v2`, brit will vendor into `v2/` alongside and migrate when the `brit-epr` `elohim-protocol` feature is ready.

## Which schemas live here

Only schemas brit actually consumes. We do NOT vendor the entire protocol schema tree — that would defeat the point. When brit starts using a new protocol primitive, vendor just that schema.

| File | Source | Purpose |
|---|---|---|
| `enums/reach.schema.json` | `elohim/sdk/schemas/v1/enums/reach.schema.json` | The reach enum (`private` → `self` → `intimate` → `trusted` → `familiar` → `community` → `public` → `commons`). Used by `BranchContentNode.reach` and `MergeProposalContentNode.sourceReach` / `targetReach`. |

## Update procedure

1. Copy the upstream file from the Elohim Protocol reference implementation into the matching path here.
2. Diff against the previous vendored version. If enum values changed, that's a protocol version break and brit needs to follow up with a `v2` migration.
3. Commit with a message referencing the upstream commit (when known).
4. Run the brit test suite — any test that depends on a vendored enum value will tell you whether the update is semantically compatible.

## Not a monorepo dependency

Brit's `Cargo.toml` and `brit-epr` crate point at these files via a `build.rs` that embeds the JSON at build time. There is no git submodule, no path reference, no network fetch. The files here are the source of truth for brit at build time, and they can be updated independently of any upstream work.
