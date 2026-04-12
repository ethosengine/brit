# Phase 3: Git Over libp2p

**Status:** Needs design session
**Depends on:** Phase 2 (ContentNode adapter — repos/commits must be addressable by CID)
**Capability unlocked:** Clone, fetch, and push over the protocol's P2P network. No GitHub required. Rakia-peer can share the transport layer.

## Vision

A brit repo can be cloned from another peer over libp2p, with no centralized forge in the path. The doorway serves as a bridge for web2 clients, but peer-to-peer git operations work directly. The protocol's existing libp2p infrastructure (swarm, discovery, NAT traversal) carries git's pack protocol wrapped in brit's content-addressed framing.

## What Changes

- `brit-transport` crate: wires gix-protocol to libp2p request-response
- New protocol: `/brit/fetch/1.0.0` — same 4-byte BE length + MessagePack framing as existing shard/sync protocols
- Clone a small repo over libp2p (two-peer test)
- Push negotiation: "I have commits X,Y,Z; which do you need?" using CIDs, not just git SHA negotiation
- Doorway acts as a relay for clients that can't speak libp2p directly

## What This Unlocks for Rakia

- `rakia-peer` can share transport with brit's fetch protocol — build manifests distributed over the same P2P layer as code
- Manifest distribution doesn't require GitHub — a peer can discover and fetch build manifests from the network directly

## What This Unlocks for the Protocol

- Repos are hostable by any peer, not just GitHub/GitLab
- The onboarding flywheel: clone from doorway (web2), then progressively shift to P2P as the peer establishes connections
- Foundation for Phase 5 (DHT-based repo discovery)

## Prerequisites

- Phase 2 complete (ContentNode adapter — CIDs exist for commits and trees)
- rust-ipfs available as submodule (libp2p transport primitives)
- elohim-storage/steward P2P infrastructure stable (swarm, discovery). Source of truth for peer identity: agent keys in the DHT.

## Sprint Sketch

1. **Protocol definition** — `/brit/fetch/1.0.0` wire format, request/response types, MessagePack framing
2. **gix-protocol adapter** — bridge gix's pack negotiation to libp2p request-response
3. **Two-peer clone** — clone a small repo from peer A to peer B over libp2p
4. **Push support** — peer A pushes new commits to peer B
5. **Doorway relay** — web2 clients fetch via doorway which proxies to P2P peers

## Risks

- **Pack protocol complexity** — git's negotiation protocol (want/have/done) is stateful. Mapping it to request-response may require multi-round exchanges.
- **NAT traversal** — peer-to-peer git requires peers to be reachable. Relies on the existing relay/DCUTR infrastructure.
