# Phase 2: Git Artifacts Become ContentNodes

**Status:** Needs design session
**Depends on:** Phase 0+1 complete (trailer parsing + validation)
**Capability unlocked:** Repos, commits, branches, trees, and blobs are addressable protocol content. Rakia can emit BuildManifestContentNode and BuildAttestationContentNode.

## Vision

A git repository, viewed through brit, is not just a DAG of commits — it's a constellation of ContentNodes in the protocol's content graph. Each commit, branch, tree, and blob has a CID. Each carries three-pillar metadata. The doorway can serve any of them to any protocol participant.

This is the phase where brit stops being "git with trailer discipline" and becomes "the version control surface of a distributed knowledge network."

## What Changes

- `RepoContentNode`, `CommitContentNode`, `BranchContentNode`, `TreeContentNode`, `BlobContentNode` types implemented in `brit-epr`
- Export adapter: git repo -> ContentNodes (serialize to DAG-CBOR, produce CIDs). Source of truth: git object store. ContentNodes are projections of git objects, not replacements.
- Import adapter: ContentNodes -> git objects (for repos cloned via P2P, not just git). Source of truth for imported repos: the DHT-notarized ContentNodes until a local git object store is populated.
- The elohim monorepo itself is imported as a test — the first brit-native repo
- JSON Schema files for all ContentNode types written and validated
- Rust types generated from schemas (codegen pipeline established)
- `schema_contract.rs` validation harness (mirrors elohim-storage pattern)

## What This Unlocks for Rakia

- `BuildManifestContentNode` can be implemented — rakia's Sprint 6 ("The Manifest Becomes a ContentNode") depends on this adapter
- `BuildAttestationContentNode` can be stored and retrieved as protocol content
- Manifest CIDs are computable — rakia can content-address its build manifests

## What This Unlocks for the Protocol

- Repos are addressable by CID — "give me this repo" works regardless of which peer hosts it. Source of truth: the git object store, projected as ContentNodes into the DHT.
- Commits are linked to their ContentNode representations — the doorway can serve rich commit views
- Branches resolve to ContentNodes with reach, governance, and purpose metadata. Source of truth for reach/governance: the DHT-notarized BranchContentNode. Source of truth for branch content: the git ref.

## Prerequisites

- Phase 0+1 complete (trailer parsing, PillarTrailers struct, brit-verify)
- rust-ipfs available for DAG-CBOR serialization and CID computation (may require adding as submodule)
- Protocol schemas for all brit ContentNode types finalized (source of truth: `elohim/sdk/schemas/v1/`, vendored into brit's `schemas/elohim-protocol/v1/`)

## Sprint Sketch (to be decomposed in design session)

1. **Schema first** — JSON Schema files for all 12+ ContentNode types, codegen pipeline, validation harness
2. **Core adapter** — RepoContentNode + CommitContentNode: serialize git repo/commit to DAG-CBOR, produce CIDs
3. **Tree/blob adapter** — TreeContentNode + BlobContentNode: file-level content addressing
4. **Branch/ref adapter** — BranchContentNode with reach, protection rules, per-branch README slot
5. **Reserved types** — BuildManifestContentNode + BuildAttestationContentNode: implement the reserved slots from §5.12
6. **Import test** — import the elohim monorepo as the first brit-native repo; validate round-trip

## Risks

- **DAG-CBOR canonical serialization** must produce stable CIDs. Two implementations computing a CID for the same commit must get the same result. This needs a canonicalization spec before implementation.
- **rust-ipfs integration depth** — how much of rust-ipfs does brit need? Just DAG-CBOR + CID computation, or also blockstore? Decision affects submodule timing.
- **Schema evolution** — once ContentNode types are published and have CIDs in the wild, changing the schema changes the CIDs. Schema versioning follows the P2P DAG model (source of truth: the CID-addressed schema version itself; N versions coexist; migrations compose along paths). Versioning strategy needed before publishing.
