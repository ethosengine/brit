# Phase 4: Branches Tell Their Story

**Status:** Needs design session
**Depends on:** Phase 2 (ContentNode adapter — branches must be ContentNodes)
**Capability unlocked:** Every branch is a ContentNode with audience, governance, and purpose. Per-branch READMEs resolve as EPRs. Build status per branch is visible.

## Vision

A branch isn't just a ref pointer — it's a ContentNode that tells a story. `main` tells users one story ("this is the stable release"). `dev` tells developers another ("this is where active work lands"). `feature/x` tells what x unlocks for the learning platform. Each branch carries:

- **Reach level** — who sees this branch's content
- **Audience** — who this branch is for (learners, developers, stewards)
- **Protection rules** — what governance applies to changes
- **README as EPR** — a content-addressed description that resolves through the doorway

When rakia builds an artifact, the branch's reach level determines the artifact's initial reach. A build from `dev` starts at `reach=trusted`. A build from `main` starts at `reach=public` (if attestation thresholds are met).

## What Changes

- `BranchContentNode` fully implemented with reach, protection rules, and README slot
- `.brit/README.epr` (or equivalent) per branch — resolves as EPR through doorway
- Branch creation (`brit branch`) produces a ContentNode, not just a git ref
- Doorway serves per-branch views: reach level, protection status, recent commits with pillar summaries
- `brit status` shows branch reach level and protection rule summary

## What This Unlocks for Rakia

- Build status per branch: "this branch's latest build is attested at reach=trusted"
- Reach-aware promotion: artifacts inherit their branch's reach as starting point

## What This Unlocks for the Protocol

- Branches become navigable content in elohim-app — learners can explore what's being built and why
- Governance is visible per-branch — who can merge, what attestations are required
- Foundation for Phase 6 (fork governance — forks need branch-level governance to work)

## Prerequisites

- Phase 2 complete (BranchContentNode type exists and is addressable)
- Protection rules format designed (entangled with merge consent from Phase 0+1 critique)
- Doorway API for serving branch views

## Sprint Sketch

1. **Branch reach model** — implement reach-per-ref in brit-epr, validate against protocol reach enum. Source of truth for reach assignment: the BranchContentNode in the DHT; git refs carry the content, not the governance metadata.
2. **Per-branch README** — `.brit/README.epr` authoring, EPR resolution through doorway
3. **Protection rules** — per-branch governance rules as ContentNode, resolved from qahal layer
4. **Doorway branch views** — API endpoint serving branch metadata, reach, protection status
5. **Integration with rakia** — branch reach feeds into rakia's artifact reach annotation
