# Brit — Elohim Protocol App Schema (Manifest)

**Status:** Draft v0.1 — exploration
**Targets protocol version:** `elohim-protocol/1.0.0` (pre-release)
**Owner:** brit maintainers
**Last updated:** 2026-04-11

---

## 1. Header & framing

### 1.1 What this document is

This is the **app-level schema manifest** for brit's integration with the Elohim Protocol. It is not a specification of the Elohim Protocol itself — that lives separately, in the protocol's own schema repository. This document is brit's answer to the question: *"If every artifact in the protocol is a ContentNode with three-pillar coupling, what are the ContentNodes of a distributed version control system, and what do their pillars say?"*

Brit is a fork of [gitoxide](https://github.com/GitoxideLabs/gitoxide) that adds covenantal semantics on top of git. The name (בְּרִית, "covenant") is deliberate: a commit in brit is not just a hash-linked snapshot, it is a witnessed agreement whose terms — what was learned, what value flowed, who consented — travel with the commit itself.

This document defines:

1. How brit-native git concepts (repos, commits, branches, trees, blobs, tags, forks, refs) map to **ContentNode** types in the Elohim Protocol vocabulary.
2. What each of those ContentNode types carries in its **three pillars** (lamad / shefa / qahal).
3. The **commit-trailer format** that is the canonical surface of the hybrid (c) design — the RFC-822 "Key: value" lines that make every brit commit legible to both stock git and the EPR graph.
4. How linked ContentNodes (addressed by CID from the trailer) resolve, validate, and degrade when unavailable.
5. What **protocol signals** brit emits as repositories change.
6. The **feature-module boundary** that keeps brit-epr (the trailer engine) usable as a generic substrate, while `elohim-protocol` is just one app schema that plugs into it.

### 1.2 How to read this document

- Readers who want the minimum viable understanding should read §2 (engine/app split), §4 (trailer spec), and §6 (signals). These are load-bearing for Phase 0–1 of the roadmap.
- Readers implementing a specific ContentNode type should jump to the relevant subsection of §3 and then read §5 (linked-node resolution).
- Readers evaluating whether brit-epr could host their own app schema (not elohim-protocol) should read §2 and §7.
- Readers looking for places where this document is deliberately underspecified should read §8.

### 1.3 Target protocol version

This manifest targets `elohim-protocol/1.0.0`, which is the version being stabilized as this document is written. The schema versions are themselves EPRs, so this manifest does not hard-code the protocol revision — it references protocol types by name and expects resolution against whichever protocol version the node has loaded. A future revision of this document will add a `ProtocolVersion:` trailer token once the protocol's own versioning story is crisp.

### 1.4 Terminology

| Term | Meaning in this document |
|---|---|
| **ContentNode** | The protocol's universal content envelope. Has id, contentType, title, description, content, contentFormat, tags, relatedNodeIds, and pillar fields. Every notarized artifact in the protocol is (or decomposes to) ContentNodes. |
| **EPR** | Elohim Protocol Reference. Canonical content address: `epr:{id}[@version][/tier][?via=][#fragment]`. |
| **Tier** | One of Head (~500B, DHT-gossipped), Document (~5-50KB, peer-cached), Bytes (arbitrary, shard-delivered). |
| **Pillar** | One of lamad (knowledge), shefa (value), qahal (governance). Every ContentNode carries all three; blanks are explicit, not implicit. |
| **Trailer** | RFC-822-style `Key: value` line at the end of a git commit message. `git interpret-trailers`-compatible. |
| **Linked node** | A ContentNode whose CID is referenced from a commit trailer. Optional; the trailer's inline summary is always authoritative. |
| **CID** | Content identifier per multiformats CIDv1. Brit prefers BLAKE3; accepts SHA-256 on input. |
| **brit-epr** | The feature-gated Rust module/crate(s) implementing the trailer engine and ContentNode adapter. |
| **elohim-protocol app schema** | The specific vocabulary described in this document. One possible app schema; brit-epr is designed to host others. |

---

## 2. Separation of concerns — brit-epr engine vs. elohim-protocol app schema

### 2.1 The reframing

In earlier drafts of the Phase 0 plan, "brit-epr" was both the trailer parser and the protocol vocabulary. That conflation is wrong. Trailer parsing is generic — any app that wants to carry structured metadata in commit messages faces the same parsing and validation problems. The pillar vocabulary (what keys exist, what values are legal, what linked-node types are valid) is specific to the elohim-protocol app schema.

**Reframe:**

- **brit-epr** is an *engine*. It parses, validates, and round-trips RFC-822 trailers in git commits. It knows nothing about lamad, shefa, or qahal. It exposes a trait that app schemas implement.
- **elohim-protocol** is an *app schema*. It implements the trait. It declares the pillar key names, value formats, linked-node type constraints, ContentNode catalog, and signal taxonomy described in this document.
- A third party could fork brit, disable the `elohim-protocol` feature, and implement `brit-epr-acme` with their own trailer keys, ContentNode catalog, and signals. The engine would not care.

This separation matters because the engine-level work (trailer parse/serialize, commit round-trip, validator scaffolding) is upstreamable to gitoxide in the long run. The app schema is brit's opinionated covenant and stays in brit.

### 2.2 Engine responsibilities (brit-epr crate)

The engine owns:

1. **Trailer parsing** — walking a commit's body, finding the trailer block, splitting into `(key, value)` pairs. In gitoxide terms, this wraps and extends `gix_object::commit::message::BodyRef::trailers()`.
2. **Trailer serialization** — given an ordered set of `(key, value)` pairs, writing the trailer block back into a commit message in a form that round-trips through stock git, `git interpret-trailers`, and `git rebase`.
3. **Generic validation** — key shape (ASCII token), value constraints (no embedded newlines unless explicitly continuation-indented, length caps, CR/LF normalization), duplicate-key policy.
4. **Schema dispatch** — looking up which app schema owns which keys, delegating semantic validation to the schema.
5. **CID parsing/formatting** — multiformats CIDv1 parse, display, kind/codec check. Engine-level because multiple app schemas will carry CIDs; the protocol for spelling them is stable.
6. **Signing adapter hooks** — surface for signed commits (GPG, SSH, minisign, agent attestation) so that app schemas can attach signatures to their linked nodes without the engine knowing the signing kind.

The engine does NOT own:

- The set of known keys (that's the schema's problem).
- The set of ContentNode types (that's the schema's problem).
- The signal taxonomy (that's the schema's problem).
- Network transport (lives in the future `brit-transport` crate).
- The storage backend (lives in the future `brit-store` crate, or in rust-ipfs via the substrate integration).

### 2.3 The engine-to-schema trait

Pseudocode only — no Rust below. The engine exposes something morally equivalent to:

```
trait AppSchema {
    // A stable identifier for the schema, e.g. "elohim-protocol/1.0.0".
    fn id() -> SchemaId;

    // Does this schema recognize this trailer key?
    fn owns_key(key: &str) -> bool;

    // Validate a single (key, value) pair in isolation (no cross-field rules).
    fn validate_pair(key: &str, value: &str) -> Result<(), ValidationError>;

    // Validate a whole trailer set together (cross-field rules, e.g.
    // "Lamad-Node: present requires Lamad: non-empty").
    fn validate_set(trailers: &TrailerSet) -> Result<(), ValidationError>;

    // Report which CID-bearing trailer keys exist so the resolver can walk them.
    fn cid_bearing_keys() -> &'static [&'static str];

    // For each CID-bearing key, what ContentNode type(s) is a valid resolution target?
    fn allowed_target_types(key: &str) -> &'static [ContentNodeTypeId];
}
```

The engine's public API takes an `&dyn AppSchema` (or monomorphizes over it via generic). The `elohim-protocol` feature-gated module provides the one implementation this crate ships with.

### 2.4 Why a feature flag, not just a separate crate

We expect brit to always ship with the elohim-protocol schema enabled in its default build. The feature flag is not there to make compilation smaller — it is there to make the *boundary legible*. Every symbol behind `#[cfg(feature = "elohim-protocol")]` is a symbol that is brit-as-a-protocol-app, not brit-as-a-covenant-engine. Someone reading the code should be able to tell at a glance: "if I remove this feature, do I still have a working git?" The answer must always be yes.

Additionally, a downstream fork that wants to write their own schema should be able to express "I want brit-epr but not elohim-protocol" in their `Cargo.toml` in one line, not by surgery on brit's source.

### 2.5 Crate layout (provisional)

| Crate | Purpose | Features |
|---|---|---|
| `brit-epr` | Engine. Trailer parse/serialize, generic validator, schema dispatch, CID utilities. | `elohim-protocol` (default on) — enables the app schema module. |
| `brit-epr-elohim` *(optional second crate)* | Pure app schema. Implements `AppSchema` for elohim-protocol. Consumed by brit-epr when the feature is on. | — |
| `brit-cli` | `brit-verify`, `brit-show-pillars`, `brit-inspect-trailers` binaries. Consumes brit-epr with the default feature. | — |

The "one crate with a feature" vs. "two crates where the feature is a re-export" choice is left open in §8. Either pattern honors the boundary; the two-crate form makes the boundary more obvious to tooling (cargo metadata, crates.io), while the one-crate form keeps Phase 0 scaffolding minimal.

---

## 3. ContentNode type catalog

This section enumerates every ContentNode type brit introduces. Each is addressed by a deterministic CID over its canonical serialization (DAG-CBOR, same as the rest of the protocol). Each declares its three-pillar couplings, relationships to other types, and the open questions that this exploration hasn't resolved.

A note on required vs. optional fields: every ContentNode type must answer all three pillars, but an answer of *"this artifact does not carry that pillar because X"* is a valid answer. The validator enforces that the pillar field is *present*, not that it is *non-empty*. An explicit `{"rationale": "infrastructure commit, no lamad dimension"}` is legal; an absent field is not.

### 3.1 RepoContentNode

**Purpose.** The top-level envelope for a brit repository. Every repo on the network is addressable by a stable id that is independent of any particular clone. This is the thing you point at when you say "give me *this* repo," regardless of which peer happens to host it today.

**Content-address strategy.** CID over the canonical serialization of `{repo_id, genesis_commit_cid, created_at, name, stewardship_agent}`. The repo's id is derived from its genesis commit and its original steward — forks get a new repo_id, not a branch of the same one. A rename of the repo does not change the id.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | CID | Self-address. |
| `contentType` | `"brit.repo"` | Literal. |
| `title` | string | Human-readable repo name. |
| `description` | string | One-paragraph summary; often derived from the top-level `.brit/README.epr` if present. |
| `genesisCommit` | CID of `CommitContentNode` | The first covenantal commit. |
| `currentHead` | map of ref name → CID of `CommitContentNode` | Snapshot at publish time. |
| `stewardshipAgent` | agent id | Who currently holds curation rights. |
| `lamad` | Lamad-pillar object | See below. |
| `shefa` | Shefa-pillar object | See below. |
| `qahal` | Qahal-pillar object | See below. |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `parentRepo` | CID of `RepoContentNode` | Present if this repo is a fork. |
| `forkReason` | string | Human-readable explanation of why the fork exists. |
| `relatedRepos` | array of CID | Sibling repos in a family (e.g., bindings, docs, examples). |
| `license` | SPDX id or CID of a license ContentNode | — |

**Lamad coupling — what does a repo teach?** A repo declares its *domain of knowledge*. For brit itself, that is "distributed version control for covenantal software." For a learning platform repo, it is whatever subject the platform covers. The lamad field of a repo is typically the anchor for a *learning path* — the repo's README, tutorials, and example walks are organized around a path that a newcomer follows. Concretely: `lamad.primaryPath` is a CID of a path ContentNode in the lamad vocabulary, and `lamad.unlocks` is an array of capability tags the reader gains by grokking the repo.

**Shefa coupling — what value flows?** A repo is a stewardship surface. Contributors earn standing by having their commits accepted into the steward's chosen refs (see §3.2 on commits). The repo's shefa field declares: who is the current steward, what is the resource kind of the repo (typically `code` or `text` or `schema`), and what economic events have happened at the repo level (adoption, fork, archival). It also declares whether the repo participates in an economic rail — e.g., whether contributions are tracked for later value distribution.

**Qahal coupling — what governance?** A repo declares its governance rules: who can merge to protected refs, what attestations are required, whether constitutional council review is needed for certain changes (e.g., license changes), and where the governance ContentNode for the repo lives. The qahal field's most important responsibility is naming *where* governance happens — the actual rules are a ContentNode of their own, resolved via CID. This keeps the RepoContentNode small enough to fit in an EPR Head tier.

**Relationships.**

- Outbound: → `CommitContentNode` (many, via `currentHead` map and genesisCommit); → `RepoContentNode` (at most one, via `parentRepo`); → `RepoContentNode` (many, via `relatedRepos`); → linked lamad/shefa/qahal ContentNodes.
- Inbound: ← `ForkContentNode` (many, children); ← `BranchContentNode` (many, because each branch is stewarded inside a repo).

**Open questions.**

- Does renaming a repo produce a new version of the RepoContentNode or a new repo? Strong lean: new version (id stable, version bumped).
- Is `currentHead` redundant with the refs projection? (See §3.7 on refs.) Lean: keep it in the head tier for fast snapshotting; the authoritative view is still the refs.
- Does the repo carry its DNS or web2 shadow name (e.g., `github.com/ethosengine/brit`) as a tag? Lean yes, as a hint for onboarding flow — with the explicit caveat that the shadow name is not authoritative.

---

### 3.2 CommitContentNode

**Purpose.** The covenantal commit. This is the central type in brit's vocabulary. It wraps the git commit object with the pillar couplings that make the commit a *witnessed agreement* rather than just a snapshot. Critically, the CommitContentNode is *not* stored instead of the git commit — it is stored *alongside*, and the git commit's trailers are the canonical summary (see §4).

**Content-address strategy.** Two CIDs exist for every commit:

1. The git object id (SHA-1 or SHA-256 per repo's configured hash), computed by gitoxide exactly as upstream git does.
2. The CID of the CommitContentNode itself, computed over its canonical serialization. The CommitContentNode carries the git object id as one of its fields, so the two are linked but not equal.

This duality is a load-bearing part of the hybrid design: stock git tools see the git object id, brit tools see either.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | CID of this CommitContentNode | — |
| `contentType` | `"brit.commit"` | Literal. |
| `gitObjectId` | hex git object id | What `git log` prints. |
| `repo` | CID of `RepoContentNode` | Which repo this commit belongs to. |
| `parents` | array of CID of `CommitContentNode` | Zero for root, one for linear, 2+ for merge. Ordered. |
| `treeRoot` | CID of `TreeContentNode` | The repo snapshot at this commit. |
| `author` | agent id + display name + timestamp | Mirrors git author. |
| `committer` | agent id + display name + timestamp | Mirrors git committer. |
| `messageSubject` | string | First line of the commit message. |
| `messageBody` | string | Remaining lines, excluding the trailer block. |
| `trailerSummary` | inline trailer key/value pairs | The exact string parsed out of the commit message. |
| `lamad` | Lamad-pillar object | See below. |
| `shefa` | Shefa-pillar object | See below. |
| `qahal` | Qahal-pillar object | See below. |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `signatures` | array of signature descriptors | GPG, SSH, minisign, agent attestation. Each has kind, signer id, signature bytes CID. |
| `lamadNode` | CID of a lamad ContentNode | Rich lamad context, see §5. |
| `shefaNode` | CID of a shefa ContentNode | Rich shefa events, see §5. |
| `qahalNode` | CID of a qahal ContentNode | Rich governance context, see §5. |
| `reviewedBy` | array of review attestations | Each carries an agent id, capability CID, timestamp, decision, optional note CID. |
| `supersededBy` | CID of `CommitContentNode` | Set when a commit has been rebase-dropped, amended, or force-pushed over. Historical breadcrumb. |

**Lamad coupling — what does a commit teach?** The lamad of a commit is a structured answer to "what does a reader learn by studying this diff?" For a feature commit, it might be `{"demonstrates": "how to wire a new behaviour into the libp2p swarm", "unlocks": ["libp2p-behaviour-composition"], "path": "brit/substrate-integration"}`. For a fix commit, it might be `{"corrects": "previously documented-but-wrong SLA path", "relatedMistake": <cid of a learning-from-failure node>}`. For infrastructure commits, `{"rationale": "ci-only change, no lamad"}` is valid.

The inline trailer form of lamad (see §4) is a short human-readable string like `Lamad: demonstrates libp2p behaviour composition`. The rich form is the `lamadNode`.

**Shefa coupling — what value flows?** The shefa of a commit records REA events triggered by the commit landing. At minimum: `{"author": <agent>, "contributionKind": "code|docs|test|schema|review|infra", "effort": <coarse bucket>, "stewardAccepting": <agent>}`. For commits that merge third-party contributions, the shefa field also carries the provenance — who submitted, through what flow, and whether any economic reward flows back to them.

For bots and machine commits (e.g., CI baseline updates), the shefa field is `{"contributorKind": "machine", "parentAgent": <the human or system who owns the bot>}`. Machine contributions do not earn economic standing for the machine itself; they pass through to the owning agent.

**Qahal coupling — what governance?** The qahal field records what collective authorized this commit. For a solo commit on a personal branch, `{"authorizedBy": "self", "ref": "refs/heads/personal/matthew/scratch"}`. For a commit landing on a protected ref, `{"authorizedBy": <governance node cid>, "mechanism": "consent|vote|attestation", "quorum": "...", "dissent": [...]}`. The dissent field is important: consent-based governance should carry the dissent record forward even when the decision was to merge.

**Relationships.**

- Outbound: → `CommitContentNode` (parents); → `TreeContentNode` (treeRoot); → `RepoContentNode` (repo); → linked lamad/shefa/qahal nodes (optional); → review attestation ContentNodes (optional).
- Inbound: ← `CommitContentNode` (children via parents); ← `BranchContentNode` (as head); ← `TagContentNode` (as target); ← `RefContentNode` (as pointed).

**Open questions.**

- How do we handle commits authored in stock git and later adopted into brit? They have no trailers. Lean: wrap them in a `CommitContentNode` with `lamad = {"provenance": "imported-legacy"}`, `qahal = {"authorizedBy": "retroactive-adoption", "adoptingSteward": <agent>}`. The trailer requirement is enforced for *new* brit commits, not for imported history.
- Do amended commits have a relationship to their pre-amendment version? Lean yes, via `supersededBy`, but git amend breaks the SHA so this is a best-effort breadcrumb.
- How do we handle rebases that rewrite history across a range? Lean: each rewritten commit gets `supersededBy` pointing at its new form; the old CommitContentNodes are still resolvable if anyone has them cached, but are flagged as historical.

---

### 3.3 TreeContentNode

**Purpose.** The repo snapshot at a particular commit. Mirrors a git tree object (directory entry list), but is content-addressed as a ContentNode with its own pillar fields. Most of the time the pillars of a tree are a passthrough from the commit; they exist so that individual trees (e.g., a `docs/` subtree) can carry their own lamad/shefa/qahal context for sub-repository stewardship.

**Content-address strategy.** CID over the canonical serialization of the tree's entries (name, mode, target CID, target type). When a brit repo is using git's native SHA hash, the tree's git object id and the TreeContentNode CID are separate addresses over the same logical content, the same way commits have two ids.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | CID | — |
| `contentType` | `"brit.tree"` | Literal. |
| `gitObjectId` | hex | What `git cat-file -p` would show. |
| `entries` | array of `{name, mode, target, targetType}` | Sorted by name. `targetType` is `"blob"` or `"tree"`. |
| `lamad` | Lamad-pillar object (may be `{inherit: "parent-commit"}`) | — |
| `shefa` | Shefa-pillar object (may be `{inherit: "parent-commit"}`) | — |
| `qahal` | Qahal-pillar object (may be `{inherit: "parent-commit"}`) | — |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `subRepo` | CID of a `RepoContentNode` | When this subtree is itself a sub-repo boundary (similar to submodules, but first-class). |
| `codeowners` | array of agent ids | Per-tree curation delegates. |

**Lamad coupling.** Usually `{inherit: "parent-commit"}`. Non-inherit values are meaningful for *sub-repo trees* (see §3.8 on forks and submodules) and for `docs/` subtrees that should be treated as their own learning path. A tree saying `lamad = {"pathAnchor": <cid>}` means "anyone who walks this subtree is walking this specific path."

**Shefa coupling.** Usually inherit. Non-inherit means "this subtree has its own stewardship accounting" — e.g., a `translations/` tree where translators earn standing independent of the main code contributors.

**Qahal coupling.** Usually inherit. Non-inherit means "this subtree has its own governance rules" — e.g., `docs/legal/` requires constitutional council consent to modify while the rest of the repo uses steward-accept governance.

**Relationships.**

- Outbound: → `TreeContentNode` (sub-directories); → `BlobContentNode` (files); optionally → `RepoContentNode` (subRepo).
- Inbound: ← `CommitContentNode` (as treeRoot); ← `TreeContentNode` (as a sub-entry).

**Open questions.**

- Can a tree have relatedNodeIds outside its subtree? Lean no — the tree should be content-addressable without reaching outside. Governance overrides live in qahal, which is a reference, not an embed.
- Does the tree carry an index of its subtree's CIDs for fast traversal? Lean no — that's a caching concern, not a schema concern.

---

### 3.4 BlobContentNode

**Purpose.** A file in a repo, wrapped as a ContentNode. Mirrors a git blob. Most blobs in a brit repo will carry minimal pillar metadata — blobs inherit from their parent tree/commit unless something in the file itself justifies separate pillar fields.

**Content-address strategy.** CID over the raw bytes. When the repo uses git's native hashing, the git blob id and the BlobContentNode CID are separate addresses over the same bytes.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | CID | — |
| `contentType` | `"brit.blob"` | Literal. |
| `gitObjectId` | hex | — |
| `size` | integer | Bytes. |
| `contentFormat` | string | Best-effort mime / format tag. `unknown` is legal. |
| `lamad` | Lamad-pillar object (default: `{inherit: "parent-tree"}`) | — |
| `shefa` | Shefa-pillar object (default inherit) | — |
| `qahal` | Shefa-pillar object (default inherit) | — |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `embeddedEpr` | CID | If this blob is itself an EPR-native artifact (e.g., a `.epr.json` rendered ContentNode), its canonical EPR id. |
| `binaryKind` | enum | `text | image | audio | video | executable | archive | other` — coarse classifier. |

**Lamad coupling.** Usually inherit. Blobs that *are* learning artifacts (tutorials, example notebooks, Gherkin feature files) should carry their own lamad. Convention: if the blob's path matches `**/*.feature`, `**/README*.md`, `docs/**/*.md`, the importer populates lamad non-trivially.

**Shefa coupling.** Usually inherit. Blobs that are the product of specific value flows (e.g., translated strings earning translation standing) carry their own shefa. Most don't.

**Qahal coupling.** Usually inherit. Blobs under governance-sensitive paths (`.gov/`, `LICENSE*`, `SECURITY.md`) carry explicit qahal fields.

**Relationships.**

- Outbound: → embedded EPR (optional).
- Inbound: ← `TreeContentNode` (as an entry).

**Open questions.**

- Should very large blobs be sharded into multiple ContentNodes automatically? The protocol's Bytes tier already handles this via the shard protocol, so lean no — a single BlobContentNode can point at a sharded payload without changing its own shape.
- Should binary blobs refuse pillar metadata entirely? Lean no — inheritance is the right default; it keeps the schema uniform.

---

### 3.5 BranchContentNode

**Purpose.** A branch is a stewarded view over a repo's history. In plain git, a branch is a mutable ref pointer. In brit, a branch is a ContentNode with its own lamad/shefa/qahal context — *"main tells users one story; dev tells developers another; feature/x tells what x unlocks"* (from the roadmap). The ref is the pointer; the BranchContentNode is the view.

This is one of the type distinctions that makes brit feel different from git. A branch in brit is not a lightweight pointer — it is a first-class witnessed surface.

**Content-address strategy.** The branch has a *stable id* (agent-scoped composite: `{repo_cid, branch_name, owning_agent}`) and a *versioned content-address* (CID over the current metadata, which changes each time the branch's head or pillar fields are updated). The stable id is how you reference "the main branch of this repo over time"; the versioned CID is how you pin a specific view of it.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | stable branch id | Not a CID — a composite. |
| `versionCid` | CID of this snapshot | Changes whenever the branch updates. |
| `contentType` | `"brit.branch"` | Literal. |
| `repo` | CID of `RepoContentNode` | — |
| `name` | string | Local branch name, e.g. `main`, `dev`, `feature/foo`. |
| `head` | CID of `CommitContentNode` | Current head. |
| `steward` | agent id | Who decides what lands on this branch. |
| `lamad` | Lamad-pillar object | See below. |
| `shefa` | Shefa-pillar object | See below. |
| `qahal` | Qahal-pillar object | See below. |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `readmeEpr` | CID of a `BlobContentNode` or lamad content | The per-branch README. Resolves to what readers of this branch see. |
| `protectionRules` | CID of a qahal governance node | What's required to merge to this branch. |
| `relatedBranches` | array of branch stable ids | Branches that travel together (e.g., `main` + `release/*`). |
| `abandoned` | boolean | True if steward has marked it no longer maintained. |

**Lamad coupling.** The lamad of a branch is its *audience and unlocks*. `main.lamad` might say `{"audience": "users", "primaryPath": "brit/getting-started"}`. `dev.lamad` might say `{"audience": "contributors", "primaryPath": "brit/developer-onboarding"}`. `feature/new-merge.lamad` might say `{"audience": "reviewers", "unlocks": ["p2p-merge-flow"]}`. This is how per-branch READMEs get their meaning.

**Shefa coupling.** The shefa of a branch is its *stewardship and cost*. Who is the steward, what resource events have they performed on this branch, what is the branch's affinity rating, how much attention does it consume. Abandoned branches have a shefa field indicating their resting state.

**Qahal coupling.** The qahal of a branch is its *protection and mechanism*. What must happen for a commit to land. Who can approve merges. Whether dissent on a merge blocks or is recorded. For `main` on a brit-substrate repo, qahal typically names a governance ContentNode with "requires steward + one other reviewer"; on a personal scratch branch, qahal might say `{"self-governance": true}`.

**Relationships.**

- Outbound: → `RepoContentNode`; → `CommitContentNode` (head); → readme blob/lamad node; → governance qahal node.
- Inbound: ← `RefContentNode` (a ref points at the branch); ← other `BranchContentNode`s (relatedBranches).

**Open questions.**

- Is `main`-vs-`dev` distinction part of the protocol or just a convention? Lean: convention. The schema doesn't special-case them; tooling does.
- Should branch rename be a new version of the same branch or a new branch? Lean: new version, because the stable id includes the name. Wait — that would make rename a new id. Correct answer: rename is a new branch whose `supersededBy` points at the old one. Stable id includes the name.
- Is the branch's `versionCid` tracked in the ref system or computed on read? See §3.7. Lean: computed on read but cached.

---

### 3.6 TagContentNode

**Purpose.** A brit tag is a covenantal attestation that a specific commit represents a specific release or milestone. Mirrors a git annotated tag. Unlike stock git, brit tags always carry pillar fields — because a release is an assertion to the community about what has been achieved.

**Content-address strategy.** CID over the tag's canonical serialization. Tags are immutable once created; re-tagging produces a new TagContentNode with a new id.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | CID | — |
| `contentType` | `"brit.tag"` | Literal. |
| `repo` | CID of `RepoContentNode` | — |
| `name` | string | e.g., `v1.2.0`. |
| `target` | CID of `CommitContentNode` | What the tag points at. |
| `tagger` | agent id + timestamp | — |
| `message` | string | Tag annotation. |
| `lamad` | Lamad-pillar object | See below. |
| `shefa` | Shefa-pillar object | See below. |
| `qahal` | Qahal-pillar object | See below. |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `releaseNotes` | CID of a lamad ContentNode | Rich release notes. |
| `signatures` | array of signature descriptors | Signed tags. |
| `supersededBy` | CID of `TagContentNode` | If this tag has been retracted and replaced. |
| `yanked` | boolean + reason CID | Release was pulled. |

**Lamad coupling.** A release tag declares what capabilities the release unlocks, what paths it advances, what prior knowledge is now obsolete. `v1.2.0.lamad = {"unlocks": ["per-branch-readmes"], "obsoletes": ["manual-trailer-parsing"]}`.

**Shefa coupling.** A release tag accumulates the shefa events of every commit between the previous release and this one, rolled up. It declares contributor credits, steward time, and any explicit value distributions tied to the release.

**Qahal coupling.** A release declares how it was authorized — release manager, release vote, automated on-merge-to-main, etc. Yanks carry their own qahal pointer to the decision that led to the yank.

**Relationships.**

- Outbound: → `CommitContentNode` (target); → `RepoContentNode`; → release notes, signatures, superseding tag.
- Inbound: ← `RefContentNode` (refs/tags/X); ← other `TagContentNode`s (supersededBy).

**Open questions.**

- Lightweight tags (git's non-annotated form) — reject, or wrap them as a minimal TagContentNode with inherited pillar fields from the target commit? Lean: wrap them with inheritance, but emit a warning during verify that brit-native tags should be annotated.
- Are pre-release suffixes (e.g., `v1.2.0-rc1`) a naming convention or a schema field? Lean: schema field `preReleaseKind: "rc" | "beta" | "alpha" | null` to make tooling simpler.

---

### 3.7 RefContentNode

**Purpose.** A ref is the authoritative *pointer*: a named entry in a namespace like `refs/heads/main`, `refs/tags/v1.2.0`, `refs/notes/brit`, `refs/brit/pipelines/app`. RefContentNode exists because in brit, the act of *moving a ref* is itself a governance event — it needs to be witnessed. A branch's current head isn't just "where the pointer is"; it's "where the steward last accepted a commit to be."

This separation between `BranchContentNode` (the view) and `RefContentNode` (the pointer) is the move that lets forks, mirrors, and stewardship transfers become first-class.

**Content-address strategy.** Refs form a *log*, not a single CID. Each ref update is a new `RefUpdateContentNode`, chained by parent. The "current ref CID" at any moment is the CID of the latest update. This is morally similar to git's reflog, but every entry is a witnessed ContentNode with pillar fields.

To save space: `RefContentNode` refers to the *identity* of a ref (its name and repo), while `RefUpdateContentNode` is one entry in its log.

**Required fields (RefContentNode).**

| Field | Type | Notes |
|---|---|---|
| `id` | composite (repo cid + ref path) | — |
| `contentType` | `"brit.ref"` | Literal. |
| `repo` | CID of `RepoContentNode` | — |
| `path` | string | e.g., `refs/heads/main`. |
| `currentUpdate` | CID of `RefUpdateContentNode` | Head of the log. |
| `kind` | enum | `head | tag | note | pipeline | custom` |

**Required fields (RefUpdateContentNode).**

| Field | Type | Notes |
|---|---|---|
| `id` | CID | — |
| `contentType` | `"brit.ref-update"` | Literal. |
| `ref` | composite ref id | — |
| `previous` | CID of prior RefUpdateContentNode, or null for first update | — |
| `from` | CID (commit/tag/etc.), null for create | — |
| `to` | CID (commit/tag/etc.), null for delete | — |
| `reason` | enum | `fast-forward | merge | force-push | create | delete | rebase | rename` |
| `actor` | agent id | Who moved the pointer. |
| `timestamp` | — | — |
| `lamad` | Lamad-pillar object | Usually inherited from the commit being pointed at. |
| `shefa` | Shefa-pillar object | The value event this ref move represents. |
| `qahal` | Qahal-pillar object | What authorized this ref move. Critical for force-push policy. |

**Lamad coupling.** Usually inherited from the target commit's lamad. For pipeline-ref updates (e.g., `refs/brit/pipelines/app` advancing to a new successful build commit), the lamad field names what pipeline-level capability the update represents.

**Shefa coupling.** Records the steward's resource event: "stewarded-merge", "stewarded-force-push", "steward-delete". Force-pushes are heavy-shefa events because they overwrite history.

**Qahal coupling.** This is the load-bearing pillar for refs. A force-push to `main` requires a much stronger qahal authorization than a fast-forward. The qahal field of a RefUpdateContentNode must satisfy the branch's `protectionRules` or the update is rejected. In brit's world, a ref update without qahal authority is a protocol violation, not a repository anomaly.

**Relationships.**

- Outbound: → `RefUpdateContentNode` (log head); → prior `RefUpdateContentNode` (previous); → target (from/to).
- Inbound: ← `RepoContentNode` (via its currentHead map); ← tooling that walks the log.

**Open questions.**

- Does the full log live in the DHT or only locally? Lean: log is local + peer-synced; the DHT carries only the current head and a compact Merkle digest of the log.
- How are force-pushes in "personal" ref namespaces handled — still witnessed, or exempted? Lean: always witnessed, but personal namespaces have `qahal = {self-governance: true}` so the witnessing is cheap.
- Should notes-refs be a distinct kind or just `kind: "note"` with the same schema? Lean: same schema, different kind tag.

---

### 3.8 ForkContentNode

**Purpose.** A fork is a legitimate alternate lineage — a new covenant grown from an old one, not a defection. This is the type that makes brit's governance story work. Forking is not an act of abandonment; it is an act of proposing a different answer to the same question. The ForkContentNode records the provenance, the reason, and the stewardship transfer so that forks can later *negotiate merges* with the parent on equal footing.

**Content-address strategy.** CID over the fork's canonical serialization: `{parent_repo, fork_repo, fork_point_commit, reason, steward_new, created_at}`.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | CID | — |
| `contentType` | `"brit.fork"` | Literal. |
| `parentRepo` | CID of `RepoContentNode` | — |
| `forkRepo` | CID of `RepoContentNode` | The new repo. |
| `forkPoint` | CID of `CommitContentNode` | The commit the fork diverges from. |
| `reason` | string or CID of qahal node | Why the fork was created. |
| `originalSteward` | agent id | The steward of the parent at fork time. |
| `newSteward` | agent id | The steward of the fork. |
| `lamad` | Lamad-pillar object | See below. |
| `shefa` | Shefa-pillar object | See below. |
| `qahal` | Qahal-pillar object | See below. |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `mergeBackAgreement` | CID of a qahal node | If the fork was created with an explicit agreement to merge back under conditions, that agreement lives here. |
| `relatedForks` | array of CID of `ForkContentNode` | Other forks from the same parent. |
| `healed` | CID of a merge commit | If this fork has been merged back into the parent, the healing commit. |

**Lamad coupling.** A fork declares what *different* knowledge trajectory it represents. `"fork because upstream is abandoning WebRTC signaling and we want to keep it working"` is a lamad statement. The fork's lamad field declares what path the fork advances that the parent doesn't, and vice versa.

**Shefa coupling.** A fork is a stewardship transfer event. The shefa field records how the new steward came to hold standing over the fork — was it assigned, claimed, earned? For friendly forks (e.g., new maintainer takes over with blessing), the parent's steward signs a qahal attestation. For hostile forks, the shefa field records the cost to the network of maintaining two lineages.

**Qahal coupling.** A fork has its own governance rules from the moment it exists. The qahal field of a ForkContentNode names two things: the *new* governance of the fork (inherited from the parent unless explicitly diverged), and the *cross-fork negotiation rules* that govern future merge-back conversations between the forks.

**Relationships.**

- Outbound: → parent `RepoContentNode`; → fork `RepoContentNode`; → `CommitContentNode` (forkPoint); → qahal agreement nodes.
- Inbound: ← parent repo's forks list; ← the fork repo's own `parentRepo` field.

**Open questions.**

- How are shallow clones distinguished from forks? Lean: a shallow clone is not a fork; it's a bandwidth optimization. A fork requires a ForkContentNode.
- What about mirrors (read-only peer caches of a repo)? Lean: mirrors are not forks; they are RepoContentNode instances that share the same repo_id but declare themselves as mirror-role in shefa.
- Is `healed` one healing commit or a set? Multiple merge-backs can happen over time. Lean: array of CID.

---

### 3.9 NoteContentNode *(optional, provisional)*

**Purpose.** Some metadata attaches to a commit *after* the commit is created — code review outcomes that weren't available at merge time, retroactive pillar annotations, bug reports that reference the commit. Git solves this with notes-refs (`refs/notes/*`). Brit inherits this pattern, and wraps each note as a ContentNode.

**Why "optional, provisional".** It's not clear yet whether notes are a distinct ContentNode type or a special case of a generic `AttestationContentNode` that lives in the protocol layer rather than brit's app schema. Keeping it in §3.9 for completeness; may move into the protocol layer.

**Minimum sketch.**

- `contentType: "brit.note"`
- `target`: CID of any brit ContentNode (usually a commit).
- `author`: agent.
- `body`: markdown or CID of rich content.
- Three pillars, typically inherit-from-target with overrides.
- Indexed via `refs/notes/*` refs, which are themselves RefContentNodes.

Revisit in Phase 2 design.

---

### 3.10 Cross-cutting: what's intentionally not a new type

For legibility, here is what is *not* a distinct ContentNode type in brit's catalog, and why:

| Concept | Why not a new type | Where it lives |
|---|---|---|
| Working directory | Ephemeral, not content-addressable. | Local filesystem, no ContentNode. |
| Index / staging area | Ephemeral; when you commit, it becomes a tree. | Local filesystem. |
| Stash | Local state that becomes a commit when published. | Local; publishable as a normal commit. |
| Pack files | Storage optimization, not semantic content. | Storage layer (rust-ipfs / git-pack). |
| Git hooks | Local policy; may be captured as qahal rules at the repo level. | Repo qahal node, not a separate type. |
| Submodules | For brit, a submodule boundary is a sub-`RepoContentNode` reference from a `TreeContentNode`. | TreeContentNode.subRepo. |

---

## 4. Commit trailer specification

This section is normative for any tool that writes or reads brit commit trailers.

### 4.1 Goals

1. Round-trip through stock git without rewriting. `git commit`, `git rebase`, `git interpret-trailers`, `git cherry-pick`, `git am`, `git format-patch` must all preserve the trailer block.
2. Be the authoritative *summary* surface — a commit whose linked ContentNodes are unavailable (offline, peer unreachable, GC'd) is still inspectable for its pillar commitments.
3. Be parseable by non-brit tooling with only an RFC-822-style scan. No base64, no JSON, no magic bytes. Boring wins.
4. Fail loudly: malformed trailers are rejected at write time by brit, and verify surfaces them at read time.

### 4.2 Trailer block location

The trailer block is the final contiguous block of `Key: value` lines in the commit message, preceded by at least one blank line separating it from the body. This matches `git interpret-trailers`'s definition exactly. Brit uses `gix_object::commit::message::BodyRef::trailers()` to locate it.

If a commit message has no trailer block, it has no brit pillar commitments — brit-verify rejects such a commit as non-compliant (with a configurable severity level; see §4.7).

### 4.3 Token namespace

All brit-introduced trailer keys use the prefix of the pillar name. The engine reserves no keys itself; every key listed below is owned by the elohim-protocol app schema.

| Key | Required | Purpose |
|---|:---:|---|
| `Lamad:` | yes | Inline summary of the lamad commitment. |
| `Lamad-Node:` | no | CID of a linked lamad ContentNode with rich context. |
| `Shefa:` | yes | Inline summary of the shefa commitment. |
| `Shefa-Node:` | no | CID of a linked shefa ContentNode. |
| `Qahal:` | yes | Inline summary of the qahal commitment. |
| `Qahal-Node:` | no | CID of a linked qahal ContentNode. |
| `Reviewed-By:` | no | Agent attestation: `<display> <agent-id> [capability=<cid>]`. Repeatable. |
| `Signed-Off-By:` | no | DCO-style author affirmation. Inherited from git convention. Not pillar-specific. |
| `Brit-Schema:` | no | App schema id in use, e.g., `elohim-protocol/1.0.0`. Defaults to `elohim-protocol/1.0.0`. |

Trailer keys NOT in this list that begin with a namespace prefix registered by another app schema are passed through unchanged — brit-epr's engine does not reject unknown keys; it delegates them to whichever schema claims them.

### 4.4 Value grammar

```
trailer-line  := key ":" SP value LF
key           := ALPHA *( ALPHA / DIGIT / "-" )
value         := <printable ASCII and UTF-8, no LF unless followed by continuation>
continuation  := LF SP ; leading SP on the next line continues the previous value
```

- Values are UTF-8. Non-ASCII is legal.
- Values are capped at **256 bytes** (pre-continuation folding) for pillar summary keys, **512 bytes** for CID-bearing keys (CIDs plus URI fragment), and **1024 bytes** for free-form trailers like `Reviewed-By:`.
- CR/LF normalization: brit writes LF only. Brit-verify accepts CRLF and normalizes for validation, but a write that would introduce CRLF is rejected.
- Duplicate keys: `Lamad:`, `Shefa:`, `Qahal:`, `Lamad-Node:`, `Shefa-Node:`, `Qahal-Node:`, `Brit-Schema:` must each appear **at most once**. `Reviewed-By:`, `Signed-Off-By:` are repeatable. Duplicates of single-valued keys are a *parse-level* error.
- Key order: the three pillar summary keys should appear in the canonical order (Lamad, Shefa, Qahal), with the corresponding Node keys immediately after each summary, though readers must accept any order.
- Value continuation: A line starting with a space after a key line is a continuation of the previous value. Folding at 80 columns is the brit-writer convention; readers must unfold before validating length.

### 4.5 Pillar-summary value microgrammar

The summary values are intentionally small and human-readable. They are not JSON — they are a flat `tag:verb microform` with optional free text. Pseudo-EBNF:

```
lamad-value := verb SP claim [ " | path=" path-slug ] [ " | unlocks=" id-list ]
verb        := "demonstrates" | "teaches" | "corrects" | "documents" | "imports" | "refactors-no-lamad"
claim       := <up to 200 bytes of free text, no | characters>
path-slug   := <slug matching [a-z0-9/-]+>
id-list     := <comma-separated slugs>

shefa-value := actor-kind SP contribution-kind [ " | effort=" effort-bucket ] [ " | stewards=" agent ]
actor-kind        := "human" | "agent" | "machine" | "collective"
contribution-kind := "code" | "docs" | "test" | "schema" | "review" | "infra" | "translation" | "data"
effort-bucket     := "trivial" | "small" | "medium" | "large" | "epic"

qahal-value := auth-kind [ " | ref=" ref-path ] [ " | mechanism=" mechanism ] [ " | dissent=" count ]
auth-kind   := "self" | "steward" | "consent" | "vote" | "attestation" | "council" | "retroactive"
mechanism   := <short tag>
ref-path    := <e.g., refs/heads/main>
```

The `|`-separated fragments are optional. The first segment (verb / actor-kind / auth-kind) is required.

Example well-formed trailer values:

```
Lamad: demonstrates wiring a libp2p behaviour for brit fetch | path=brit/substrate-integration | unlocks=libp2p-behaviour-composition
Shefa: human code | effort=medium | stewards=agent:matthew
Qahal: steward | ref=refs/heads/dev | mechanism=solo-accept
```

### 4.6 Linked-node key grammar

```
Lamad-Node: <cid>[#<fragment>]
Shefa-Node: <cid>[#<fragment>]
Qahal-Node: <cid>[#<fragment>]
```

The `<cid>` must be CIDv1 in multibase base32. The optional `#<fragment>` narrows to a sub-ContentNode within a composite node, matching the EPR URI scheme's fragment semantics.

The engine validates: it's a syntactically valid CIDv1 and the multicodec is in the allow-list of the app schema (for elohim-protocol: `dag-cbor`, `raw`, `dag-json`). The app schema validates: resolving the CID yields a ContentNode whose `contentType` is in the allowed-target set declared by `AppSchema::allowed_target_types()` — but this is a *resolution-time* check, not a *parse-time* check.

### 4.7 Validation levels — parser vs. validator

Brit distinguishes two layers of checking, and users should understand which layer catches which problem.

| Layer | Runs when | Rejects |
|---|---|---|
| **Parser** | Every commit read by brit-epr | Malformed trailer lines; duplicate single-valued keys; values exceeding hard length caps; invalid CID syntax in Node keys; key/value format violations. |
| **Schema validator** | `brit verify`, pre-commit hook, pre-push hook | Missing required pillar summary; verb/actor-kind/auth-kind not in enum; dangling Node CIDs (optional — see below); pillar cross-field inconsistencies (e.g., `Qahal: retroactive` without an `Approved-By:` attestation). |
| **Resolver** | When linked nodes are actually fetched | Wrong target type; resolution failure (reported as warning, not error, because offline is a legitimate state). |

The parser is strict because parse-level errors indicate a broken writer. The schema validator is strict about shape but lenient about availability because the protocol is offline-tolerant. The resolver is a *reporter*, not a *rejector*.

### 4.8 Example well-formed commit

```
Refactor merge conflict display to use per-hunk witness cards

The previous one-line-per-conflict rendering didn't leave room for
the pillar attribution on either side. Moving to per-hunk cards
surfaces the shefa contribution of each side's author and makes the
qahal consent flow legible at conflict time.

No behavior change for simple three-way merges.

Lamad: demonstrates per-hunk witness card rendering | path=brit/merge-ui
Lamad-Node: bafkreiabcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd
Shefa: human code | effort=medium | stewards=agent:matthew
Shefa-Node: bafkreishefa9876shefa9876shefa9876shefa9876shefa9876shefa98
Qahal: steward | ref=refs/heads/dev | mechanism=solo-accept
Reviewed-By: Jessica Example <agent:jessica> capability=bafkreicap1111cap1111cap1111cap1111cap1111cap1111cap1111cap1
Signed-Off-By: Matthew Example <matthew@example.org>
Brit-Schema: elohim-protocol/1.0.0
```

### 4.9 Examples of ill-formed commits

**Missing required pillar (schema-validator rejects, parser accepts).**

```
Fix a typo in the README.

Lamad: documents typo fix in README | path=brit/docs
Shefa: human docs | effort=trivial | stewards=agent:matthew
```

No `Qahal:` line — rejected by validator. Parser sees a valid trailer block.

**Duplicate single-valued key (parser rejects).**

```
Wire new protocol.

Lamad: demonstrates new protocol
Lamad: teaches new protocol
Shefa: human code
Qahal: steward
```

Two `Lamad:` lines — parser rejects. Never reaches the validator.

**Malformed CID in Node key (parser rejects).**

```
Add feature.

Lamad: demonstrates feature
Lamad-Node: not-a-cid
Shefa: human code
Qahal: steward
```

`not-a-cid` is not a valid CIDv1 multibase string — parser rejects.

**Unknown verb (validator rejects).**

```
Add feature.

Lamad: invents demo | path=brit/demo
Shefa: human code
Qahal: steward
```

`invents` is not in the verb enum — validator rejects with a hint.

**Trailer block fused with body (parser rejects or warns).**

```
Fix bug.
Lamad: corrects off-by-one
Shefa: human code
Qahal: steward
```

No blank line separating body from trailer — git's own trailer scan may or may not recognize this. Brit-verify treats this as a parser error; brit-write always emits the blank line.

### 4.10 What the canonical summary is *for*

The summary is not the graph — it is the *minimum viable witness*. It exists so that:

1. Stock git tools can see the commitments without any brit-specific software.
2. A verifier without network access can still tell whether a commit is pillar-compliant in shape.
3. The commit's content-address (git object id) covers the commitments in a tamper-evident way: if anyone rewrites a trailer, the commit hash changes, and every downstream commit notices.
4. The commit is still legible when the linked ContentNodes have been garbage-collected, replaced, or censored.

The linked node exists to carry the rich context that doesn't fit in a trailer value. It is the graph surface; the trailer is the protocol surface. When they disagree, the trailer wins — the linked node is an enrichment, not a source of truth for commitments.

---

## 5. Linked-node resolution

### 5.1 Resolution flow

When a brit tool encounters a trailer of the form `Lamad-Node: <cid>`, the flow is:

1. **Parse the CID.** If malformed, the parser has already rejected the commit; resolution never runs.
2. **Look up in local store.** Brit-epr consults rust-ipfs (or the local git object store, for legacy mode) for the CID. If present, return the content immediately.
3. **Check the DHT for provider records.** If not local, query the DHT for peers advertising the CID. A miss here transitions to step 4; a hit gives a peer list.
4. **Fetch via the configured transport.** For Phase 3+, `/brit/fetch/1.0.0` is the protocol; earlier phases use plain rust-ipfs bitswap.
5. **Validate the fetched content.** Parse as the expected ContentNode type. If parse fails, the resolution is reported as *poisoned* — a CID that resolves to something the schema doesn't accept is a stronger error than a CID that doesn't resolve at all.
6. **Cache and return.**

### 5.2 Target type constraints

The elohim-protocol app schema declares the allowed target types for each CID-bearing trailer key:

| Trailer key | Allowed target ContentNode types |
|---|---|
| `Lamad-Node:` | Any lamad-pillar ContentNode from the protocol vocabulary. At minimum: `lamad.path`, `lamad.content`, `lamad.exercise`, `lamad.mastery-claim`. (Names illustrative; protocol-layer source of truth resides in the lamad app schema, not brit's.) |
| `Shefa-Node:` | Shefa economic event or bundle. At minimum: `shefa.economic-event`, `shefa.contribution-bundle`. |
| `Qahal-Node:` | Qahal governance decision or rule. At minimum: `qahal.decision`, `qahal.rule-set`, `qahal.consent-record`. |

A linked node with a `contentType` outside the allowed set is a *target type mismatch*. Brit-verify surfaces this as a hard error (schema violation) rather than a soft warning (unavailability).

### 5.3 Failure modes and their severity

| Failure | Severity | Response |
|---|---|---|
| CID parse error | Parser error | Commit rejected at parse time. Does not reach resolution. |
| CID resolves locally; wrong target type | Schema error | brit-verify fails. Record as "poisoned link." |
| CID does not resolve locally; DHT lookup succeeds; fetch succeeds; wrong target type | Schema error | Same as above — pulled content is also poisoned. |
| CID does not resolve locally; DHT miss | Warning | Commit passes verify with an "offline" flag. Not a schema violation. |
| CID does not resolve locally; DHT hit; fetch times out | Warning | Offline flag. |
| CID resolves; parse of ContentNode fails | Schema error | Poisoned. |
| Linked node's own pillars contradict the inline summary | Warning (possibly error in strict mode) | Flag as "drift." Trailer wins for authority; warn the user. |

The rationale: "unavailable" is a legitimate state in a P2P network. "Lying" is not. The schema is strict about agreement when data is present; permissive about absence.

### 5.4 Caching policy

Brit-epr caches resolved linked nodes in whatever CID-addressed store is configured (Phase 0–1: none; Phase 2+: rust-ipfs blockstore). Cache entries are immutable — a CID always resolves to the same bytes — so cache invalidation is trivial. Cache eviction follows the host's general GC policy; brit does not pin linked nodes by default. Operators who want to guarantee long-term availability pin at the application layer.

### 5.5 The "trailer-only" mode

For environments where network access is forbidden (airgapped CI, bootstrapping a new node from a blob), brit-verify supports a `--trailer-only` mode that refuses to attempt any resolution. In this mode, all linked-node keys are treated as opaque CIDs — only the parser and schema validator run. This is the mode that a stock git host can reach for, because all it needs is the commit message itself.

---

## 6. Signals emitted

Brit emits protocol-level signals when state changes in ways the rest of the network cares about. Signals are small, notifiable events (not ContentNodes themselves — they *point* at ContentNodes). They flow through the protocol's general signal bus (same substrate as other apps' signals). Brit produces; consumers decide what to do.

Every signal names its trigger, payload, and pillar alignment. Pillar alignment is which pillar the signal primarily belongs to — signals often touch all three pillars, but the primary one determines routing priority.

### 6.1 Signal catalog

| Signal name | Trigger | Payload | Primary pillar |
|---|---|---|---|
| `brit.repo.created` | A new RepoContentNode is published for the first time. | `{repo_cid, steward, genesis_commit_cid}` | shefa |
| `brit.repo.stewardship.changed` | A repo's `stewardshipAgent` changes. | `{repo_cid, old_steward, new_steward, decision_cid}` | qahal |
| `brit.repo.archived` | Repo marked no longer maintained. | `{repo_cid, reason_cid}` | qahal |
| `brit.commit.witnessed` | A commit has been parsed, validated, and has valid trailers. | `{commit_cid, git_object_id, repo_cid, pillar_summary}` | lamad |
| `brit.commit.poisoned` | A commit's linked node fails schema validation. | `{commit_cid, key, cid, reason}` | qahal |
| `brit.commit.signed` | A commit carries a valid signature. | `{commit_cid, signer, signature_kind}` | qahal |
| `brit.branch.created` | A new BranchContentNode is published. | `{repo_cid, branch_id, steward, readme_cid?}` | qahal |
| `brit.branch.head.updated` | A branch's head advances via a fast-forward or merge. | `{repo_cid, branch_id, from_commit, to_commit, update_cid}` | shefa |
| `brit.branch.force-pushed` | A branch's head moves non-fast-forward. | `{repo_cid, branch_id, from_commit, to_commit, update_cid, authorizer}` | qahal |
| `brit.branch.stewardship.changed` | A branch's `steward` changes. | `{repo_cid, branch_id, old_steward, new_steward, decision_cid}` | qahal |
| `brit.branch.protection.changed` | A branch's protection rules CID changes. | `{repo_cid, branch_id, old_rules, new_rules}` | qahal |
| `brit.branch.abandoned` | Steward marks branch as abandoned. | `{repo_cid, branch_id}` | shefa |
| `brit.ref.updated` | Any RefContentNode gets a new RefUpdateContentNode. Lower-level than branch.head.updated. | `{ref_id, update_cid, reason}` | qahal |
| `brit.tag.published` | A new TagContentNode is published. | `{repo_cid, tag_cid, target_commit, name}` | lamad |
| `brit.tag.yanked` | A tag is yanked. | `{repo_cid, tag_cid, reason_cid}` | qahal |
| `brit.fork.created` | A ForkContentNode is published. | `{parent_repo, fork_repo, fork_point, new_steward, reason}` | qahal |
| `brit.fork.healed` | A fork is merged back into its parent. | `{parent_repo, fork_repo, healing_commit_cid}` | shefa |
| `brit.merge.consented` | A merge lands on a protected branch with valid qahal authorization. | `{commit_cid, branch_id, decision_cid, dissent}` | qahal |
| `brit.merge.rejected` | A proposed merge fails qahal check. | `{proposed_commit_cid, branch_id, reason}` | qahal |
| `brit.review.attested` | A `Reviewed-By:` trailer is published on a commit. | `{commit_cid, reviewer, capability_cid, decision}` | qahal |

### 6.2 Signal shape conventions

- Every signal has `{name, timestamp, producer_agent}` as metadata.
- Payloads are flat; complex context is delivered by CID reference, not inline.
- Signals are idempotent keyed by `(name, primary_cid, event_timestamp)` — a re-emission by the same producer for the same event is deduped by consumers.
- Signals are not themselves ContentNodes, but every signal's payload references at least one CID so that the recipient can walk to the full context.

### 6.3 Subscription patterns *(forward reference, not decided here)*

Phase 4+ will wire brit signals into the protocol's subscription model (feed types: path, steward, community, layer — from the EPR companion specs). For now, the signal catalog is the vocabulary; the delivery mechanism is the next document.

### 6.4 Pillar alignment commentary

The "primary pillar" column above is load-bearing for routing. When a signal is `qahal`-primary, consumers who subscribe to qahal feeds see it. When a signal is `shefa`-primary, consumers tracking economic flows see it. The fact that `brit.commit.witnessed` is lamad-primary is an editorial statement: witnessing a commit is primarily a learning event (someone built something, someone else can study it), and the economic and governance sub-events are secondary notifications.

These assignments are open to revision as we learn how the feed consumers behave in practice. The rationale for each is worth writing down in a follow-on doc.

---

## 7. Feature-module boundary

This section is the concrete plan for the engine-vs-schema split from §2, expressed as crate layout, feature flags, and public surface area.

### 7.1 Default decision: one crate with feature, revisit at Phase 2

For Phase 0–1, the simplest shape is:

- **`brit-epr`** — a single crate with:
  - Unconditional module `engine` — trailer parser, serializer, generic validator, schema dispatch trait `AppSchema`, CID utilities.
  - `#[cfg(feature = "elohim-protocol")]` module `elohim` — the `AppSchema` implementation for elohim-protocol, the ContentNode type ids, the signal catalog constants.
  - Default features: `["elohim-protocol"]`.

For Phase 2, when the ContentNode adapter grows substantially, the `elohim` module can be promoted to its own crate `brit-epr-elohim` with zero public API changes to callers (re-export via the feature).

### 7.2 Public surface (engine)

Exposed unconditionally:

- `AppSchema` trait — the dispatch contract described in §2.3.
- `TrailerSet` type — an ordered, duplicate-aware map of `(key, value)` pairs with roundtrip-preserving display.
- `TrailerBlock` parser — given a commit body, locate and extract the trailer block.
- `ValidationError` type with categorized variants (`ParseError`, `SchemaError`, `ResolutionWarning`).
- `Cid` newtype — thin wrapper over the `cid` crate, constrained to CIDv1 and brit's allowed codecs.
- `SignatureDescriptor` — opaque signing adapter hook.

### 7.3 Public surface (elohim-protocol feature)

Exposed only when the `elohim-protocol` feature is on:

- `ElohimProtocolSchema` — the `AppSchema` implementor.
- `PillarTrailers` struct — strongly-typed wrapper around the six trailer keys (three inline, three Node).
- `ContentNodeTypeId` enum — `RepoContentNode`, `CommitContentNode`, etc. from §3.
- `Signal` enum — the signal catalog from §6.
- Free functions: `parse_pillar_trailers(body: &str)`, `validate_pillar_trailers(&PillarTrailers)`, `render_pillar_trailers(&PillarTrailers) -> String`.

### 7.4 What a downstream app schema would replace

Someone writing a different app schema — call it `acme-protocol` — would:

1. Disable the `elohim-protocol` default feature in their `Cargo.toml`.
2. Write their own crate `brit-epr-acme` that provides an `AcmeSchema: AppSchema`.
3. Wire their binary to construct an `AcmeSchema` and pass it into brit-epr's engine APIs.

They do **not** fork brit. They do not touch the engine. Their entire app schema is a ~2000-line crate that implements the trait and declares a trailer catalog.

The elohim-protocol app schema is one implementation; brit's covenant is to make sure it is not the *only possible* implementation. This is what keeps brit-epr useful as a generic substrate — and what keeps the gitoxide upstream contribution story plausible for the engine half.

### 7.5 Boundary smells to watch for

During implementation, if any of these happen, the boundary is drifting and we need to fix it before shipping:

- Engine code directly references `Lamad`/`Shefa`/`Qahal` by name. *(Boundary violation — schema-specific.)*
- Engine code hard-codes CID codecs that the elohim schema uses but others might not. *(Make it configurable at `AppSchema` construction.)*
- The app schema has to reach into engine internals to do its job. *(Expose a new engine extension point instead.)*
- A "simple" feature needs `#[cfg(feature = "elohim-protocol")]` to appear inside engine modules. *(Move the feature-gated logic into the schema module.)*

---

## 8. Open questions

This section collects the places where the design is deliberately unfinished and needs human judgment before implementation.

### 8.1 Hard design decisions that await an opinion

1. **One crate or two?** §7.1 punts on whether `brit-epr-elohim` is a separate crate or a feature-gated module. The phased plan is "one crate for Phase 0–1, split at Phase 2 if needed," but some reviewers will want the split immediately for the legibility benefit. Needs a call.
2. **Legacy commits (no trailers).** §3.2 proposes wrapping them with `lamad = {"provenance": "imported-legacy"}` and `qahal = {"authorizedBy": "retroactive-adoption"}`. Is that the right story for the moment brit imports the elohim monorepo itself? Some of those commits predate the protocol existing at all. A cleaner answer would be "the adoption ceremony produces a single retroactive attestation that blanket-covers the pre-brit history," but that requires a new ContentNode type. Flag.
3. **Force-push policy.** §3.5 and §3.7 leave the exact authorization shape of force-pushes underspecified. The current sketch is "qahal field must satisfy protection rules," but what the *protection rules* look like — a DSL, a CID-addressed policy ContentNode, a set of required attestation kinds — is Phase 2 design work. Must be decided before §3.5 hardens.
4. **Pillar summary enums — closed or extensible?** §4.5 declares a fixed set of verbs (`demonstrates`, `teaches`, `corrects`, …) and actor-kinds. Should these be closed (protocol law), open (schema-scoped), or hybrid (closed core + schema-scoped extensions)? Closed is safest for round-trip and interop; open is friendlier to learning-what-we-meant over time. Lean: closed for v1, versioned protocol upgrades to extend.
5. **Agent-scoped vs. repo-scoped branch identity.** §3.5 uses a composite `{repo_cid, branch_name, owning_agent}` as the stable id. This means two agents with a branch named `main` on the same repo have two different BranchContentNodes. Is that correct — it honors the per-steward view model — or does it break too many intuitions from git's single-main-per-repo model? Lean: correct, because the agent-scoped identity is what makes fork→negotiate→merge legible, but it has UX implications we haven't thought through.
6. **Notes as a distinct type.** §3.9 flags this explicitly. Needs a call before Phase 4.
7. **Sub-repos vs. submodules.** §3.3 and §3.10 sketch sub-repos as a TreeContentNode `subRepo` reference. How that interacts with gitoxide's existing submodule support is not worked out. Could be a Phase 5+ concern; flag for now.

### 8.2 Areas where the hybrid (c) design may need revisiting

The exercise of writing this document turned up two places where the locked-in hybrid design feels under stress:

1. **Inline summary grammar is load-bearing.** The "trailer wins when it disagrees with the linked node" rule from §4.10 is clean, but it puts a lot of weight on the inline summary being *expressive enough* to carry real commitments. §4.5's microgrammar is a first attempt; it might be too narrow (forcing people to pick a verb from a short list) or too permissive (the `claim` free text field defeats validation). A round of writing real sample trailers from real commit histories before implementation will calibrate this.

2. **Review attestations are ambiguously placed.** §3.2 has reviews as fields on CommitContentNode and §4.3 has `Reviewed-By:` as a trailer. These must stay consistent — a review that only exists in the trailer and not in the linked node, or vice versa, creates the same drift the hybrid design was built to avoid. The clean answer is: `Reviewed-By:` trailers are authoritative; linked review ContentNodes are enrichment. But commit trailers are per-commit and rarely numerous, whereas code reviews can produce long threaded discussions. The length cap in §4.4 (1024 bytes for `Reviewed-By:`) is tight. May need a second look.

Neither of these requires abandoning the hybrid design, but both suggest a Phase 1.5 "calibration" pass where we stress-test the grammars against real commits before declaring the schema stable.

### 8.3 Things deliberately out of scope

- Transport (`/brit/fetch/1.0.0`, libp2p wiring) — Phase 3.
- DHT announcement and peer discovery — Phase 5.
- Per-branch README rendering and tooling — Phase 4.
- Migration strategy for the elohim monorepo itself — needs its own design doc.
- Interaction with the lamad/shefa/qahal app schemas' own ContentNode vocabularies — those are the protocol layer's problem, not brit's.
- Upstream-contribution shape for the engine half — TBD after Phase 1 stabilizes.

---

## 9. Cross-references

- **Roadmap:** `docs/plans/README.md` — the seven-phase decomposition. This schema document is the substrate for Phases 0–1 directly and Phases 2–6 by implication.
- **Phase 0+1 plan:** `docs/plans/2026-04-11-phase-0-epr-trailer-foundation.md` — will be revised after this schema lands. The trailer keys, the parser's validation levels, and the `AppSchema` trait sketch from this document are the new substrate for that plan.
- **Phase 2 plan (forthcoming):** will consume §3 (ContentNode type catalog) as its contract for the adapter's output types.
- **Phase 3 plan (forthcoming):** will consume §5 (linked-node resolution) and §6 (signals) as its contract for what flows over the wire.
- **Phase 4 plan (forthcoming):** will consume §3.5 (BranchContentNode) and §3.3 (TreeContentNode) for per-branch README resolution.
- **Phase 5 plan (forthcoming):** will consume §6 (signals) to decide which signals are DHT-announced and which are peer-gossip-only.
- **Phase 6 plan (forthcoming):** will consume §3.8 (ForkContentNode) as the fork lifecycle contract.

### Which sections go where

| Section | Primary consumer phase | Secondary consumers |
|---|---|---|
| §2 engine/schema split | Phase 0 | Phase 2 (adapter), Phase 7+ (upstream contribution) |
| §3 ContentNode catalog | Phase 2 (adapter) | Phase 4 (branches), Phase 6 (forks) |
| §4 trailer spec | Phase 0 + Phase 1 | Every phase (trailers are forever) |
| §5 linked-node resolution | Phase 2 + Phase 3 | Phase 5 (DHT) |
| §6 signals | Phase 3 + Phase 5 | Phase 4 (branch signals) |
| §7 feature-module boundary | Phase 0 | Any downstream fork |
| §8 open questions | Every phase | Human reviewers before implementation |

---

## Appendix A — Quick reference: trailer keys

| Key | Required | Value shape | Cap | Owner |
|---|:---:|---|---|---|
| `Lamad:` | yes | verb + free text + optional modifiers | 256B | elohim-protocol |
| `Shefa:` | yes | actor-kind + contribution-kind + modifiers | 256B | elohim-protocol |
| `Qahal:` | yes | auth-kind + modifiers | 256B | elohim-protocol |
| `Lamad-Node:` | no | CIDv1 + optional fragment | 512B | elohim-protocol |
| `Shefa-Node:` | no | CIDv1 + optional fragment | 512B | elohim-protocol |
| `Qahal-Node:` | no | CIDv1 + optional fragment | 512B | elohim-protocol |
| `Reviewed-By:` | no | display + agent + capability | 1024B | elohim-protocol |
| `Signed-Off-By:` | no | display + email (DCO) | 1024B | inherited |
| `Brit-Schema:` | no | schema id | 256B | engine |

## Appendix B — Quick reference: ContentNode types

| Type | Purpose | Phase that implements |
|---|---|---|
| `RepoContentNode` | Top-level repo envelope. | Phase 2 |
| `CommitContentNode` | Covenantal commit. | Phase 2 |
| `TreeContentNode` | Directory snapshot. | Phase 2 |
| `BlobContentNode` | File payload wrapper. | Phase 2 |
| `BranchContentNode` | Stewarded view over history. | Phase 4 |
| `TagContentNode` | Covenantal release attestation. | Phase 2 |
| `RefContentNode` + `RefUpdateContentNode` | Authoritative pointer log. | Phase 2 / Phase 5 (DHT integration) |
| `ForkContentNode` | Alternate lineage with stewardship transfer. | Phase 6 |
| `NoteContentNode` *(provisional)* | Retroactive attestation. | Phase 4 or deferred to protocol layer |

## Appendix C — Quick reference: signals

See §6.1 for the full catalog. Grouped by phase that first emits them:

- **Phase 1 (from trailers only):** `brit.commit.witnessed`, `brit.commit.poisoned`, `brit.commit.signed`, `brit.review.attested`.
- **Phase 2 (adapter):** `brit.repo.created`, `brit.tag.published`.
- **Phase 4 (branches):** `brit.branch.created`, `brit.branch.head.updated`, `brit.branch.force-pushed`, `brit.branch.stewardship.changed`, `brit.branch.protection.changed`, `brit.branch.abandoned`, `brit.ref.updated`, `brit.merge.consented`, `brit.merge.rejected`.
- **Phase 6 (forks):** `brit.fork.created`, `brit.fork.healed`, `brit.repo.stewardship.changed`, `brit.repo.archived`, `brit.tag.yanked`.

---

*End of Elohim Protocol App Schema Manifest v0.1.*
