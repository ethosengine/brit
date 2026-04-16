# Brit — EPR-Applied Git Roadmap

**Brit** (בְּרִית, "covenant") is an expansion of [gitoxide](https://github.com/GitoxideLabs/gitoxide) that makes version control covenantal. Every commit carries three-pillar metadata (lamad/shefa/qahal). Merges are covenantal joinings of lineages. Forks are new covenants, legitimately grown from old ones. Branches carry reach levels that govern who sees what.

## Why Fork gitoxide

gitoxide is a pure-Rust git implementation with a clean modular design — each concern lives in its own `gix-*` crate and swaps independently. That modularity lets us layer protocol semantics onto git without rewriting the object model. The engine/app-schema split (§2 of the schema manifest) keeps brit-epr usable as a generic substrate while the Elohim Protocol vocabulary stays behind a feature flag.

## Architecture

Brit is the **semantic layer** (commits, refs, pillar coupling, governance) over two substrates:
- **gitoxide** — the git object model, refs, pack protocol
- **rust-ipfs** — CIDs, multihash, Bitswap, libp2p (Phase 2+)

The hybrid design: commit **trailers** are the protocol surface (survive `git clone` from any forge); linked **ContentNodes** are the graph surface (rich pillar metadata, resolved through doorway).

See [composition.md](../composition.md) for how brit composes with rakia, protocol schemas, and storage.

## Phase Decomposition

Seven phases, decomposed by **protocol capability** — what the protocol gains, not what crate gets written. Each phase produces working, testable software.

| Phase | Capability unlocked | What the protocol gains | Status |
|---|---|---|---|
| **0+1** | **Commits carry covenant** | Every commit can be checked for three-pillar compliance. Trailers parse, validate, round-trip through stock git. `brit verify` works. | **Complete** ([plan](./2026-04-11-phase-0-epr-trailer-foundation.md)) |
| **2a** | **Artifacts become self-aware** | Build, deploy, and validation attestations as signed ContentNodes. Refs under `refs/notes/brit/`. Reach computation. `brit build-ref` CLI. Pure local. | **Complete** ([plan](./2026-04-16-phase-2a-build-attestation-primitives.md)) |
| **2** | **Git artifacts become protocol content** | Repos, commits, branches, trees addressable by CID. Schema-driven types. Rakia can emit BuildManifestContentNode. | [Summary](phases/phase-2-contentnode-adapter.md) |
| **3** | **Git over P2P** | Clone, fetch, push over libp2p. No forge required. Rakia-peer shares transport. | [Summary](phases/phase-3-libp2p-transport.md) |
| **4** | **Branches tell their story** | Per-branch READMEs as EPRs. Reach-governed visibility. Build status per branch. | [Summary](phases/phase-4-branch-readmes.md) |
| **5** | **Repos discoverable on the network** | DHT announcement. Peers find repos by CID. Build manifests discoverable. | [Summary](phases/phase-5-dht-discovery.md) |
| **6** | **Forks are governance acts** | Fork as ContentNode with stewardship. Cross-fork merges via qahal consent. | [Summary](phases/phase-6-fork-governance.md) |

### Parallel evolution with rakia

Brit and [rakia](https://github.com/ethosengine/rakia) evolve on parallel tracks. Each brit phase unlocks rakia capability, but neither blocks the other:

| Brit phase | What rakia gains |
|---|---|
| Phase 0+1 (current) | Attestation trailers. Change detection via gix. Baseline refs. |
| Phase 2 | BuildManifestContentNode + BuildAttestationContentNode as protocol content |
| Phase 3 | Shared P2P transport for manifest distribution |
| Phase 4 | Build status per branch via reach-governed branch metadata |
| Phase 5 | Build manifest discovery via DHT |
| Phase 6 | Forked build recipes with independent stewardship |

## Design Principles

1. **Round-trip with stock git.** Every commit brit produces must be readable by stock git. Trailers are RFC-822 lines, not magic bytes. `git clone` from any forge works.

2. **Schema-driven development.** JSON Schema files define all ContentNode types, trailer grammar, and enum vocabularies. Rust types are generated from schemas. Validation harness catches drift.

3. **Engine doesn't know the protocol.** brit-epr parses trailers and dispatches to `AppSchema`. The Elohim Protocol vocabulary is behind `#[cfg(feature = "elohim-protocol")]`. Someone can write `AcmeSchema` for carbon accounting without touching brit.

4. **Brit doesn't own governance.** Brit reads consent requirements from the parent EPR's qahal context. The governance gateway handles tally logic. Brit publishes proposals and executes results.

5. **Upstream-rebaseable.** New functionality goes in new crates. Modifications to `gix-*` crates are limited to bugs and additive extension points. The fork doesn't become a boat anchor.

6. **Additive, not destructive.** Phase 0 doesn't rename `gix-*` to `brit-*`. We earn renames only when semantics diverge enough to justify the churn.

7. **LLM-first CLI.** Command names mirror git (use training as cognitive carrier). Hard parts of pillar authoring pushed into skill + template, not into the prompt. Humans use the UI for review and consent.

## Loops This Closes

| Loop | How brit closes it |
|---|---|
| **Build baselines** | Pipeline baselines become git refs (`refs/notes/rakia/baselines`), not Jenkins artifacts |
| **Build artifacts** | CI outputs become CID-addressed, attested ContentNodes via rakia |
| **Schema versioning** | Every schema version is a commit, every evolution is a branch, N versions coexist |
| **Journal publishing** | Journal entries become commits to stewarded refs |
| **Attestation** | Agent-signed reviews are commit trailers with capability claims |
| **Fork governance** | Forks are ContentNodes with their own stewardship and governance |
| **Branch views** | Branches are ContentNodes with reach, audience, and per-branch READMEs |

## Key Documents

| Document | Purpose |
|---|---|
| [Schema manifest](../schemas/elohim-protocol-manifest.md) | 1700+ line exploration of the full app schema: ContentNode catalog, trailer spec, CLI surface, signals, doorway registration |
| [Design spec](../specs/2026-04-12-brit-design.md) | Formal spec with architecture, design decisions, and open questions |
| [Composition model](../composition.md) | How brit composes with rakia, protocol schemas, rust-ipfs, storage |
| [Merge consent critique](../schemas/reviews/2026-04-11-merge-consent-critique.md) | Design review of async-default merge consent |
| [Phase 0+1 plan](./2026-04-11-phase-0-epr-trailer-foundation.md) | Implementation plan (complete) |

## How to Read the Plans

Phase plans in `phases/` are summary documents with vision, prerequisites, sprint sketches, and risks. Each gets a full implementation plan (in this directory) before execution.

Implementation plans follow the [superpowers writing-plans skill](https://github.com/obra/superpowers) format: bite-sized tasks, TDD-first, implementation-ready for someone with Rust experience and zero prior context.
