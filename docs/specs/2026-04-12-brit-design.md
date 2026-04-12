# Brit Design Specification

**Date:** 2026-04-12
**Status:** Approved
**Author:** Matthew Dowell + Claude Opus 4.6
**Schema reference:** `docs/schemas/elohim-protocol-manifest.md` (1700+ lines, 15 sections)

## TL;DR

Brit is an expansion of gitoxide that makes version control covenantal. Every commit carries three-pillar metadata (lamad/shefa/qahal) as RFC-822 trailers that survive `git clone` from any forge. The engine (brit-epr) is generic — it parses trailers and dispatches to a pluggable AppSchema. The Elohim Protocol vocabulary is one schema implementation behind a feature flag. Schema-driven development: JSON Schema files define all ContentNode types; Rust types are generated from them.

## 1. Problem

Git tracks content but not value, governance, or provenance beyond author/email. Open-source projects live at the intersection of knowledge (informational), contribution (economic), and maintainer decisions (governance), but git knows about none of it. Contributors are invisible. Governance is implicit. Value extraction is structurally easy; value recognition is structurally absent.

## 2. Vision

A commit is a witnessed agreement whose terms travel with the code. A merge is a covenantal joining of lineages. A fork is a legitimate new covenant, not a defection. Branches carry reach levels that govern who sees what. The protocol's three-pillar coupling — lamad (knowledge), shefa (value), qahal (governance) — is embedded at the commit level, not bolted on afterward.

Every brit repo is also a valid git repo. `git clone` works from any forge. Inside the Elohim Protocol network, the same repo resolves to a richer view: linked ContentNodes, attestation graphs, per-branch governance, and content-addressed links that know where your code is running.

## 3. Architecture

### 3.1 Engine vs. App Schema (the foundational boundary)

| Layer | Crate | Owns | Does NOT own |
|---|---|---|---|
| **Engine** | `brit-epr` | Trailer parsing/serialization, generic validation, AppSchema trait, CID utilities, signing adapter hooks, commit round-trip | Any pillar vocabulary, ContentNode types, signal taxonomy, network transport |
| **App Schema** | `brit-epr` behind `#[cfg(feature = "elohim-protocol")]` | Pillar trailer keys, value grammar, ContentNode catalog, signal catalog, protection rules format | Trailer parsing mechanics, CID computation, commit object manipulation |

The boundary is load-bearing: every symbol behind the feature flag is brit-as-a-protocol-app, not brit-as-a-covenant-engine. Removing the feature must leave a working git tool.

### 3.2 AppSchema trait

```rust
trait AppSchema {
    fn id() -> SchemaId;
    fn owns_key(key: &str) -> bool;
    fn required_keys() -> &'static [&'static str];
    fn validate_pair(key: &str, value: &str) -> Result<(), ValidationError>;
    fn validate_set(trailers: &TrailerSet) -> Result<(), ValidationError>;
    fn cid_bearing_keys() -> &'static [&'static str];
    fn allowed_target_types(key: &str) -> &'static [ContentNodeTypeId];
    fn render(trailers: &TrailerSet) -> String;
    fn signals_for(commit: &CommitView) -> Vec<Signal> { vec![] }
}
```

### 3.3 Schema-driven types

JSON Schema files in `schemas/elohim-protocol/v1/` define all ContentNode types, trailer key grammar, and enum vocabularies. Rust types are generated from these schemas. A validation harness (`tests/schema_contract.rs`) asserts that published Rust structs serialize to JSON that validates against their schema.

### 3.4 The four framings

These are non-negotiable constraints from the schema manifest (§1.2):

1. **LLM-first CLI.** The primary user of `brit commit` is an LLM agent. Humans use the elohim-app UI for review and consent. Command names mirror git (use training as cognitive carrier).
2. **Backward-compatible with stock git.** Any brit repo is a valid git repo. Trailers are RFC-822 lines in commit messages. `.brit/` config files travel with the repo.
3. **Build system substrate.** Brit provides the VCS layer that rakia builds on. BuildManifest and BuildAttestation are reserved ContentNode slots.
4. **Feature-module boundary.** brit-epr is the engine; elohim-protocol is one app schema. A downstream fork disables the feature and writes their own schema.

## 4. ContentNode Catalog

Defined in schema manifest §5. Each type is content-addressed (CID over DAG-CBOR canonical serialization) and carries all three pillar fields (present but possibly empty with rationale).

| Type | Purpose | Content-address strategy |
|---|---|---|
| `RepoContentNode` | Top-level repo envelope | CID of `{repo_id, genesis_commit_cid, created_at, stewardship_agent}` |
| `CommitContentNode` | Covenantal commit | Dual: git object id + CID of CommitContentNode |
| `TreeContentNode` | Directory snapshot | CID of `{entries}`, stable under reorder |
| `BlobContentNode` | File content | CID of raw content bytes |
| `BranchContentNode` | Branch with reach + governance | CID of `{name, repo, reach, head, protectionRules}` |
| `TagContentNode` | Annotated tag | CID of `{name, target, tagger, annotation}` |
| `RefContentNode` | Lightweight ref | CID of `{name, target, repo}` |
| `RefUpdateContentNode` | Ref movement event | CID of `{ref, old_target, new_target, authorization}` |
| `ForkContentNode` | Legitimate fork as new covenant | CID of `{parent_repo, new_repo, fork_reason, steward}` |
| `DoorwayRegistration` | Web2 bridge pointer | CID of `{doorway_url, repo, steward_signature}` |
| `PerBranchReadme` | Branch-level README as EPR | CID of `{branch, readme_epr}` |
| `MergeProposalContentNode` | Async merge consent lifecycle | CID of `{source, target, frozen_requirements, ttl}` |
| *`BuildManifestContentNode`* | *(reserved for rakia)* | Defined by rakia's schema |
| *`BuildAttestationContentNode`* | *(reserved for rakia)* | Defined by rakia's schema |

## 5. Commit Trailer Specification

Six pillar trailer keys (three inline summary + three linked-node CID):

| Key | Format | Required | Example |
|---|---|---|---|
| `Lamad:` | `verb claim [modifiers]` | Yes | `demonstrates per-hunk witness card rendering \| path=brit/merge-ui` |
| `Shefa:` | `actor-kind contribution-kind [modifiers]` | Yes | `agent code \| effort=medium \| stewards=agent:matthew` |
| `Qahal:` | `auth-kind [modifiers]` | Yes | `steward \| ref=refs/heads/dev \| mechanism=solo-accept` |
| `Lamad-Node:` | CID | No | `bafkrei...` |
| `Shefa-Node:` | CID | No | `bafkrei...` |
| `Qahal-Node:` | CID | No | `bafkrei...` |

Reserved keys for future phases: `Built-By:`, `Brit-Schema:`, `Reviewed-By:`.

Vocabulary is closed within the app schema. Extensibility is via the feature-module boundary (different app = different schema), not by adding enum values at runtime.

## 6. CLI Command Surface

Git-analogous, LLM-first. New verbs only when no git command maps:

| New verb | Purpose |
|---|---|
| `brit merge` | Opens MergeProposalContentNode (async-default with `--wait` escape hatch) |
| `brit fork` | Creates ForkContentNode with independent stewardship |
| `brit attest` | Unified attestation + consent surface (code review, build attest, merge consent) |
| `brit verify` | Schema validator across commit range |
| `brit register-doorway` | Write/update `.brit/doorway.toml` |
| `brit set-steward` | Update repo stewardship |

All other commands (`commit`, `log`, `branch`, `push`, `pull`, `show`, `blame`) extend git equivalents with pillar awareness. Pass-through commands (`reset`, `revert`, `cherry-pick`, `stash`, `rebase`, `gc`, `config`) are unchanged.

## 7. Signal Taxonomy

Brit emits protocol signals when witnessed events occur. Signals are gossipped through the DHT. Categories:

- **Repo lifecycle:** `repo.created`, `repo.stewardship.changed`, `repo.archived`
- **Commit lifecycle:** `commit.witnessed`, `commit.superseded`
- **Branch lifecycle:** `branch.created`, `branch.reach.changed`, `branch.deleted`
- **Merge lifecycle:** `merge.proposed`, `merge.consented`, `merge.rejected`, `merge.completed`, `merge.expired`, `merge.withdrawn`
- **Attestation:** `attestation.published`
- **Fork lifecycle:** `fork.created`, `fork.merge-back.proposed`

## 8. Phase Decomposition

Seven phases, decomposed by **protocol capability** (not by crate). Each phase unlocks something for the protocol. Phases 0+1 are complete. Phases 2-6 have summary plans.

| Phase | Capability | What the protocol gains | Status |
|---|---|---|---|
| **0+1** | Pillar trailers parse and validate | Every commit can be checked for three-pillar compliance. `brit verify` works. | **Complete** (2026-04-12) |
| **2** | Git artifacts become ContentNodes | Repos, commits, branches, trees are addressable protocol content. Rakia can emit BuildManifestContentNode. | Summary plan |
| **3** | Git over libp2p | Clone, fetch, push over the protocol's P2P network. No GitHub required. Rakia-peer can share transport. | Summary plan |
| **4** | Branches tell their story | Per-branch READMEs as EPRs. Build status per branch visible. Each branch is a ContentNode with audience, governance, and purpose. | Summary plan |
| **5** | Repos discoverable on the network | DHT announcement of repo and commit CIDs. Peers find repos without central coordination. Build manifests discoverable. | Summary plan |
| **6** | Forks are governance acts | Fork as ContentNode with stewardship. Cross-fork merges via qahal consent. Community build recipes diverge and reconverge. | Summary plan |

See `docs/plans/phases/` for individual phase summaries.

## 9. Key Design Decisions

Captured from the schema manifest and merge consent critique:

1. **Reach is per-ref, not per-commit.** Branches hold reach; commits inherit from refs they're reachable from. This makes reach governance tractable — you govern the branch, not every commit individually.

2. **Vocabulary is closed within an app schema.** Extensibility is via the feature-module boundary. A different app = a different schema with its own vocabulary. No runtime enum extension.

3. **Merge consent is async-default.** `brit merge` opens a MergeProposalContentNode with a TTL. The proposal is a persistent governance artifact, not a CLI command's return value. The LLM opens a proposal, gets an ID back, and the proposal lives in the protocol until it terminates.

4. **Brit doesn't own governance.** Brit reads consent requirements from the parent EPR's qahal context. The governance gateway (elohim-storage) handles tally logic, delegation, and settlement. Brit publishes proposals and executes results.

5. **Dual CID for commits.** Every commit has both a git object id (SHA-1/SHA-256) and a ContentNode CID (CIDv1 over DAG-CBOR). Stock git tools see the former; protocol tools see either. This duality is load-bearing for backward compatibility.

6. **Trailer is the protocol surface; linked node is the graph surface.** If a reader has only the commit, they can tell whether it's pillar-compliant. The linked ContentNode enriches the view but isn't required for validation.

7. **Key recovery is the protocol's social recovery substrate, not brit's concern.** Agent key management, recovery, and rotation belong to the identity layer (imagodei pillar), not to version control.

8. **LLM authoring via skill + template.** `.claude/skills/brit/SKILL.md` teaches the LLM the pillar grammar. `.brit/commit-template.yaml` carries repo-specific defaults and enum hints. The hard parts of pillar metadata authoring are pushed into tooling, not into the LLM's prompt.

## 10. What's Deferred

| Item | Deferred to | Why |
|---|---|---|
| Protection rules DSL | Phase 2-4 | Shape of protection rules affects merge consent; needs design session |
| Cross-fork merge negotiation | Phase 6 | Requires fork governance model |
| Signed commits (GPG/SSH/agent) | Phase 2+ | Signing adapter hooks exist; implementation needs agent key infrastructure |
| `brit rebase` signal emission | Open question | Should rebase emit `commit.superseded`? Needs decision. |
| Force-push semantics | Phase 4+ | Mostly dissolved into reach-change governance with optional `extraProtectionRules` |
| Dynamic template enrichment | Phase 2+ | Requires doorway to be reachable and serving enrichment API |

## 11. Open Questions for Human Judgment

1. **Schema codegen tooling.** Same question as rakia: use the monorepo's existing codegen pipeline, or brit's own? Brit vendors schemas, so it needs its own build-time codegen.

2. **When does `brit-epr-elohim` become its own crate?** Currently behind a feature flag in `brit-epr`. Phase 2 (ContentNode adapter) may grow it enough to justify extraction. Decision deferred to Phase 2 design session.

3. **rust-ipfs as submodule timing.** Brit will use rust-ipfs for CID computation and blockstore. When does it become a submodule? Phase 2 (ContentNode adapter needs DAG-CBOR serialization) or Phase 3 (libp2p transport)?

4. **Skill file bundling.** Should `.claude/skills/brit/SKILL.md` be committed to the brit repo or provided externally by the LLM harness? Lean: bundled for discoverability, with harness override.
