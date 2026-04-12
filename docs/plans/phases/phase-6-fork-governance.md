# Phase 6: Forks Are Governance Acts

**Status:** Needs design session
**Depends on:** Phase 4 (branch governance — protection rules must work), Phase 5 (DHT discovery — forks must be discoverable)
**Capability unlocked:** A fork is a ContentNode with its own stewardship, attestations, and peers — a legitimate alternate lineage, not a second-class copy. Cross-fork merges via qahal consent.

## Vision

Today, forking on GitHub is a platform feature. The fork has no formal relationship to the parent beyond a URL. No one can tell, from the code alone, whether a fork is a legitimate community effort or a fly-by-night copy.

With brit, a fork is a **first-class covenant** — a `ForkContentNode` with its own stewardship, its own attestations, its own peers. The code is the same; the stewardship graph is different. When you choose to depend on Community Fork X instead of Corporate Original Y, that choice is visible on the protocol's content graph. Your dependency isn't just a semver string in a lockfile — it's an EPR reference that points at specific stewards, specific attestations, specific governance.

## What Changes

- `ForkContentNode` fully implemented: parent repo link, fork reason, new stewardship, new governance
- `brit fork --as <new-url>` creates the fork, registers it on the network, announces via DHT
- Cross-fork merge: propose merging changes from fork A back into parent B, requiring qahal consent from B's stewards
- Fork discovery: "show me all forks of repo X" via DHT traversal
- Fork legitimacy signals: attestations accumulate on a fork independently from the parent

## What This Unlocks for Rakia

- Forked build manifests with independent stewardship — a community maintains its own build recipes
- Build recipe divergence and reconvergence — fork a manifest, improve it, merge back

## What This Unlocks for the Protocol

- The "Coop AWS" scenario from the README: choose which stewardship lineage you trust, with that choice protocol-legible
- Feature requests as forks: a non-developer proposes a change by forking, modifying, and proposing a merge-back
- Legitimate lineage diversity: N forks of the same codebase, each with different governance and stewardship, all visible on the content graph

## Prerequisites

- Phase 4 complete (branch-level governance, protection rules)
- Phase 5 complete (DHT discovery — forks must be findable)
- Governance gateway operational (qahal consent for cross-fork merges). Source of truth for fork identity: the ForkContentNode CID in the DHT. Source of truth for fork content: the git object store in the fork's hosting peers.
- MergeProposalContentNode lifecycle stable (from Phase 0+1 critique resolution)

## Sprint Sketch

1. **ForkContentNode implementation** — create, serialize, announce, discover
2. **Fork-aware merge** — MergeProposalContentNode that crosses repo boundaries
3. **Cross-fork consent** — qahal consent from both fork and parent stewards
4. **Fork graph traversal** — "show me the lineage tree of this repo" via DHT
5. **Attestation independence** — fork attestations don't inherit from parent; each lineage builds its own trust

## The Coop AWS Scenario

This phase enables the thought experiment from brit's README:

> Imagine a critical piece of infrastructure built by a corporation that starts doing things you disagree with. You fork the repo. With brit, the fork is a first-class covenant — a new ForkContentNode with its own stewardship, attestations, and peers. Everyone on the graph can see which collective you're trusting, and every steward can independently attest that the tags and branches they serve have the integrity needed for deployment.

The fork isn't a defection. It's a new covenant, legitimately grown from the old one. The protocol makes the governance visible so people can choose their stewards with full information.
