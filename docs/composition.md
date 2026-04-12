# Composition Model

How brit composes with the protocol schemas, rust-ipfs, rakia, and elohim-storage.

## The Four Sources

Brit sits at the intersection of four systems. It consumes protocol schemas and rust-ipfs from below, and provides git primitives upward to rakia and the broader protocol.

```
                  ┌─────────────────────────────────┐
                  │     Protocol Schemas             │
                  │  elohim/sdk/schemas/v1/          │
                  │                                  │
                  │  ContentNode types               │
                  │  Reach enum (8 levels)           │
                  │  Pillar vocabulary               │
                  │  Attestation format              │
                  │  Trailer key grammar             │
                  └──────────┬──────────────────────┘
                             │ defines vocabulary
                             v
       ┌─────────────────────────────────────────────────────┐
       │                     Brit                            │
       │              (covenant on git)                      │
       │                                                     │
       │  Engine (brit-epr): trailer parse, validate,        │
       │    schema dispatch, CID utilities, signing hooks    │
       │                                                     │
       │  App schema (elohim-protocol): pillar trailers,     │
       │    ContentNode catalog, signal taxonomy,             │
       │    reach-per-ref, merge consent                      │
       └──────┬──────────────────────┬──────────────────────┘
              │                      │
              │ provides git         │ provides CID storage
              │ primitives           │ and transport
              v                      v
   ┌──────────────────┐    ┌──────────────────────────┐
   │     Rakia         │    │     rust-ipfs             │
   │  (firmament)      │    │  (storage/transport       │
   │                   │    │   substrate)              │
   │  Change detection │    │                           │
   │  Baseline refs    │    │  CIDs, multihash          │
   │  Manifest CIDs    │    │  Bitswap, libp2p          │
   │  Attestation      │    │  DAG-CBOR serialization   │
   └──────────────────┘    └──────────────────────────┘
              ^                      ^
              │                      │
   ┌──────────┴──────────────────────┴──────────────────┐
   │        elohim-storage / steward infrastructure      │
   │                                                     │
   │  libp2p swarm + discovery                           │
   │  ContentNode storage API                            │
   │  DHT publication                                    │
   │  Doorway gateway (web2 bridge)                      │
   └─────────────────────────────────────────────────────┘
```

## Rules of Composition

### 1. The engine knows nothing about the protocol

brit-epr's engine layer parses RFC-822 trailers, validates structure, dispatches semantic checks to a loaded `AppSchema`, and provides CID utilities. It does not know the words lamad, shefa, or qahal. The Elohim Protocol vocabulary lives behind `#[cfg(feature = "elohim-protocol")]`. Someone could disable the feature, implement `AcmeSchema: AppSchema` for carbon accounting, and use the same engine without touching brit's source.

### 2. Brit doesn't own governance

The merge consent critique (2026-04-11) established: brit reads consent requirements from the parent EPR's governance primitives. It does not implement governance logic. A `MergeProposalContentNode` carries a TTL and frozen requirements; the actual consent accumulation happens in the governance gateway (elohim-storage). Brit publishes the proposal, waits for the governance surface to respond, and acts on the result.

This principle extends to every reach-change operation. Brit doesn't decide "can this ref move to reach=public?" — the protocol's qahal layer decides. Brit asks and executes.

### 3. Brit talks to the network through rust-ipfs

Content-addressed storage and retrieval use rust-ipfs's blockstore and Bitswap. Brit doesn't implement its own block storage or P2P transfer. The CID of a ContentNode is computed by rust-ipfs's DAG-CBOR serializer. Brit wraps the semantic layer (commits, refs, branches) over rust-ipfs's storage layer (CIDs, blocks, Bitswap).

### 4. The doorway is the web2 bridge

Stock git hosting (GitHub, GitLab) carries commits with trailers — the protocol surface. The linked ContentNodes, attestation graphs, reach governance, and per-branch READMEs resolve through the doorway. `.brit/doorway.toml` points the repo at its primary steward's gateway. Without a doorway, a brit repo degrades gracefully to "git with extra trailer discipline."

### 5. Schema changes flow from the protocol

```
Protocol Schema changes
  -> regenerate brit's Rust types (brit-epr-elohim)
  -> regenerate rakia's Rust types (rakia-core)
  -> regenerate TypeScript types (storage-client)
```

Brit vendors the protocol schemas at `schemas/elohim-protocol/v1/`. The authoritative copy lives in `elohim/sdk/schemas/v1/`. Updating is an explicit act.

## Submodule Topology

```
brit/                              (ethosengine/brit — fork of gitoxide)
  schemas/
    elohim-protocol/v1/            (vendored protocol schemas)
  elohim/
    rust-ipfs/                     (submodule -> ethosengine/rust-ipfs, future)
```

In the monorepo:
```
elohim/
  brit/                            (submodule -> ethosengine/brit)
  rakia/                           (submodule -> ethosengine/rakia)
    elohim/brit/                   (rakia's own submodule ref to brit)
  rust-ipfs/                       (submodule -> ethosengine/rust-ipfs)
  sdk/schemas/v1/                  (authoritative protocol schemas)
```

## How Brit Serves Rakia

Rakia never talks to git directly — it talks to brit. This means rakia automatically benefits from brit's protocol enrichment without reimplementing any of it.

| What rakia needs | What brit provides | Brit phase |
|---|---|---|
| Changed file paths since baseline | gix diff (object store, no shell-out) | Phase 0+1 (current) |
| Baseline ref management | notes-ref API (`refs/notes/rakia/baselines`) | Phase 0+1 |
| Attestation format for build outputs | Pillar trailers + `Built-By:` reserved key | Phase 0+1 |
| Build manifest as ContentNode | `BuildManifestContentNode` via ContentNode adapter | Phase 2 |
| Build attestation as ContentNode | `BuildAttestationContentNode` via ContentNode adapter | Phase 2 |
| Manifest distribution over P2P | libp2p fetch protocol (`/brit/fetch/1.0.0`) | Phase 3 |
| Build status per branch | Per-branch READMEs resolving to ContentNodes | Phase 4 |
| Manifest discovery via DHT | DHT announcement of repo + manifest CIDs | Phase 5 |
| Forked build recipes | ForkContentNode with independent stewardship | Phase 6 |

## How Brit Serves the Protocol

Beyond rakia, brit provides foundational infrastructure for the entire protocol:

| Protocol need | What brit provides |
|---|---|
| Provenance-aware code | Every commit carries pillar trailers: who built it, what value it creates, who governs it |
| Content-addressed repositories | Repo, commit, tree, blob all addressed by CID |
| Governance-aware merging | MergeProposalContentNode with async consent from qahal layer |
| Fork legitimacy | ForkContentNode — a new covenant with its own stewardship, not a second-class copy |
| Reach-governed visibility | Branches carry reach levels; merging IS reach elevation |
| Schema extensibility | AppSchema trait allows domain-specific vocabularies without forking brit |
| Stock git compatibility | Every brit repo is a valid git repo; `git clone` from any forge works |

## The Reach Bridge (Shared with Rakia)

Reach is the concept that bridges brit and rakia most directly:

**In brit:** Reach is per-ref. A branch at `reach=trusted` means its content is visible to trusted peers. Merging to main is reach-elevation from `trusted` to `public`.

**In rakia:** Reach is per-artifact. A build at `reach=self` is a local build. At `reach=trusted` it's CI-verified. At `reach=community` it's staging-verified. At `reach=public` it's production-deployed.

**The bridge:** Both are reach-elevation proposals. Both require attestation accumulation. Both flow through the protocol's governance surface. The governance system doesn't distinguish "code review approval" from "build verification" — both are witnessed claims accumulating toward a reach threshold. This is why brit and rakia share an attestation format.
