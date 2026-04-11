# Brit — EPR-Applied Git Roadmap

**Brit** (בְּרִית, "covenant") is an expansion of [gitoxide](https://github.com/GitoxideLabs/gitoxide) that integrates Elohim Protocol primitives — pillar coupling (lamad / shefa / qahal), three-tier EPR content addressing, libp2p transport, and ContentNode-first repository semantics.

The name rhymes with *git* for a reason: git is the substrate, brit is the covenant laid on top. A commit in brit isn't just a hash-linked snapshot — it's a witnessed agreement whose terms (lamad, shefa, qahal) are carried in the commit itself. Merges are covenantal joinings of lineages. Forks are new covenants, legitimately grown from old ones, not defections from them.

## Why fork gitoxide

gitoxide is a pure-Rust git implementation with a clean modular design — each concern lives in its own `gix-*` crate (`gix-hash`, `gix-object`, `gix-protocol`, `gix-pack`, `gix-transport`, …) and swaps independently. That modularity is exactly what we need to layer EPR semantics onto git without rewriting the object model from scratch.

## Architecture — semantic layer over rust-ipfs

Brit is the **semantic layer**: commits, refs, signing, authorship, history, pillar coupling, governance.
[rust-ipfs](https://github.com/ethosengine/rust-ipfs) is the **storage/transport substrate**: CIDs, multihash, Bitswap, libp2p.
EPR lives in **both places**:

- **Commit trailers** carry a canonical summary of pillar metadata (`Lamad:`, `Shefa:`, `Qahal:`) — git-compatible, every existing tool reads them, drift-free.
- **Linked ContentNodes** carry the rich graph — full lamad descriptions, shefa economic events, qahal governance context, per-branch READMEs, etc. — addressed by CID embedded in the trailer (`Lamad-Node: bafkreiabc…`).

This is the "both" / hybrid (c) design: trailer is the protocol surface; linked node is the graph surface. Same split as EPR Head/Document.

## Loops this closes

| Loop | How EPR-git closes it |
|---|---|
| **Orchestrator baselines** | Pipeline baselines become refs (or notes refs) in an EPR-native repo. No more Jenkins artifact roulette. |
| **Build artifacts** | CI outputs become CID-addressed git objects in a stewarded namespace. Steward nodes reproduce and publish. Harbor becomes optional. |
| **Schema versioning** | Every schema version is a commit, every evolution is a branch. N versions coexist along a DAG (see `project-schema-versioning-p2p` memory). |
| **Journal publishing** | Journal entries become commits to a `journal/` ref. Publishing = pushing to a stewarded ref. The "journal as protocol mouth" memory, realized. |
| **Attestation layer** | Agent-signed reviews become commit trailers with agent capability claims (`Reviewed-By: agent-security-v1 <cid>`). Signature proves review by a specific capability. |
| **Governance of forks** | A fork is a ContentNode with its own stewardship, attestations, and peers — a legitimate alternate lineage, not a second-class copy. |
| **Branches-as-views** | A branch isn't a ref pointer, it's a ContentNode with a per-branch EPR README/governance context. `main` tells users one story; `dev` tells developers another; `feature/x` tells what x unlocks. |

## Phased decomposition

Each phase produces working, testable software on its own. Phases 0–1 are implementation-ready today. Phases 2+ need design work before their own plans are written.

| # | Phase | Scope | Status |
|---|---|---|---|
| **0** | Workspace scaffolding | New `brit-epr` crate, config-crate stub, new `brit-verify` example binary. Leaves `gix-*` crates untouched. Upstream-rebaseable. | **Plan: [2026-04-11-phase-0-epr-trailer-foundation.md](./2026-04-11-phase-0-epr-trailer-foundation.md)** |
| **1** | Pillar trailer model | `PillarTrailers` struct, parser, validator, `brit-verify <commit>` CLI. Uses existing `gix-object::commit::message::BodyRef::trailers()`. Commits round-trip through stock git. | **Plan: [2026-04-11-phase-0-epr-trailer-foundation.md](./2026-04-11-phase-0-epr-trailer-foundation.md)** *(bundled with Phase 0 — they're tiny and related)* |
| **2** | ContentNode adapter | `RepoContentNode`, `CommitContentNode`, `BranchContentNode` types in `brit-epr`. Export adapter: git repo → ContentNodes in elohim-storage. Import the elohim monorepo itself. | 📝 needs design session |
| **3** | libp2p transport | `brit-transport` crate wiring gix-protocol to libp2p request-response. New `/brit/fetch/1.0.0` protocol. Clone a small repo over libp2p. | 📝 needs design session |
| **4** | Per-branch READMEs | Branches resolve to ContentNodes via `.brit/README.epr` (or equivalent). Display + tooling. Round-trip test. | 📝 needs design session |
| **5** | DHT announcement + peer hosting | Announce repo CID + commit CIDs to DHT. Fetch finds peers via DHT. Two-peer test. | 📝 needs design session |
| **6** | Forking as governance | Fork as ContentNode with its own stewardship. Cross-fork merges via qahal consent. Simulate fork+merge governance flow. | 📝 needs design session |

Phases 2+ are sketched here only to make the shape of the whole visible. Each gets its own brainstorming pass and its own plan file before implementation.

## Design principles

1. **Round-trip with stock git.** Every commit brit produces must be readable by stock git. Trailers are lines in the commit message, not magic bytes. This preserves the onboarding flywheel — you can `git clone` a brit repo from GitHub without tooling.
2. **Upstream-rebaseable.** New functionality goes in new crates (`brit-epr`, `brit-transport`, `brit-cli`). Modifications to existing `gix-*` crates are limited to bugs and additive extension points, proposed upstream where possible. This keeps the fork from becoming a boat anchor.
3. **Trailer is the protocol surface.** Linked ContentNode is the graph surface. If a reader has only the commit, they can tell whether it's pillar-compliant. The linked node enriches the view but is not required for validation.
4. **Additive, not destructive.** Phase 0 doesn't rename `gix-*` to `brit-*`. It adds alongside. We earn wholesale renames only once the semantics diverge enough to justify the churn.

## How to read the plans

Each plan in this directory follows the [superpowers writing-plans skill](https://github.com/obra/superpowers) format: bite-sized tasks, every step with actual content (tests, code, commits), TDD-first. They are implementation-ready for someone with Rust experience who has **zero** prior context on this codebase.

Start with `2026-04-11-phase-0-epr-trailer-foundation.md`.
