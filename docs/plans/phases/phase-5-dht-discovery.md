# Phase 5: Repos Discoverable on the Network

**Status:** Needs design session
**Depends on:** Phase 3 (libp2p transport — peers must be able to fetch over P2P)
**Capability unlocked:** DHT announcement of repo and commit CIDs. Peers find repos without central coordination. Build manifests are discoverable.

## Vision

A peer announces "I host repo X" to the DHT. Other peers discover repo X by its CID or name, find hosting peers, and fetch directly. No forge, no registry, no central index. The network IS the index.

This is the phase where brit repos stop depending on GitHub for discoverability. GitHub remains a valid mirror (web2 backward compat), but the protocol's own DHT is the primary discovery mechanism for peers inside the network.

## What Changes

- Repo CID + commit CIDs announced to DHT via existing Kademlia provider records
- Feed subscription: peers subscribe to repos they steward, receiving new commit notifications
- Peer capability advertisement: "I host repo X at branches Y,Z with reach levels A,B"
- `brit clone epr:{repo-cid}` — clone by EPR reference, resolving hosting peers via DHT
- Two-peer test: peer A announces repo, peer B discovers and clones without knowing A's address

## What This Unlocks for Rakia

- Build manifests discoverable via DHT — rakia Stage 2 (Canopy) needs peers to find manifests to build without central coordination
- Artifact CIDs discoverable — built artifacts can be found and fetched by any peer

## What This Unlocks for the Protocol

- Repos are truly decentralized — no forge dependency for discovery or hosting
- The onboarding flywheel completes: discover via doorway (web2) -> clone via P2P -> host and announce
- Foundation for Phase 6 (fork discovery — forks are discoverable as related repos)

## Prerequisites

- Phase 3 complete (P2P fetch works between peers)
- elohim-storage DHT infrastructure stable (Kademlia provider records). Source of truth for repo hosting: DHT provider records (who hosts what). Source of truth for repo content: the git object store on hosting peers.
- Feed subscription protocol operational (`/elohim/feed/1.0.0`)

## Sprint Sketch

1. **DHT announcement** — announce repo CID + branch head CIDs on commit/push
2. **DHT discovery** — resolve repo CID to hosting peers, select best peer, initiate fetch
3. **Feed subscription** — subscribe to repo updates, receive new commit notifications
4. **EPR-based clone** — `brit clone epr:{repo-cid}` resolves through DHT
5. **Multi-peer redundancy** — repo hosted by N peers, fetch negotiates with closest/fastest
