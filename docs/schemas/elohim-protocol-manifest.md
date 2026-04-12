# Brit — Elohim Protocol App Schema Manifest

**Status:** Draft v0.2 — exploration
**Targets protocol version:** `elohim-protocol/1.0.0` (pre-release)
**Owner:** brit maintainers
**Last updated:** 2026-04-11

---

## 1. Header & framing

### 1.1 What this document is

This is the **app-level schema manifest** for brit's integration with the Elohim Protocol. It is not a specification of the Elohim Protocol itself — that lives separately, in the protocol's reference implementation. This document is brit's answer to the question: *"If every artifact in the protocol is a ContentNode with three-pillar coupling, what are the ContentNodes of a distributed version control system, what command surface drives them, and how do they survive a `git clone` from GitHub?"*

Brit (בְּרִית, "covenant") is an expansion of [gitoxide](https://github.com/GitoxideLabs/gitoxide) that adds covenantal semantics on top of git. A commit in brit is not just a hash-linked snapshot — it is a witnessed agreement whose terms (lamad, shefa, qahal) travel with the commit itself.

This document defines:

1. The **engine vs. app schema** boundary that keeps brit-epr usable as a generic substrate.
2. The **brit CLI command surface**, designed git-analogous and LLM-first.
3. The **skill and template artifacts** that let an LLM agent drive `brit commit` without reasoning from scratch about pillar positioning.
4. The **ContentNode catalog** for repos, commits, trees, blobs, branches, tags, refs, forks, doorway registrations, and per-branch READMEs — plus reserved extension slots for build manifests, attestations, and merge consent.
5. The **commit-trailer specification** — the canonical RFC-822 surface that survives `git clone`.
6. The **linked-node resolution protocol** through the doorway bridge.
7. The **backward compatibility contract** with stock git hosting.
8. The **protocol signals** brit emits.
9. The **doorway registration format** that points a brit repo at its primary steward's gateway.
10. The **alignment with the p2p-native build system roadmap**.
11. **Two persona scenarios** that exercise the schema end-to-end.
12. An honest **open-questions** section.

### 1.2 The four framings (read these before anything else)

These framings are non-negotiable. They drive every design decision in the document.

**Framing 1 — The brit CLI is an LLM development tool first.** The primary user of `brit commit`, `brit branch`, `brit fork` is an LLM agent. Humans use a UI on top (the elohim-app frontend served through doorway) for review and consent. This means: command names mirror git (use the LLM's existing training as the cognitive carrier; do not invent clever new verbs); the cognitive complexity budget per command is "what an LLM with a skill file and a per-repo template can drive reliably"; and the hard parts of authoring pillar metadata are pushed into the skill+template, not the prompt the LLM has to reason about. Humans still need to be able to use the CLI in offline/bootstrap scenarios — but the happy path is LLM-driven and the human happy path is the UI.

**Framing 2 — Brit repos are fully backward-compatible with stock git hosting.** Any brit-managed repo is also a valid git repo. `git clone https://github.com/...` from any machine with stock git must work, on any forge — GitHub, GitLab, Codeberg, Gitea, sourcehut. The clone gets the commits (with their RFC-822 trailers intact) and a small set of repo-local config files in `.brit/`. That's all that travels through web2. Inside the elohim network, a doorway registration file in the repo points at a steward's doorway, which resolves the EPR view: linked ContentNodes, per-branch READMEs as EPRs, attestation graphs, build manifests, shefa events. Outside the elohim network, a brit repo degrades gracefully to "git with extra trailer discipline."

**Framing 3 — Brit sits in the middle of the p2p-native build system arc.** Per the p2p-native build system roadmap, Stage 1 (Root) introduces `BuildManifest` and `BuildAttestation` as ContentNode schemas, Stage 2 (Canopy) gossips them through the DHT for peer-attested builds, and Stage 3 (Forest) dissolves Jenkins as the build system builds itself. Brit is the VCS layer that makes this possible: a BuildManifest is a file in a brit repo whose CID is the content address of the tree/blob containing it; a BuildAttestation is a commit (or ref-note) signed by a steward node's agent key. The build graph IS the git graph, read through the EPR lens. This document does not define BuildManifest or BuildAttestation in detail — that's the build-system roadmap's job — but it explicitly **reserves extension points** so they can plug in without a breaking schema change.

**Framing 4 — The feature-module boundary: brit-epr is the engine, elohim-protocol is one app.** Brit must remain usable as a plain expansion of gitoxide that happens to parse RFC-822 trailers. The elohim-protocol vocabulary — its three pillars, its trailer key names, its ContentNode catalog — lives behind a cargo feature flag (provisional name `elohim-protocol`, default-on). Someone could fork brit, disable the feature, and write a different app schema for a different domain (carbon accounting, music composition, biological sequence annotation) without touching the engine. The engine knows nothing about lamad, shefa, or qahal. It dispatches to whatever schema is loaded.

### 1.3 How to read this document

- Implementing Phase 0+1 (parser + CLI scaffolding): read §1, §2, §3, §6, §11.
- Implementing Phase 2 (ContentNode adapter): read §5, §7, §11.
- Implementing Phase 3+ (transport, doorway integration): read §7, §10.
- Writing a different app schema on top of brit-epr: read §2, §11.
- Trying to understand whether the schema supports your story: read §13 (scenarios) first, then jump where needed.
- Looking for the things this document deliberately did not decide: read §14.

### 1.4 Target protocol version

This manifest targets `elohim-protocol/1.0.0`, the version being stabilized as it is written. The schema does not hard-code the protocol revision in trailer values; it references protocol types by name and expects resolution against whichever protocol version the resolving node has loaded. A future revision will introduce a `Brit-Schema:` trailer (already reserved in §6) that explicitly names the schema version when needed for interop.

### 1.5 Terminology

| Term | Meaning |
|---|---|
| **ContentNode** | The protocol's universal content envelope. Has `id`, `contentType`, `title`, `description`, `content`, `contentFormat`, `tags`, `relatedNodeIds`, and pillar fields. Every notarized artifact in the protocol is (or decomposes to) ContentNodes. |
| **EPR** | Elohim Protocol Reference. Canonical content address: `epr:{id}[@version][/tier][?via=][#fragment]`. |
| **Tier** | One of Head (~500B, DHT-gossipped), Document (~5–50KB, peer-cached), Bytes (arbitrary, shard-delivered). |
| **Pillar** | One of lamad (knowledge), shefa (value), qahal (governance). Every ContentNode carries all three; blanks are explicit, not implicit. |
| **Trailer** | RFC-822-style `Key: value` line at the end of a git commit message. `git interpret-trailers` compatible. |
| **Linked node** | A ContentNode whose CID is referenced from a commit trailer. Optional; the trailer's inline summary is always authoritative. |
| **CID** | Content identifier per multiformats CIDv1. Brit prefers BLAKE3; accepts SHA-256 on input. |
| **Doorway** | The Rust gateway service that bridges browsers/CLIs to the elohim peer network. Each brit repo's `.brit/doorway.toml` points at a primary doorway URL. |
| **brit-epr** | The engine crate(s) implementing trailer parsing, validation, and schema dispatch. |
| **elohim-protocol app schema** | The vocabulary in this document. One implementation of the engine's schema trait. |

---

## 2. Separation of concerns — brit-epr engine vs. elohim-protocol app schema

### 2.1 The reframing

Earlier sketches of brit-epr conflated the trailer parser with the protocol vocabulary. That conflation is wrong. Trailer parsing is generic — any app that wants to carry structured metadata in commit messages faces the same problems. The pillar vocabulary (which keys exist, what values are legal, what linked-node types are valid) is specific to one app's worldview.

**Reframe:**

- **brit-epr** is an *engine*. It parses, validates, and round-trips RFC-822 trailers in git commits, dispatches semantic checks to a loaded app schema, and provides CID utilities. It knows nothing about lamad, shefa, or qahal.
- **elohim-protocol** is an *app schema*. It implements the engine's schema trait. It declares pillar key names, value formats, linked-node target type constraints, the ContentNode catalog, and the signal taxonomy described in this document.
- A third party could fork brit, disable the `elohim-protocol` feature, and ship `brit-epr-acme` with their own trailer keys. The engine would not care.

The separation matters because the engine half (trailer parse/serialize, commit round-trip, validator scaffolding, CID parsing) is potentially **upstreamable to gitoxide** in the long run. The schema half is brit's opinionated covenant and stays in brit.

### 2.2 Engine responsibilities

The engine owns:

1. **Trailer parsing.** Walking a commit's body, finding the trailer block, splitting into `(key, value)` pairs. In gitoxide terms, this wraps and extends `gix_object::commit::message::BodyRef::trailers()`.
2. **Trailer serialization.** Given an ordered set of `(key, value)` pairs, writing the trailer block back into a commit message in a form that round-trips through stock git, `git interpret-trailers`, `git rebase`, `git cherry-pick`, `git am`, and `git format-patch`.
3. **Generic validation.** Key shape (ASCII token), value constraints (no embedded LF unless folded with continuation indent, length caps, CRLF normalization), duplicate-key policy.
4. **Schema dispatch.** Looking up which app schema owns which keys, delegating semantic validation to the schema.
5. **CID parsing/formatting.** Multiformats CIDv1 parse, display, kind/codec checking. Engine-level because multiple app schemas will carry CIDs and the spelling is stable.
6. **Signing adapter hooks.** A surface for signed commits (GPG, SSH, minisign, agent attestation) so app schemas can attach signatures to linked nodes without the engine knowing the signing kind.
7. **Commit round-trip plumbing.** Reading a `gix-object::Commit`, extracting the trailer block, modifying it, and writing the new commit object. The engine guarantees byte-stable round-trip when no semantic changes occur.

The engine does **not** own:

- The set of known keys (schema's problem).
- The set of ContentNode types (schema's problem).
- The signal taxonomy (schema's problem).
- Network transport (lives in the future `brit-transport` crate).
- The storage backend (lives in `brit-store` or via rust-ipfs integration).
- The doorway registration format itself — although §10 puts this format in the elohim-protocol schema, the engine just sees it as a config file the schema parses.

### 2.3 The engine-to-schema trait (pseudocode)

Pseudocode only. No syntactically valid Rust below — the next session writes the real types.

```text
trait AppSchema {
    // Stable identifier, e.g. "elohim-protocol/1.0.0".
    fn id() -> SchemaId;

    // Does this schema recognize this trailer key?
    fn owns_key(key: &str) -> bool;

    // Required keys. Engine uses this to short-circuit validation when the
    // commit message is missing the required surface entirely.
    fn required_keys() -> &'static [&'static str];

    // Validate one (key, value) pair in isolation (no cross-field rules).
    fn validate_pair(key: &str, value: &str) -> Result<(), ValidationError>;

    // Validate the whole trailer set together (cross-field rules, e.g.
    // "Lamad-Node: present requires Lamad: non-empty").
    fn validate_set(trailers: &TrailerSet) -> Result<(), ValidationError>;

    // Which keys carry CID references? The resolver walks these.
    fn cid_bearing_keys() -> &'static [&'static str];

    // For each CID-bearing key, what ContentNode type(s) is a valid target?
    fn allowed_target_types(key: &str) -> &'static [ContentNodeTypeId];

    // Render a TrailerSet as RFC-822 lines in canonical order. Used by the
    // commit writer.
    fn render(trailers: &TrailerSet) -> String;

    // OPTIONAL — schema may emit signals when commits are witnessed.
    fn signals_for(commit: &CommitView) -> Vec<Signal> { vec![] }
}
```

The engine's public API takes an `&dyn AppSchema` (or monomorphizes via generic). The `elohim-protocol` feature-gated module provides the implementation this crate ships with.

### 2.4 Why a feature flag, not just a separate crate

We expect brit to always ship with the elohim-protocol schema enabled in its default build. The feature flag is not there to make compilation smaller — it is there to make the **boundary legible**. Every symbol behind `#[cfg(feature = "elohim-protocol")]` is a symbol that is brit-as-a-protocol-app, not brit-as-a-covenant-engine. Someone reading the code should be able to tell at a glance: "if I remove this feature, do I still have a working git?" The answer must always be yes.

A downstream fork that wants its own schema should be able to express "I want brit-epr but not elohim-protocol" in their `Cargo.toml` in one line, not by surgery on brit's source.

### 2.5 Building a different app schema

A team building `acme-protocol` (e.g., a carbon-accounting protocol) would:

1. Disable the `elohim-protocol` default feature in their `Cargo.toml`.
2. Write a new crate `brit-epr-acme` that provides an `AcmeSchema: AppSchema` implementation, declaring trailer keys like `Carbon-Footprint:`, `Offset-Source:`, `Verification-Body:`.
3. Wire their CLI binary to construct an `AcmeSchema` and pass it into brit-epr's engine APIs.
4. Optionally, ship their own JSON Schema files in `schemas/acme-protocol/v1/*.schema.json` mirroring the elohim-protocol layout in brit.

They do not fork brit. They do not touch the engine. Their entire app schema is a focused crate that implements one trait and declares its catalog.

---

## 3. The brit CLI command surface (LLM-first, git-analogous)

Per Framing 1, every brit command shape-shifts a git command. The LLM already knows git; brit uses that training as the cognitive carrier. A new verb is introduced only when no git verb maps. Commands are listed below with their git analogue, the additional pillar-awareness brit adds, how an LLM drives it (skill + template), and what the human reviews afterward.

### 3.1 Command catalog

| brit command | Git analogue | New behavior | New verb? |
|---|---|---|---|
| `brit init` | `git init` | Plus interactive doorway registration prompt; writes `.brit/doorway.toml`; emits `brit.repo.created` signal. | No |
| `brit clone <url>` | `git clone` | After clone, reads `.brit/doorway.toml` and (if a doorway is reachable) hydrates linked ContentNodes for the default branch. | No |
| `brit add` | `git add` | Unchanged. Working-tree manipulation. | No |
| `brit status` | `git status` | Plus a one-line summary of unresolved pillar drift in the staged commit, if any. | No |
| `brit commit [-m]` | `git commit` | Loads `.brit/commit-template.yaml` + skill file, prompts for missing pillar trailer values (or accepts them via flags `--lamad`, `--shefa`, `--qahal`, `--lamad-node`, …), validates the resulting trailer block, writes the commit. | No |
| `brit log` | `git log` | Default format includes pillar summary lines; `--graph` overlays branch stewardship coloring. | No |
| `brit branch [name]` | `git branch` | Creates the git ref AND a `BranchContentNode` with a per-branch README slot, lamad audience field, default qahal protection rules inherited from the repo. | No |
| `brit checkout` / `brit switch` | identical | Unchanged. Read-only ref movement. | No |
| `brit push [remote] [ref]` | `git push` | After git push, emits a `brit.commit.witnessed` signal stream and posts the new commits' linked-node CIDs to the doorway for steward acceptance. | No |
| `brit pull` / `brit fetch` | identical | After git fetch, hydrates linked ContentNodes for the fetched commits via the doorway. | No |
| `brit merge` | `git merge` | Opens a `MergeProposalContentNode` (§5.13) against the target ref, freezing the consent requirements resolved from the target's protection rules at the moment of proposal. Default is **async**: publishes the proposal, emits `brit.merge.proposed`, prints the proposal manifest as JSON to stdout, exits 0. The proposal lives with a TTL (default inherited from the protection rule, fallback 48h). LLM re-engages via `brit status`, `brit merge --wait --proposal <id>`, or by subscribing to the proposal's doorway event stream. Fast paths skip proposal creation: `self-governance` qahal, pre-satisfied requirements, no-op merges. `--wait[=duration]` polls with cap (default 5min); `--withdraw <id>` cancels an open proposal. **Brit does not own governance** — it reads consent requirements from the parent EPR's governance primitives (see §14.1 #4 resolution). | **Yes** |
| `brit fork` | (none directly) | Creates a `ForkContentNode`, registers a new repo CID with its own stewardship, links to the parent. The user can `git remote add` the parent themselves; `brit fork --as <new-url>` automates that and pushes. | **Yes** |
| `brit attest <commit>` / `brit attest --proposal <id> --consent` | (closest: `git notes add`) | Unified attestation + consent surface. For commits: creates a `Reviewed-By:` trailer (amending if local and unpublished) OR an out-of-band `ReviewAttestationContentNode`. For proposals: records consent from the invoking agent against an open `MergeProposalContentNode` (§5.13); `--as-delegate-of <agent>` supports delegated consent (e.g., an elohim agent voting on behalf of a human who delegated its interests). This is the single verb for every "I stand behind this" act the LLM or human can make. | **Yes** |
| `brit verify [revrange]` | (no direct analogue; closest: `git fsck`) | Runs the parser + schema validator across a commit range; resolves linked nodes via the doorway if reachable; reports drift between trailer summaries and linked nodes. | **Yes** |
| `brit register-doorway <url>` | (none) | Writes/updates `.brit/doorway.toml` with the steward's doorway pointer. Optionally signs the file with the steward's agent key. | **Yes** |
| `brit set-steward <agent>` | (none) | Updates the repo's `stewardshipAgent` field, emits `brit.repo.stewardship.changed`. Requires existing steward's signature OR co-steward quorum (see §14). | **Yes** |
| `brit show <commit>` | `git show` | Pretty-prints the commit including its pillar trailers, expanded with linked-node summaries (one line each) when reachable. | No |
| `brit blame` | `git blame` | Plus per-line shefa attribution overlay (which contributor's shefa events are tied to the introducing commit). | No |
| `brit diff` | `git diff` | Unchanged. Diffs are diffs. | No |

### 3.2 Commands deliberately NOT extended

These git commands are intentionally pass-through with no brit-side semantics:

- `brit reset`, `brit revert`, `brit cherry-pick` — they produce commits, and the existing `brit commit` interception path handles those new commits' trailer requirements at write time. No need for command-level wrappers.
- `brit stash` — stash entries are local-only; they become regular commits when applied, so they get pillar awareness then.
- `brit rebase` — interactive history rewriting. Brit verifies the rewritten commits satisfy trailers, but does not augment the rebase machinery itself. (Open question §14: should rebase emit `brit.commit.superseded` signals?)
- `brit gc`, `brit prune` — storage maintenance, no semantic content.
- `brit config` — pass-through to gitoxide config; brit-specific config lives in `.brit/`, not in `.git/config`, so the boundary is clean.

### 3.3 How an LLM drives `brit commit` (the load-bearing case)

This is the command the LLM runs most often, so it gets the most ergonomic attention.

**Without skill + template:** The LLM would have to invent pillar values from scratch every commit, and they would drift wildly across commits in the same repo. That's the failure mode we are designing against.

**With skill + template:** The LLM loads `.claude/skills/brit/SKILL.md` (or whatever skill format the harness supports) when it begins working in a brit repo. The skill file teaches the LLM:

- The pillar grammar (verbs, actor-kinds, auth-kinds).
- How to read `.brit/commit-template.yaml` to find the repo's *active learning paths*, *contributor agent ids*, and *current branch protection rules*.
- The two-step authoring pattern: (1) write the commit body as you would for any project, (2) populate trailers by selecting from the template's enums plus a brief free-text claim derived from the body.
- When to set linked-node trailers (`Lamad-Node:` etc.) — never mandatory, but encouraged if the change demonstrates a learning path that already has a CID, or attests to a steward decision that lives as a ContentNode.

**The actual call shape:**

```text
brit commit \
  -m "Refactor merge conflict display to per-hunk witness cards" \
  --lamad "demonstrates per-hunk witness card rendering | path=brit/merge-ui" \
  --shefa "agent code | effort=medium | stewards=agent:matthew" \
  --qahal "steward | ref=refs/heads/dev | mechanism=solo-accept"
```

If any required trailer is missing, brit refuses to commit and prints the missing keys, the template's enum candidates, and a one-line hint. The LLM reads the error, fills the missing trailers, retries. This is the LLM's "feedback loop" — fast, mechanical, no reasoning required.

**The human afterward:** Sees the commit in the elohim-app UI as a card with three colored badges (one per pillar). Click expands the linked nodes. If the pillar values look wrong, the human can `brit attest <commit>` with a corrective note OR (for unpublished commits) ask the LLM to amend.

### 3.4 The CLI is also human-usable

In offline / bootstrap scenarios — first developer in a new region, no doorway reachable, plain laptop with stock Rust — the CLI must work without LLM assistance. The skill file is for ergonomics, not for compliance. A human running `brit commit -m "..."` with all three `--lamad / --shefa / --qahal` flags directly gets the same commit. The error messages are human-readable. The template file is human-editable.

What humans don't get from the CLI: graphical pillar review, drift visualizations, governance dashboards. Those live in the elohim-app UI. The CLI is the producer; the UI is the reviewer.

---

## 4. Skill + template artifacts the LLM uses

This section names the two artifacts that carry the hard parts of pillar authoring out of the LLM's prompt and into the repo's tooling.

### 4.1 The skill file: `.claude/skills/brit/SKILL.md` (or harness-equivalent)

Format: a YAML frontmatter block followed by markdown body, mirroring the convention used by the elohim project's existing skills (`.claude/skills/epr-content-addressing/SKILL.md`, `.claude/skills/seed-workflow/SKILL.md`, etc.).

Frontmatter (illustrative):

```yaml
---
name: brit
description: Reference for committing in a brit repo. Use when running `brit commit`, `brit branch`, `brit fork`, `brit attest`, or any command that produces a witnessed artifact. Covers pillar grammar, template usage, and the LLM-driven authoring loop.
triggers:
  - "brit commit"
  - "pillar trailer"
  - "lamad shefa qahal"
---
```

Body sections (proposed):

1. **Pillar grammar quick-reference.** The verbs/actor-kinds/auth-kinds enumerated in §6.5, with a one-line explanation of each.
2. **Authoring loop.** Step-by-step: read template → write commit body → choose verbs from template → fill claim → invoke `brit commit` with flags → on error, read missing-key list and retry. Worked example with a real (fake) commit.
3. **When to add linked-node trailers.** Decision tree: is the change part of a known learning path? → set `Lamad-Node:`. Is the change a stewardship-significant economic event? → set `Shefa-Node:`. Is the change a governance decision (merge of a contested PR, license change, force-push)? → set `Qahal-Node:`.
4. **How to read template-driven enums.** The template carries enum overrides per repo. Read those before defaulting to the protocol-wide enum.
5. **What to do when the doorway is unreachable.** Commit with trailer-summary only. Skip linked-node lookups. Note in the commit body if the offline mode is intentional.
6. **Failure modes and rescue.** Five worked examples of LLM-emitted bad commits and how to fix them — drawn from §6.9 below.

### 4.2 The per-repo template file: `.brit/commit-template.yaml`

Lives in the repo, version-controlled, edited by the steward. The template carries repo-specific defaults and enum extensions; brit-epr loads it on each commit.

Illustrative shape:

```yaml
schema: elohim-protocol/1.0.0
repo:
  steward: agent:matthew
  doorway: https://doorway.elohim.host/repos/brit
  active_paths:
    - id: brit/substrate-integration
      title: "Wiring rust-ipfs as the storage substrate"
    - id: brit/merge-ui
      title: "Per-hunk witness card rendering"
    - id: brit/llm-authoring
      title: "LLM-first commit ergonomics"
  contributors:
    - agent: agent:matthew
      display: "Matthew"
      default_kind: human
    - agent: agent:claude-opus-4-6
      display: "Claude (Opus 4.6)"
      default_kind: agent

defaults:
  lamad:
    # Used when --lamad is omitted entirely on a non-infrastructure commit.
    verb_hint: "documents"
  shefa:
    actor_kind: agent
    contribution_kind: code
    effort: small
  qahal:
    auth_kind: steward
    ref_default: refs/heads/dev

defaults:
  # Per-repo defaults and helpers. The elohim-protocol vocabulary is CLOSED
  # (see §14.1 #6 resolution) — this block does NOT extend enums. It records
  # repo-local DEFAULTS (e.g., prefer `refactors-no-lamad` for test-only
  # commits) and HELPER HINTS the LLM can use when filling in trailers.
  # A different app schema plugged into brit-epr supplies its own vocabulary
  # via its own manifest; brit never mixes vocabularies at the repo level.
  prefer_lamad_verb_when:
    test_only_change: refactors-no-lamad
    docs_only_change: documents
  shefa_contribution_kind_hints:
    - benchmark    # hint: commit-template suggests this kind for bench/ dir changes
    - reproducibility-evidence  # hint: suggests this for ci/reproduce/ changes

protection_rules:
  refs/heads/main:
    qahal_node: bafkreirepoprotmainabcd1234abcd1234abcd1234abcd1234abcd
    requires:
      - kind: review
        count: 1
      - kind: steward-accept
  refs/heads/dev:
    qahal_node: bafkreirepoprotdevabcd1234abcd1234abcd1234abcd1234abcd12
    requires:
      - kind: steward-accept
```

Behaviors:

- **Active paths** are the lamad path slugs the LLM may choose from when populating `path=` in a `Lamad:` trailer. The skill teaches the LLM to pick one of these (or to leave `path=` off if none fit).
- **Contributors** map agent ids to their default actor-kind. Used when the LLM sets `--shefa` and the actor-kind isn't explicit.
- **Defaults** fill in trailer values when the LLM omits them. Defaults are never silently substituted for required-by-protocol values that the validator would reject; they only fill optional modifiers.
- **Protection rules** carry the qahal CIDs that `brit merge` consults when validating merges to protected refs.

### 4.3 Dynamic template enrichment via the doorway

The static template file in the repo carries the snapshot. When a doorway is reachable, brit-epr can enrich the template at commit time by querying the doorway:

- "What lamad path EPRs are currently linked to this repo's RepoContentNode?" → updates `active_paths`.
- "Who are the recognized contributor agents for this repo?" → updates `contributors`.
- "What is the current protection-rules CID for `refs/heads/main`?" → updates `protection_rules`.

The doorway returns these as a small JSON envelope. Brit-epr merges it into the in-memory template before passing the template to the LLM via the skill's I/O. This is how the schema stays *living* without requiring template commits every time stewardship changes.

When the doorway is unreachable, the static file is the truth. The LLM's authoring loop is unchanged.

### 4.4 Open ergonomic question

Should the skill file be **bundled inside the brit repo** (committed to `.brit/skill.md`) or **provided externally** by the LLM harness (e.g., installed in `~/.claude/skills/brit/`)? Lean toward bundled-in-repo for discoverability and per-repo customization. Discussed further in §14.

---

## 5. ContentNode type catalog

Each subsection enumerates a ContentNode type brit introduces. Each type is addressed by a deterministic CID over a canonical serialization (DAG-CBOR, same as the rest of the protocol). Each declares its three-pillar couplings, relationships to other types, and the open questions this exploration hasn't resolved.

A note on required vs. optional fields: every ContentNode must answer all three pillars, but an answer of *"this artifact does not carry that pillar because X"* is a valid answer. The validator enforces that the pillar field is *present*, not that it is *non-empty*. An explicit `{"rationale": "infrastructure commit, no lamad dimension"}` is legal; an absent field is not.

### 5.1 RepoContentNode

**Purpose.** The top-level envelope for a brit repository. Every repo on the network is addressable by a stable id independent of any clone. This is the thing you point at when you say "give me *this* repo," regardless of which peer hosts it today.

**Content-address strategy.** CID over the canonical serialization of `{repo_id, genesis_commit_cid, created_at, name, stewardship_agent}`. The repo's id is derived from its genesis commit and its original steward — forks get a new repo_id, not a branch of the same one. Renaming the repo does not change the id.

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
| `doorwayRegistration` | CID of `DoorwayRegistration` | Pointer to the in-tree config artifact. |
| `lamad` | object | See below. |
| `shefa` | object | See below. |
| `qahal` | object | See below. |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `parentRepo` | CID of `RepoContentNode` | Present if this repo is a fork. |
| `forkReason` | string | Human-readable explanation. |
| `relatedRepos` | array of CID | Sibling repos (bindings, docs, examples). |
| `license` | SPDX id or CID of a license ContentNode | — |
| `web2Shadows` | array of URL | Hint-only mirrors at GitHub/GitLab/etc. for onboarding flow. Not authoritative. |

**Lamad coupling.** What does the repo teach? Brit itself teaches "distributed version control for covenantal software." A learning-platform repo teaches its subject. The lamad field anchors a *primary learning path* (`lamad.primaryPath` is a CID of a path ContentNode in the lamad vocabulary), plus an `unlocks` array of capability tags the reader gains.

**Shefa coupling.** A repo is a stewardship surface. Contributors earn standing by having their commits accepted into the steward's chosen refs. The shefa field declares: current steward, repo resource kind (typically `code`, `text`, or `schema`), economic events at the repo level (adoption, fork, archival), and whether contributions are tracked for later value distribution.

**Qahal coupling.** A repo declares its governance: who can merge to protected refs, what attestations are required, whether constitutional council review is needed for certain changes (license changes, steward rotation). The qahal field's main job is **naming where governance happens** — actual rules live in their own ContentNode, resolved via CID. This keeps the RepoContentNode small enough for the EPR Head tier.

**Relationships.** Out → CommitContentNode (many, via `currentHead` and `genesisCommit`); → RepoContentNode (parentRepo, relatedRepos); → DoorwayRegistration; → linked lamad/shefa/qahal nodes. In ← ForkContentNode (children); ← BranchContentNode (each branch is stewarded inside a repo).

**Open questions.** Does renaming a repo produce a new version or a new repo? (Lean: new version.) Is `currentHead` redundant with the refs projection? (Lean: keep it for fast snapshotting; refs are authoritative.)

---

### 5.2 CommitContentNode

**Purpose.** The covenantal commit. Wraps a git commit object with the pillar couplings that make the commit a *witnessed agreement* rather than just a snapshot. The CommitContentNode is *not* stored instead of the git commit — it is stored *alongside*, and the git commit's trailers are the canonical summary.

**Content-address strategy.** Two CIDs exist for every commit:

1. The git object id (SHA-1 or SHA-256 per repo's configured hash), computed by gitoxide exactly as upstream git does.
2. The CID of the CommitContentNode itself, computed over its canonical serialization. The CommitContentNode carries the git object id as one of its fields, so the two are linked but not equal.

This duality is load-bearing for the hybrid design: stock git tools see the git object id; brit tools see either.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | CID of this CommitContentNode | — |
| `contentType` | `"brit.commit"` | Literal. |
| `gitObjectId` | hex git object id | What `git log` prints. |
| `repo` | CID of `RepoContentNode` | Which repo this commit belongs to. |
| `parents` | array of CID of `CommitContentNode` | Zero/one/many. Ordered. |
| `treeRoot` | CID of `TreeContentNode` | Repo snapshot at this commit. |
| `author` | `{agent_id, display, timestamp}` | Mirrors git author. |
| `committer` | `{agent_id, display, timestamp}` | Mirrors git committer. |
| `messageSubject` | string | First line of the commit message. |
| `messageBody` | string | Remaining lines, excluding the trailer block. |
| `trailerSummary` | inline trailer key/value pairs | Exact string parsed out of the commit message. |
| `lamad` | object | — |
| `shefa` | object | — |
| `qahal` | object | — |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `signatures` | array of signature descriptors | GPG, SSH, minisign, agent attestation. |
| `lamadNode` | CID | Rich lamad context (see §7). |
| `shefaNode` | CID | Rich shefa events. |
| `qahalNode` | CID | Rich governance context. |
| `reviewedBy` | array of `{agent, capability_cid, decision, note_cid?}` | Per-review attestations. |
| `supersededBy` | CID of `CommitContentNode` | When a commit is rebase-dropped, amended, or force-pushed over. |
| `buildAttestations` | array of CID of `BuildAttestationContentNode` | **Reserved extension** for the build system roadmap. |

**Lamad coupling.** What does a reader learn from studying this diff? For a feature commit: `{"demonstrates": "wiring a libp2p behaviour into the swarm", "unlocks": ["libp2p-behaviour-composition"], "path": "brit/substrate-integration"}`. For a fix commit: `{"corrects": "previously documented-but-wrong SLA path"}`. For infra: `{"rationale": "ci-only change, no lamad"}` is valid.

**Shefa coupling.** Records REA events triggered by the commit landing. At minimum: `{"author": <agent>, "contributionKind": "code|docs|test|schema|review|infra", "effort": <bucket>, "stewardAccepting": <agent>}`. For commits merging third-party contributions, includes provenance — who submitted, through what flow, whether economic reward flows back.

For bots and machine commits (CI baseline updates), `{"contributorKind": "machine", "parentAgent": <human or system that owns the bot>}`. Machines do not earn standing for themselves; standing passes to the owning agent.

**Qahal coupling.** Records what collective authorized this commit. For solo: `{"authorizedBy": "self", "ref": "refs/heads/personal/matthew/scratch"}`. For protected ref: `{"authorizedBy": <gov node cid>, "mechanism": "consent|vote|attestation", "quorum": "...", "dissent": [...]}`. Dissent records survive consent decisions — the "we merged but Bob disagreed" trail.

**Relationships.** Out → parents, treeRoot, repo, lamad/shefa/qahal nodes, reviews, supersededBy, build attestations. In ← child commits, BranchContentNode (head), TagContentNode (target), RefContentNode.

**Open questions.** How to wrap legacy commits without trailers (imported from elohim monorepo's pre-brit history)? Lean: `lamad = {"provenance": "imported-legacy"}`, `qahal = {"authorizedBy": "retroactive-adoption", "adoptingSteward": <agent>}`. Trailer requirement enforced for **new** brit commits, not imported history. Open question §14: should there be a single retroactive-adoption ContentNode that blanket-covers a range, instead of individually tagging?

How to handle rebases that rewrite history? Lean: each rewritten commit gets `supersededBy` pointing at its new form; old CommitContentNodes are still resolvable if cached but flagged as historical.

---

### 5.3 TreeContentNode

**Purpose.** The repo snapshot at a particular commit. Mirrors a git tree object. Most of the time the pillars of a tree are passthrough from the parent commit; they exist so individual subtrees (e.g., `docs/`) can carry their own lamad/shefa/qahal context for sub-repository stewardship.

**Content-address strategy.** CID over the canonical serialization of the tree's entries `{name, mode, target_cid, target_type}`. When using git's native SHA hash, the git tree's object id and the TreeContentNode CID are separate addresses over the same logical content.

**Required fields.** `id`, `contentType: "brit.tree"`, `gitObjectId`, `entries`, `lamad`, `shefa`, `qahal` (each pillar may be `{inherit: "parent-commit"}`).

**Optional fields.** `subRepo` (CID of a sub-RepoContentNode for first-class sub-repo boundaries, similar to submodules), `codeowners` (per-tree curation delegates).

**Pillar coupling.** Usually inherit. Non-inherit values matter for sub-repo trees and for `docs/` subtrees treated as their own learning path (`lamad = {"pathAnchor": <cid>}`). A `translations/` tree may have its own shefa for translator standing. A `docs/legal/` tree may have its own qahal requiring constitutional council consent.

**Relationships.** Out → child trees, blobs, optionally sub-repo. In ← parent commit (treeRoot), parent tree (sub-entry).

---

### 5.4 BlobContentNode

**Purpose.** A file in a repo, wrapped as a ContentNode. Mirrors a git blob. Most blobs carry minimal pillar metadata — they inherit from parent tree/commit unless something in the file justifies separate fields.

**Content-address strategy.** CID over the raw bytes. Git blob id and BlobContentNode CID are separate addresses over the same bytes when git's native hashing is in use.

**Required fields.** `id`, `contentType: "brit.blob"`, `gitObjectId`, `size`, `contentFormat` (best-effort mime/format tag, `unknown` is legal), three pillars (default `{inherit: "parent-tree"}`).

**Optional fields.** `embeddedEpr` (CID, when the blob is itself an EPR-native artifact like a `.epr.json`), `binaryKind` enum (`text | image | audio | video | executable | archive | other`).

**Pillar coupling.** Usually inherit. Non-inherit when the blob *is* a learning artifact (tutorials, example notebooks, `.feature` files), a translation product, or a governance-sensitive file (`LICENSE*`, `SECURITY.md`, `.gov/**`).

**Convention.** If a blob's path matches `**/*.feature`, `**/README*.md`, or `docs/**/*.md`, importers populate lamad non-trivially.

**Open questions.** Large blobs are sharded by the protocol's Bytes tier — a single BlobContentNode points at sharded payload without changing shape. Confirmed.

---

### 5.5 BranchContentNode

**Purpose.** A branch is a stewarded view over a repo's history. In plain git, a branch is a mutable ref pointer. In brit, a branch is a first-class witnessed surface — *"main tells users one story; dev tells developers another; feature/x tells what x unlocks."* The ref is the pointer; the BranchContentNode is the view.

**Content-address strategy.** Two ids: a *stable id* (composite: `{repo_cid, branch_name, owning_agent}`) and a *versioned content-address* (CID over the current metadata, which changes whenever the branch's head or pillar fields update). Stable id is for "the main branch of this repo over time"; versioned CID is for pinning a specific snapshot.

**Reach is the central primitive.** *(Added 2026-04-11 after reach reframe.)* Every branch carries a **reach** drawn from the protocol's existing reach enum — `private`, `self`, `intimate`, `trusted`, `familiar`, `community`, `public`, `commons` — vendored from the Elohim Protocol at `schemas/elohim-protocol/v1/enums/reach.schema.json`. Reach is per-ref: the branch has the reach, commits inherit the reach of the ref they're reachable from, and moving a commit into a higher-reach ref is a reach-elevation act subject to consent. Reach is what decides whether a branch propagates at all, and to whom. An exploratory branch at `reach=self` is a local experiment the LLM is running; it does not gossip to any other peer. A `trusted` branch gossips only to the steward's relationship graph. A `public` branch gossips to the full network. **`brit push` is reach-aware** — it only announces a branch to peers whose relationship with the steward matches the branch's reach. This replaces the earlier "protection rules" framing as the primary governance gate on branches (see §14.1 #12 for the entanglement note).

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | stable branch id | Composite, not a CID. |
| `versionCid` | CID of this snapshot | Changes on update. |
| `contentType` | `"brit.branch"` | Literal. |
| `repo` | CID of `RepoContentNode` | — |
| `name` | string | Local branch name. |
| `head` | CID of `CommitContentNode` | Current head. |
| `steward` | agent id | Who decides what lands. |
| `reach` | enum from protocol reach schema | **Load-bearing governance primitive.** `private` / `self` / `intimate` / `trusted` / `familiar` / `community` / `public` / `commons`. Controls propagation, visibility, and the consent rules that apply to elevations. |
| `lamad` | object | See below. |
| `shefa` | object | See below. |
| `qahal` | object | See below. |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `readmeEpr` | CID of `PerBranchReadme` | The per-branch README ContentNode. |
| `extraProtectionRules` | CID of a qahal governance node | **Optional extras LAYERED ON TOP of the reach-change consent requirements** (e.g., "this public branch also requires a security-audit attestation"). For most repos this is null and reach alone decides the consent rules. |
| `relatedBranches` | array of stable branch ids | Branches that travel together. |
| `abandoned` | boolean | Steward marked no-longer-maintained. |

**Branch lifecycle through reach.** A typical feature branch progresses:

```text
  [LLM spikes something]           reach = self        (local, nobody sees it)
          │
          │  brit attest ... (LLM invites review)
          ▼
  [shown to co-stewards]           reach = trusted     (steward's relationship graph)
          │
          │  brit merge feature → dev  (elevation proposal)
          ▼
  [merged into dev]                reach = community   (gossipped to repo community)
          │
          │  brit merge dev → main    (elevation proposal)
          ▼
  [released to network]            reach = public      (full-network gossip)
```

Each `→` arrow is a **reach-elevation event**. Each elevation is a `MergeProposalContentNode` (§5.13) whose consent requirements come from the protocol's reach-change rules at the target reach level. The LLM drives the elevations; the protocol's existing reach-change governance decides whether each elevation succeeds.

**Lamad coupling.** The branch's *audience and unlocks*. `main.lamad = {"audience": "users", "primaryPath": "brit/getting-started"}`. `dev.lamad = {"audience": "contributors", "primaryPath": "brit/developer-onboarding"}`. `feature/new-merge.lamad = {"audience": "reviewers", "unlocks": ["p2p-merge-flow"]}`. Audience is NOT the same as reach — audience is *who this is for*, reach is *who can see it at all*.

**Shefa coupling.** Stewardship and cost. Who is the steward, what resource events have they performed on this branch, what's the affinity rating, how much attention is consumed. Abandoned branches carry a resting-state shefa.

**Qahal coupling.** At the branch level, qahal is mostly the *reach itself* (because reach is the governance primitive) plus any `extraProtectionRules` layered on top. For `main` on a brit-substrate repo, qahal typically resolves to "public-reach elevation requires steward + one other reviewer, plus a CI attestation." For personal scratch branches at `reach=self`: no consent needed, because self-reach content is self-governed by definition.

**Relationships.** Out → repo, head commit, readme, governance node (optional extras). In ← RefContentNode (a ref points at the branch), other branches (related).

**Exploratory peer model.** A branch at `reach=self` is indistinguishable from "an agent trying something out." The agent's node holds it, nobody else sees it. If the agent abandons the experiment, the branch stays local and eventually garbage-collects. If the agent wants one teammate to review, they elevate to `trusted` (which requires the teammate's node to be in the agent's relationship graph — inherited from the protocol's trust mechanism). Nothing about this is brit-specific; brit is just using the same reach-based propagation that governs every other ContentNode in the protocol. **Branches are just content, subject to the same governance as any other content.**

**Open questions.** Branch rename: lean new branch with `supersededBy` pointing at the old, since stable id includes the name. Per-steward branch identity (different agents' "main" branches are different nodes) honors the per-steward view but has UX implications. Reach inheritance: when a branch is created from another branch, does it inherit the parent's reach or default to `self`? Lean: default to `self` (LLM agent experimenting from a snapshot), with a `--reach` flag to override at creation. Discussed in §14.

---

### 5.6 TagContentNode

**Purpose.** A covenantal attestation that a specific commit represents a specific release or milestone. Mirrors a git annotated tag. Unlike stock git, brit tags always carry pillar fields — a release is an assertion to the community about what has been achieved.

**Content-address strategy.** CID over the tag's canonical serialization. Tags are immutable; re-tagging produces a new TagContentNode.

**Required fields.** `id`, `contentType: "brit.tag"`, `repo`, `name` (e.g., `v1.2.0`), `target` (commit CID), `tagger`, `message`, three pillars.

**Optional fields.** `releaseNotes` (CID of a lamad node), `signatures`, `supersededBy` (when retracted/replaced), `yanked` (boolean + reason CID), `preReleaseKind` (`rc | beta | alpha | null`).

**Pillar coupling.** Lamad: what the release unlocks, what it obsoletes. Shefa: rolled-up contributor credits between previous tag and this one. Qahal: how the release was authorized — release manager, vote, automated on merge to main, etc. Yanks carry their own qahal pointer.

**Open questions.** Lightweight tags (non-annotated): wrap with inherited pillars but emit warning. Brit-native tags should be annotated.

---

### 5.7 RefContentNode + RefUpdateContentNode

**Purpose.** A ref is the authoritative *pointer*: a named entry in a namespace like `refs/heads/main`, `refs/tags/v1.2.0`, `refs/notes/brit`. RefContentNode exists because in brit, the act of *moving a ref* is itself a governance event — it needs to be witnessed.

The separation between BranchContentNode (the view) and RefContentNode (the pointer) lets forks, mirrors, and stewardship transfers become first-class.

**Content-address strategy.** Refs form a *log*, not a single CID. Each ref update is a `RefUpdateContentNode`, chained by `previous`. The "current ref CID" is the latest update's CID. Morally similar to git's reflog, but every entry is witnessed with pillar fields.

**RefContentNode required fields.** `id` (composite: repo cid + ref path), `contentType: "brit.ref"`, `repo`, `path`, `currentUpdate` (CID of head of log), `kind` enum (`head | tag | note | pipeline | custom`).

**RefUpdateContentNode required fields.** `id`, `contentType: "brit.ref-update"`, `ref`, `previous` (CID of prior update or null), `from` (CID or null for create), `to` (CID or null for delete), `reason` enum (`fast-forward | merge | force-push | create | delete | rebase | rename`), `actor`, `timestamp`, three pillars.

**Pillar coupling.** Lamad inherits from target commit. Shefa records the steward's resource event ("stewarded-merge", "stewarded-force-push"). Qahal is **the load-bearing pillar for refs**: a force-push to `main` requires stronger authorization than a fast-forward. The qahal field of a RefUpdateContentNode must satisfy the branch's `protectionRules` or the update is rejected. A ref update without qahal authority is a protocol violation, not a repository anomaly.

**Relationships.** RefContentNode → current RefUpdateContentNode. RefUpdateContentNode → previous, from/to targets.

**Open questions.** Full log in DHT or local-only? Lean: log is local + peer-synced; the DHT carries only the current head and a compact Merkle digest of the log. Force-push policy in personal namespaces: still witnessed but with cheap `{self-governance: true}` qahal.

---

### 5.8 ForkContentNode

**Purpose.** A fork is a legitimate alternate lineage — a new covenant grown from an old one, not a defection. The ForkContentNode records provenance, reason, and stewardship transfer so forks can later **negotiate merges** with the parent on equal footing.

**Content-address strategy.** CID over `{parent_repo, fork_repo, fork_point_commit, reason, steward_new, created_at}`.

**Required fields.** `id`, `contentType: "brit.fork"`, `parentRepo`, `forkRepo`, `forkPoint`, `reason` (string or qahal node CID), `originalSteward`, `newSteward`, three pillars.

**Optional fields.** `mergeBackAgreement` (CID of qahal node — explicit terms for merging back), `relatedForks` (other forks from same parent), `healed` (CID of merge commit if the fork has been merged back).

**Pillar coupling.** Lamad: what *different* knowledge trajectory this represents. Shefa: how the new steward came to hold standing — assigned, claimed, earned. For friendly forks, the parent's steward signs a qahal attestation. For hostile forks, shefa records the network's cost of maintaining two lineages. Qahal: governance from the moment the fork exists, plus cross-fork negotiation rules for future merge-back.

**Relationships.** Out → parent repo, fork repo, fork point commit, qahal agreement nodes. In ← parent repo's forks list, fork repo's `parentRepo` field.

**Open questions.** Mirrors (read-only peer caches) are NOT forks; they share repo_id but declare themselves mirror-role in shefa. Shallow clones are bandwidth optimizations, not forks.

---

### 5.9 DoorwayRegistration

**Purpose.** The in-tree config artifact that bridges the brit repo to the elohim network. Sits at `.brit/doorway.toml` in the working tree, gets committed like any other file, and is the **first thing brit looks for after `git clone`**. Without a DoorwayRegistration, the clone is a perfectly valid git repo with no EPR resolution. With one, brit knows where to ask for the rest of the graph.

**Content-address strategy.** Two views:

1. **In-tree file.** A TOML file at `.brit/doorway.toml`, content-addressed as a `BlobContentNode` like any other tracked file. Travels via `git clone`.
2. **DoorwayRegistration ContentNode.** A parsed, signed projection of the file, addressed by CID over its canonical serialization. Lives in the doorway's projection cache and the DHT. Used when consuming the registration via the protocol rather than via the filesystem.

The TOML file is the source of truth (committed, versioned, diffable in PRs). The ContentNode is the projection.

**Required fields (the file and the node carry the same data).**

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | string | E.g. `"elohim-protocol/1.0.0"`. |
| `repoEprId` | EPR id of the RepoContentNode | Stable across clones. |
| `primaryDoorway` | URL | The steward's primary doorway base URL. |
| `fallbackDoorways` | array of URL | Optional secondary doorways for redundancy. |
| `stewardAgent` | agent id | The steward whose doorway this is. |
| `stewardSigningKey` | public key | Used to verify the file's signature. |
| `commonsFallback` | bool | If true and all doorways unreachable, brit treats the repo as commons-stewarded — value flows accumulate to commons rather than to no-one. |
| `signature` | base64 signature over the file's canonical bytes | Binds the doorway URLs and steward identity to the steward's key. |

**Optional fields.**

| Field | Type | Notes |
|---|---|---|
| `coStewards` | array of agent ids | For repos with multiple stewards. |
| `rotationPolicy` | enum | `single-steward` (any rotation requires the previous steward's signature) or `quorum` (rotation requires N of M co-stewards). |
| `web2Mirrors` | array of URL | Hint URLs for non-doorway hosting (GitHub, GitLab, etc.). |
| `createdAt` / `updatedAt` | ISO timestamp | — |

**Pillar coupling.** Lamad: typically `{rationale: "infrastructure pointer, no lamad dimension"}`. Shefa: declares the steward's stewardship of the repo as an REA agent-resource relationship. Qahal: declares the rotation policy and the constitutional layer that handles disputes if the doorway is contested.

**Bootstrap flow when a brit client encounters a clone:**

1. Look for `.brit/doorway.toml`.
2. If absent: this is a plain git repo (or a brit repo with the file in `.gitignore`, which is wrong — warn). Operate in trailer-only mode.
3. If present: parse, verify signature against `stewardSigningKey`. If signature fails, refuse to use the doorway URL — warn loudly, fall back to commons mode.
4. Try `primaryDoorway` first. On failure, walk `fallbackDoorways`. On all-failure, fall back to `commonsFallback` behavior (if true) or trailer-only mode (if false).
5. If the doorway responds, query for the RepoContentNode by `repoEprId`. Resolve linked nodes for the current head commit.

**Steward rotation flow.**

For `single-steward` policy: outgoing steward updates `.brit/doorway.toml` with new agent id, new signing key, new signature. Commits the file. The commit's qahal trailer authorizes the rotation. Anyone reading the new clone now sees the new doorway.

For `quorum` policy: any co-steward can author the rotation commit, but the commit's qahal node must reference signatures from a quorum of co-stewards. The validator rejects rotation commits whose qahal does not satisfy the policy.

**Relationships.** Out → RepoContentNode (`repoEprId`); → constitutional council qahal node (rotation policy host). In ← RepoContentNode (`doorwayRegistration`); ← clone-time bootstrap flow.

**Open questions.** §14: Is the file signed by the steward's agent key only, or should it carry a co-signature from the constitutional layer? Is the file allowed to be unsigned during cold-start (single solo developer pre-network)? Lean: yes, with explicit `unsigned: true` flag and warnings during verify.

---

### 5.10 PerBranchReadme

**Purpose.** A branch's README is a ContentNode in its own right, not just the file at the root of the branch tree. This is what makes "main tells users one story, dev tells developers another" work. The PerBranchReadme is the artifact the doorway resolves when a visitor asks "show me this branch."

**Content-address strategy.** Two views, like DoorwayRegistration:

1. **In-tree marker file.** A file at `.brit/readme.epr` (or `README.epr` at the branch's tree root) that names the canonical README content. This file may itself be a markdown blob, OR it may be a marker pointing at a separate ContentNode CID — the latter is how branch-specific READMEs that aren't files-in-the-tree work.
2. **PerBranchReadme ContentNode.** A parsed, addressed version. Carries title, body (markdown), audience, and pillar context.

**Required fields.**

| Field | Type | Notes |
|---|---|---|
| `id` | CID | — |
| `contentType` | `"brit.per-branch-readme"` | Literal. |
| `branch` | stable branch id | Which branch this README belongs to. |
| `title` | string | Display title. |
| `body` | string (markdown) OR CID of a BlobContentNode | The actual content. |
| `audience` | enum | `users | contributors | reviewers | learners | mixed`. |
| `lamad` | object | Inherits from branch, may override. |
| `shefa` | object | Inherits from branch. |
| `qahal` | object | Inherits from branch. |

**Optional fields.** `coverImage` (CID), `nextActions` (array of links to other EPRs the reader should visit), `lastUpdatedCommit` (CID of CommitContentNode).

**Pillar coupling.** Lamad: declares the audience and the path the reader is expected to be on. Shefa: typically inherits from the branch's stewardship. Qahal: typically inherits from the branch's protection rules — readers see "this README is governed by X."

**How the doorway resolves it.** When a visitor opens `https://doorway.elohim.host/repos/brit/branch/main`, the doorway looks up the BranchContentNode, finds its `readmeEpr` field, resolves to a PerBranchReadme, and renders it. The visitor sees a learner-friendly entry point to the branch, not a raw file dump.

**Relationships.** Out → branch, body blob (if separate), lastUpdatedCommit. In ← BranchContentNode (`readmeEpr`).

**Open questions.** Should the PerBranchReadme be regenerated on every commit to the branch (a derivation), or only when the steward explicitly publishes a new version? Lean: explicit publish, with a tooling hook that nudges when the underlying README file in the tree has drifted from the published PerBranchReadme.

---

### 5.11 NoteContentNode (provisional)

**Purpose.** Some metadata attaches to a commit *after* the commit is created — code reviews not available at merge time, retroactive annotations, bug reports. Git solves this with notes-refs (`refs/notes/*`); brit inherits the pattern and wraps each note as a ContentNode.

**Why provisional.** Unclear whether notes are a distinct brit type or a special case of a generic `AttestationContentNode` that lives in the protocol layer. Keeping it for completeness; may move into the protocol layer.

**Minimum sketch.** `contentType: "brit.note"`, `target` (CID of any brit ContentNode), `author`, `body` (markdown or blob CID), three pillars (typically inherit from target with overrides). Indexed via `refs/notes/*` refs.

---

### 5.12 Reserved extension slots

These are content types brit's catalog explicitly **reserves space for** but does not define here. The build-system roadmap and the governance gateway will define them; brit's job is to make sure they have somewhere to plug in.

| Reserved type | Owner doc | Where it plugs in |
|---|---|---|
| `BuildManifestContentNode` | p2p-native build system roadmap, Stage 1 | Lives as a `.build-manifest.json` file in a tree, addressed as a BlobContentNode + parsed projection. The CommitContentNode that introduces it gets a `Lamad: ...` trailer naming the build target. |
| `BuildAttestationContentNode` | p2p-native build system roadmap, Stage 1+2 | Either a commit trailer (`Built-By: <agent> capability=<cid> output=<cid>` — repeatable) OR a separate ContentNode linked from a `refs/notes/brit-builds` ref. CommitContentNode's `buildAttestations` field collects them. |
| `ReviewAttestationContentNode` | brit + governance gateway | A separate ContentNode that the `Reviewed-By:` trailer can link to via capability CID. Body carries the review text, decision, evidence. CommitContentNode's `reviewedBy` collects them. |
| ~~`MergeConsentContentNode`~~ | ~~brit + governance gateway~~ | **Superseded by `MergeProposalContentNode` (§5.13), which is now a fully specified type.** The proposal subsumes the consent record: a terminal `consented` proposal IS the authorization artifact. The linked `decision_cid` in the proposal points at the governance engine's tally record (owned by the parent EPR, not brit). |
| `StewardshipTransferContentNode` | brit + governance gateway | The qahal node that authorizes a stewardship rotation (repo-level or branch-level). Linked from the rotation commit's `qahalNode`. Carries previous steward, new steward, ratifying co-stewards, signatures. |

The catalog above is **stable**: these types can be added without changing existing ContentNode shapes, because the existing types have CID reference fields (`buildAttestations`, `reviewedBy`, `qahalNode`, `mergeBackAgreement`) that accept them.

---

### 5.13 `MergeProposalContentNode` — also known as a reach-elevation proposal

**Purpose.** A first-class, content-addressed object representing a proposed **reach elevation** — moving content from a lower-reach ref into a higher-reach ref. Every brit merge IS a reach elevation: `feature/x` (reach=trusted) merging into `main` (reach=public) is a proposal to elevate content from trusted to public. The proposal freezes the consent requirements at the moment of proposal and tracks lifecycle. Promoted from the reserved-slots table (§5.12) to a fully specified type after the merge consent critique pass (`docs/schemas/reviews/2026-04-11-merge-consent-critique.md`) and the reach reframe (2026-04-11, §5.5). Phase 2+ — explicitly out of scope for Phase 1.

**Why this is its own type (not a transient signal).** The original schema treated `brit merge` as emitting a `brit.merge.proposed` signal and waiting for a `brit.merge.consented` signal to return. The critic pass showed this loses proposals to the void the moment the proposer's CLI exits: no proposal id to remember, no `brit status` surfacing, no expiry, no withdraw path, no partial-consent ledger. Promoting the proposal to a ContentNode gives it identity, state, and a place to accumulate consent across time and peers.

**Critical framing: brit owns the proposal type; brit does NOT own governance.** The consent mechanism, the tally engine, the voting rules — all of those come from the **protocol's existing reach-change governance**. Brit reads the source and target refs' reach levels, looks up the reach-change consent rules for that transition in the parent EPR's governance primitives, freezes the resolved requirements into the proposal, and hands the proposal to the governance engine via a doorway adapter. The engine tallies and emits signals; brit consumes them. **Brit is NOT inventing merge governance** — it's using the same reach-change machinery that governs every other reach elevation in the protocol. A commit becoming visible to the network is no different from any other piece of content becoming visible to the network: the reach gate applies. This matches GitHub's model (GitHub enforces branch protection rules configured by the repo owner — it doesn't invent governance) but routes through the protocol's uniform reach-governance substrate instead of a forge-specific feature.

**Content address.** CID over the canonical serialization of the proposal's immutable core (`repo`, `sourceBranch`, `targetRef`, `proposedMergeBase`, `requirementsFrozen`, `proposer`, `createdAt`, `expiryAt`). Mutable state (`state`, `progress`, terminal `result`) lives in separate signal-driven updates, not in the content hash — otherwise every consent signal would re-CID the proposal.

**Required fields.**

| Field | Type | Purpose |
|---|---|---|
| `id` | CID | Content address of the immutable core. |
| `contentType` | `"brit.merge-proposal"` | Type discriminator. |
| `repo` | CID → RepoContentNode | The repo the proposal is against. |
| `proposer` | agent id | Who opened the proposal (LLM or human agent). |
| `sourceBranch` | ref name | The branch being merged. |
| `sourceReach` | enum from protocol reach schema | The current reach of the source branch. |
| `targetRef` | ref name | Usually `refs/heads/main` or similar higher-reach ref. |
| `targetReach` | enum from protocol reach schema | The reach of the target branch. A proposal where `sourceReach >= targetReach` is a no-elevation merge and may take a fast path. |
| `reachElevation` | boolean | Derived: `true` when `targetReach` is strictly higher than `sourceReach`. When true, the proposal is subject to reach-change consent. When false, only ordinary merge hygiene applies. |
| `proposedMergeBase` | CID → CommitContentNode | The base commit against which the merge would be computed. Frozen — a rebase of the source branch invalidates the proposal and requires a new one. |
| `proposedMergeMetadata` | object | Merge strategy (rebase / merge-commit / squash), proposed commit message, pillar trailer preview. |
| `requirementsFrozen` | array of RequirementRef | The resolved consent requirements at the moment of proposal. For reach-elevation proposals, these are derived from the protocol's reach-change governance rules for the `sourceReach → targetReach` transition (plus any `extraProtectionRules` layered on the target branch). For non-elevation merges, this is typically empty or carries only the extras. |
| `createdAt` | ISO-8601 | Proposal open time. |
| `expiryAt` | ISO-8601 | Terminal deadline. Defaults to TTL from the protection rule, fallback 48h. After expiry, the proposal transitions to `expired` and the merge cannot complete without opening a new proposal. |
| `state` | enum | See state machine below. |
| `progress` | array of RequirementProgress | One entry per frozen requirement. Each tracks: signals received, agents consenting, current tally result (`pending` / `partially-satisfied` / `satisfied` / `rejected`). |
| `pillars` | object | Lamad / Shefa / Qahal coupling for the proposal itself (not the merge it would produce). The qahal here describes the *proposal act*, not the target ref's governance — don't confuse them. |

**Optional fields.**

| Field | Type | Purpose |
|---|---|---|
| `parentProposal` | CID → MergeProposalContentNode | For cascading merges (feature → dev → main as a chain). |
| `counterpartProposal` | CID → MergeProposalContentNode | For cross-fork two-phase merges (Phase 6). |
| `withdrawnReason` | string | Set when `state = withdrawn`. |
| `rejectedReason` | string | Set when `state = rejected`. |
| `fastPath` | string | If the proposal took a fast path (self-governance, pre-satisfied), this field names which. The proposal ContentNode still exists as a record even when the fast path skips the tally phase. |

**State machine.**

```text
                         ┌──────────────┐
                         │    open      │
                         │  (initial)   │
                         └──────┬───────┘
                                │
             ┌──────────────────┼──────────────────┬─────────────┐
             │                  │                  │             │
             ▼                  ▼                  ▼             ▼
   ┌──────────────────┐ ┌───────────────┐ ┌─────────────┐ ┌──────────┐
   │ partially-       │ │   rejected    │ │   expired   │ │withdrawn │
   │ satisfied        │ │  (terminal)   │ │ (terminal)  │ │(terminal)│
   │ (intermediate)   │ └───────────────┘ └─────────────┘ └──────────┘
   └────────┬─────────┘
            │
            ▼
   ┌──────────────────┐
   │   consented      │
   │   (terminal)     │
   └──────────────────┘
```

Terminal states are immutable. `consented` triggers `brit.merge.completed` (the actual merge commit lands). `rejected`, `expired`, and `withdrawn` do not; they close the proposal without producing a merge commit.

**Lamad coupling.** What knowledge the proposed merge advances — usually inherited from the source branch's lamad and summarized for the proposal.

**Shefa coupling.** Who stands to receive recognition if the merge lands. Frozen at proposal time so retroactive stewardship changes don't invalidate an open proposal.

**Qahal coupling.** The proposal's own governance — which agent authored the proposal, which consent mechanism is active, which parent-EPR qahal_node governs it. **Do not confuse this with the consent requirements in `requirementsFrozen`** — `requirementsFrozen` says "these consents are required to land this merge," while the `qahal` pillar says "this proposal act is itself subject to these governance rules."

**Relationships.**

- → `RepoContentNode` (inbound): the repo the proposal targets.
- → `CommitContentNode` (inbound): the source branch head and the merge base.
- → parent-EPR qahal_node (outbound reference, not a brit type): the governance rules being enforced.
- → `BranchContentNode` (outbound): the target branch whose protection rules were resolved.
- ← `CommitContentNode` (produced on `consented`): the resulting merge commit gets a `Qahal-Node: <proposal_cid>` trailer recording which proposal authorized it.

**Relationship to the parent EPR's governance engine.** A doorway adapter projects:
- Brit → engine: the frozen requirements + proposer identity + target ref.
- Engine → brit: consent signals (`brit.merge.tally.progress`, `brit.merge.requirement.satisfied`, terminal `brit.merge.consented` / `brit.merge.rejected`).

The adapter is the only place that knows the engine's wire format. Different parent EPRs may use different governance engines; the adapter is the swappable layer.

**Open questions** (cross-referenced to §14):

- Default TTL policy (brit fallback vs. required from parent EPR). §14.
- Cascading proposals (first-class `MergeProposalChain` vs. LLM-side `parentProposal` walking). §14.
- Cross-fork two-phase merges (paired proposal mechanism vs. deferred to Phase 6). §14.
- Override classes (single-steward emergency fast path with post-hoc ratification). §14.
- Proposal storage: doorway only, DHT-gossipped, or both. §14.
- Whether `brit verify` walks pending proposals from an offline cache. §14.
- Can the target ref move mid-proposal? Lean: no, proposal freezes its base. §14.

---

### 5.14 Cross-cutting: what's deliberately not a new type

| Concept | Why not | Where it lives |
|---|---|---|
| Working directory | Ephemeral, not content-addressable. | Filesystem, no ContentNode. |
| Index / staging area | Ephemeral; becomes a tree on commit. | Filesystem. |
| Stash | Local state; becomes a regular commit when applied. | Local; publishable as normal commit. |
| Pack files | Storage optimization. | Storage layer (rust-ipfs / git-pack). |
| Git hooks | Local policy; may be captured as repo-level qahal rules. | Repo qahal node. |
| Submodules | A submodule boundary is a sub-`RepoContentNode` reference from a `TreeContentNode`. | TreeContentNode.subRepo. |
| Worktrees | Multiple working dirs sharing one repo — purely operational. | Filesystem. |

---

## 6. Commit trailer specification

This section is normative for any tool that writes or reads brit commit trailers.

### 6.1 Goals

1. **Round-trip through stock git** without rewriting. `git commit`, `git rebase`, `git interpret-trailers`, `git cherry-pick`, `git am`, `git format-patch` must all preserve the trailer block.
2. Be the authoritative *summary* surface — a commit whose linked ContentNodes are unavailable (offline, peer unreachable, GC'd) is still inspectable for its pillar commitments.
3. Be parseable by non-brit tooling with only an RFC-822-style scan. No base64, no JSON, no magic bytes. Boring wins.
4. Fail loudly: malformed trailers are rejected at write time by brit, and verify surfaces them at read time.

### 6.2 Trailer block location

The trailer block is the final contiguous block of `Key: value` lines in the commit message, preceded by at least one blank line separating it from the body. This matches `git interpret-trailers`'s definition. Brit uses `gix_object::commit::message::BodyRef::trailers()` to locate it.

If a commit message has no trailer block, it has no brit pillar commitments — `brit verify` rejects it as non-compliant (severity configurable; see §6.7).

### 6.3 Token namespace

All brit-introduced trailer keys are owned by the elohim-protocol app schema. The engine reserves no keys itself.

| Key | Required | Repeatable | Purpose |
|---|:---:|:---:|---|
| `Lamad:` | yes | no | Inline summary of the lamad commitment. |
| `Lamad-Node:` | no | no | CID of a linked lamad ContentNode with rich context. |
| `Shefa:` | yes | no | Inline summary of the shefa commitment. |
| `Shefa-Node:` | no | no | CID of a linked shefa ContentNode. |
| `Qahal:` | yes | no | Inline summary of the qahal commitment. |
| `Qahal-Node:` | no | no | CID of a linked qahal ContentNode. |
| `Reviewed-By:` | no | yes | Agent attestation: `<display> <agent-id> [capability=<cid>]`. |
| `Built-By:` | no | yes | Build attestation: `<builder-agent> capability=<cid> output=<cid>`. **Reserved for build-system roadmap.** |
| `Signed-Off-By:` | no | yes | DCO-style author affirmation. Inherited from git convention. |
| `Brit-Schema:` | no | no | App schema id, e.g., `elohim-protocol/1.0.0`. Defaults to `elohim-protocol/1.0.0`. |

Trailer keys NOT in this list are passed through unchanged — brit-epr's engine does not reject unknown keys; it delegates to whichever schema (or no schema) claims them.

### 6.4 Value grammar

```text
trailer-line  := key ":" SP value LF
key           := ALPHA *( ALPHA / DIGIT / "-" )
value         := <printable ASCII and UTF-8, no LF unless followed by continuation>
continuation  := LF SP    ; leading SP on the next line continues the previous value
```

- Values are UTF-8. Non-ASCII is legal.
- Length caps: **256 bytes** for pillar summary keys, **512 bytes** for CID-bearing keys, **1024 bytes** for free-form trailers like `Reviewed-By:` and `Built-By:`.
- CR/LF normalization: brit writes LF only. The verifier accepts CRLF and normalizes; a write that would introduce CRLF is rejected.
- Duplicate keys: single-valued keys (`Lamad:`, `Shefa:`, `Qahal:`, all `*-Node:`, `Brit-Schema:`) must each appear at most once. Repeatable keys may appear multiple times.
- Key order: pillar summary keys appear in canonical order Lamad / Shefa / Qahal, with each `*-Node:` immediately after its summary, but readers must accept any order.
- Value continuation: leading-space lines fold into the previous value. Brit folds at 80 columns; readers unfold before validating length.

### 6.5 Pillar-summary value microgrammar

The summary values are intentionally small and human-readable. They are not JSON — they are flat `tag:verb microform` with optional free text.

```text
lamad-value := verb SP claim [ " | path=" path-slug ] [ " | unlocks=" id-list ]
verb        := "demonstrates" | "teaches" | "corrects" | "documents" | "imports" | "refactors-no-lamad"
claim       := <up to 200 bytes of free text, no | characters>
path-slug   := <slug matching [a-z0-9/-]+>
id-list     := <comma-separated slugs>

shefa-value       := actor-kind SP contribution-kind [ " | effort=" effort-bucket ] [ " | stewards=" agent ]
actor-kind        := "human" | "agent" | "machine" | "collective"
contribution-kind := "code" | "docs" | "test" | "schema" | "review" | "infra" | "translation" | "data"
effort-bucket     := "trivial" | "small" | "medium" | "large" | "epic"

qahal-value := auth-kind [ " | ref=" ref-path ] [ " | mechanism=" mechanism ] [ " | dissent=" count ]
auth-kind   := "self" | "steward" | "consent" | "vote" | "attestation" | "council" | "retroactive"
mechanism   := <short tag>
ref-path    := <e.g., refs/heads/main>
```

The `|`-separated fragments are optional. The first segment (verb / actor-kind / auth-kind) is required.

### 6.6 Linked-node key grammar

```text
Lamad-Node: <cid>[#<fragment>]
Shefa-Node: <cid>[#<fragment>]
Qahal-Node: <cid>[#<fragment>]
```

The `<cid>` must be CIDv1 in multibase base32. Optional `#<fragment>` narrows to a sub-ContentNode within a composite node, matching the EPR URI fragment semantics.

The engine validates: syntactically valid CIDv1, multicodec in the schema's allow-list (for elohim-protocol: `dag-cbor`, `raw`, `dag-json`). The schema validates: resolving the CID yields a ContentNode whose `contentType` is in the allowed-target set — but this is a **resolution-time** check, not a parse-time check.

### 6.7 Validation levels — parser vs. validator vs. resolver

| Layer | Runs when | Rejects |
|---|---|---|
| **Parser** | Every commit read by brit-epr | Malformed trailer lines; duplicate single-valued keys; values exceeding hard length caps; invalid CID syntax in Node keys; key/value format violations. |
| **Schema validator** | `brit verify`, pre-commit hook, pre-push hook | Missing required pillar summary; verb/actor-kind/auth-kind not in enum; pillar cross-field inconsistencies; (optionally) dangling node CIDs. |
| **Resolver** | When linked nodes are actually fetched | Wrong target type; resolution failure (warning, not error — offline is legitimate). |

The parser is strict (parse-level errors mean a broken writer). The validator is strict about shape but lenient about availability. The resolver is a *reporter*, not a *rejector*.

### 6.8 Examples of well-formed commits

**Happy path — feature commit with all linked nodes:**

```text
Refactor merge conflict display to use per-hunk witness cards

The previous one-line-per-conflict rendering didn't leave room for
the pillar attribution on either side. Per-hunk cards surface the
shefa contribution of each side's author and make qahal consent
legible at conflict time.

Lamad: demonstrates per-hunk witness card rendering | path=brit/merge-ui
Lamad-Node: bafkreiabcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd
Shefa: agent code | effort=medium | stewards=agent:matthew
Shefa-Node: bafkreishefa9876shefa9876shefa9876shefa9876shefa9876shefa98
Qahal: steward | ref=refs/heads/dev | mechanism=solo-accept
Reviewed-By: Jessica Example <agent:jessica> capability=bafkreicap1111cap1111cap1111cap1111cap1111cap1111cap1111cap1
Signed-Off-By: Matthew Example <matthew@example.org>
Brit-Schema: elohim-protocol/1.0.0
```

**Partial pillars — infrastructure commit, no lamad dimension:**

```text
Bump rust-ipfs submodule to 2026-04-09 snapshot

Pulls upstream connexa fix for the Bitswap want-list deduplication.
No code changes here; pure submodule pin.

Lamad: refactors-no-lamad | path=brit/substrate-integration
Shefa: agent infra | effort=trivial | stewards=agent:matthew
Qahal: steward | ref=refs/heads/dev | mechanism=solo-accept
```

The `refactors-no-lamad` verb is the explicit "this commit teaches nothing new" answer. The validator accepts it; the UI displays a muted lamad badge.

**Build attestation trailer (reserved):**

```text
Publish elohim-storage v0.4.7 image

Lamad: documents reproducible storage build | path=brit/build-system
Shefa: machine infra | effort=small | stewards=agent:matthew
Qahal: attestation | mechanism=builder-quorum | dissent=0
Built-By: agent:builder-arm64 capability=bafkreibuildcapbuildcapbuildcapbuildcapbuildcapbuildcapbuildcap output=bafkreioutoutoutoutoutoutoutoutoutoutoutoutoutoutoutoutoutoutout
Built-By: agent:builder-amd64 capability=bafkreibuildcap2buildcap2buildcap2buildcap2buildcap2buildcap2buildcap output=bafkreioutoutoutoutoutoutoutoutoutoutoutoutoutoutoutoutoutoutXX
Signed-Off-By: Matthew Example <matthew@example.org>
```

Two builders attested independently from different hardware. Both outputs are recorded; downstream consumers can pick whichever they trust or reproduce both.

**Fork genesis commit:**

```text
Fork brit at v0.3.1 to keep WebRTC signaling alive

The upstream maintainers have decided to drop WebRTC signaling
in favor of a relay-only model. This fork preserves the WebRTC
path for low-latency household use cases.

Lamad: imports webrtc-signaling-preservation | path=brit/fork-rationale
Shefa: human infra | effort=small | stewards=agent:dan
Qahal: council | mechanism=fork-charter | dissent=0
Qahal-Node: bafkreifkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfkfk
Signed-Off-By: Dan Example <dan@example.org>
```

The qahal node points at the fork charter — the constitutional document that authorizes the fork's existence, names its new steward, and records the friendly handoff.

### 6.9 Examples of ill-formed commits

**Missing required pillar (validator rejects, parser accepts):**

```text
Fix a typo in the README.

Lamad: documents typo fix in README | path=brit/docs
Shefa: human docs | effort=trivial | stewards=agent:matthew
```

No `Qahal:` line. Validator rejects with "missing required key Qahal." The parser saw a valid trailer block.

**Duplicate single-valued key (parser rejects):**

```text
Wire new protocol.

Lamad: demonstrates new protocol
Lamad: teaches new protocol
Shefa: human code
Qahal: steward
```

Two `Lamad:` lines. Parser rejects; never reaches the validator.

**Malformed CID in Node key (parser rejects):**

```text
Add feature.

Lamad: demonstrates feature
Lamad-Node: not-a-cid
Shefa: human code
Qahal: steward
```

`not-a-cid` is not valid CIDv1 multibase. Parser rejects.

**Unknown verb (validator rejects):**

```text
Add feature.

Lamad: invents demo | path=brit/demo
Shefa: human code
Qahal: steward
```

`invents` is not in the verb enum. Validator rejects with a hint enumerating the legal verbs.

**Trailer block fused with body (parser warns or rejects):**

```text
Fix bug.
Lamad: corrects off-by-one
Shefa: human code
Qahal: steward
```

No blank line separating body from trailers. Git's own scan may or may not recognize this as a trailer block. Brit-verify treats this as a parser error; brit-write always emits the blank line.

**Summary too long (parser rejects):**

```text
Refactor.

Lamad: demonstrates <followed by 300 bytes of text exceeding the 256-byte cap>
Shefa: human code
Qahal: steward
```

Parser rejects with explicit length-cap-violation error.

### 6.10 What the canonical summary is for

The summary is not the graph — it is the *minimum viable witness*. It exists so that:

1. Stock git tools see the commitments without any brit-specific software.
2. A verifier without network access can still tell whether a commit is pillar-compliant in shape.
3. The commit's content-address (git object id) covers the commitments tamper-evidently: rewriting a trailer changes the commit hash and every downstream commit notices.
4. The commit is still legible when linked ContentNodes have been GC'd, replaced, or censored.

The linked node carries rich context that doesn't fit. It is the graph surface; the trailer is the protocol surface. **When they disagree, the trailer wins** — the linked node is enrichment, not source-of-truth for commitments.

---

## 7. Linked-node resolution protocol

### 7.1 Resolution flow

When a brit tool encounters a trailer of the form `Lamad-Node: <cid>`:

1. **Parse the CID.** If malformed, the parser already rejected the commit; resolution never runs.
2. **Look up in local store.** Brit-epr consults the configured local store (rust-ipfs blockstore, or the local git object store for legacy mode) for the CID. If present, return immediately.
3. **Try the configured doorway.** Read `.brit/doorway.toml`. Query `<primaryDoorway>/epr/<cid>`. The doorway returns the ContentNode bytes from its projection cache, or fetches and caches.
4. **Try fallback doorways.** On failure, walk `fallbackDoorways` in order.
5. **Try the DHT directly.** If a brit-transport is configured (Phase 3+), query the DHT for provider records. Fetch via the configured transport (`/brit/fetch/1.0.0` Phase 3+, or rust-ipfs bitswap earlier).
6. **Validate the fetched content.** Parse as the expected ContentNode type. If parse fails, mark the resolution **poisoned** — a CID resolving to wrong content is a stronger error than no resolution.
7. **Cache and return.**

### 7.2 Failure modes and severity

| Failure | Severity | Response |
|---|---|---|
| CID parse error | Parser error | Rejected at parse time. Never reaches resolution. |
| Local hit; wrong target type | Schema error | `brit verify` fails. "Poisoned link." |
| No local; doorway fetch succeeds; wrong target type | Schema error | Same — poisoned. |
| No local; primary doorway unreachable; fallback succeeds | Info | Logged. |
| No local; all doorways unreachable; DHT miss | Warning | "Offline" flag on the commit. Not a schema violation. |
| No local; all doorways unreachable; DHT hit; fetch times out | Warning | Offline flag. |
| Resolves but ContentNode parse fails | Schema error | Poisoned. |
| Linked node's pillars contradict the inline summary | Warning (error in `--strict`) | "Drift" flag. Trailer wins for authority. |
| `.brit/doorway.toml` signature invalid | Warning at clone time | Doorway URLs ignored; commons fallback if enabled. |
| `.brit/doorway.toml` absent | Info | Trailer-only mode. |

Rationale: "unavailable" is a legitimate state in a P2P network. "Lying" is not. The schema is strict about agreement when data is present and permissive about absence.

### 7.3 Caching policy

Brit-epr caches resolved linked nodes in whatever CID-addressed store is configured (Phase 0–1: none; Phase 2+: rust-ipfs blockstore). Cache entries are immutable — a CID always resolves to the same bytes — so cache invalidation is trivial. Cache eviction follows the host's general GC policy; brit does not pin linked nodes by default. Operators wanting long-term availability pin at the application layer.

The doorway has its own projection cache (per the doorway architecture memory). Brit's local cache and the doorway's projection cache are independent — the doorway's cache is the network's cache for web2 visitors and offline brit clients; brit's local cache is for the developer's own working tree.

### 7.4 Trailer-only mode

For airgapped CI, bootstrapping a new node from a blob, or sandboxes that forbid network access, `brit verify --trailer-only` refuses to attempt resolution. All linked-node keys are treated as opaque CIDs; only the parser and schema validator run. This is the mode a stock git host can reach for, because all it needs is the commit message itself.

---

## 8. Backward compatibility with stock git hosting

This section is the contract brit makes with the wider git ecosystem. It is the reason the elohim network can grow inside the existing developer onboarding flywheel rather than needing its own.

### 8.1 The compatibility promise

**Any brit repo is a valid git repo on any forge that hosts git.** Period. No exceptions for "but only with brit's plugin." A developer with stock git, no elohim software installed, no network access to a doorway, can:

1. `git clone https://github.com/ethosengine/brit-managed-repo`
2. `git log --format=fuller`
3. `git show <commit>`
4. `git checkout <branch>`
5. `git push origin <branch>` (if they have credentials)
6. `git rebase`, `git cherry-pick`, `git format-patch`, `git am`, `git bisect`

…all of these work. The trailers in the commit messages are visible in `git log`. They look like any other RFC-822 trailer (the same shape as `Signed-Off-By:`). They survive every history-rewriting operation that preserves commit messages, which is all of them.

### 8.2 What's in the in-tree surface (travels via `git clone`)

| Path | Purpose |
|---|---|
| Commit messages with trailers | The canonical pillar surface. RFC-822 lines. |
| `.brit/doorway.toml` | Doorway registration. Tracked file. |
| `.brit/commit-template.yaml` | Per-repo commit template (§4.2). Tracked file. |
| `.brit/readme.epr` (per branch) | Marker for the per-branch README ContentNode (§5.10). Optional tracked file. |
| `.brit/protection-rules/*.yaml` | Per-ref protection rule snapshots (CIDs are authoritative; YAML is the human-readable cache). |
| `.claude/skills/brit/SKILL.md` | LLM skill file for the repo. Optional but recommended. |
| `.build-manifest.json` (per artifact) | Reserved extension — build manifests, when introduced. Tracked files. |

These are all plain text or YAML/JSON/TOML files. They commit. They diff. They go through GitHub PRs cleanly. They survive forges that reject custom git capabilities (because they don't *use* custom git capabilities — they're just files).

### 8.3 What's in the doorway-resolved surface

The following live in the elohim network and are reached through the doorway, NOT through the git host:

- All ContentNodes addressed by CID from the trailers (`Lamad-Node:`, `Shefa-Node:`, `Qahal-Node:`).
- The PerBranchReadme rich rendering for branches (when distinct from the in-tree README file).
- The signal stream (`brit.commit.witnessed`, `brit.merge.consented`, etc.).
- Build attestations from peer builders (Stage 2 of the build system roadmap).
- The full reflog as `RefUpdateContentNode` log.
- Cross-fork merge negotiation state.
- The shefa economic event ledger for the repo.
- The qahal decision history (not just rule snapshots — the actual votes/consents).

A stock git clone gets none of this. It gets the commits and the in-tree files. That's enough to **reproduce** what the project has built (the source compiles, the tests run, the trailers are inspectable), but not enough to **participate** in its governance or its economic events. To participate, the user installs brit, points it at the repo's `.brit/doorway.toml`, and the EPR view hydrates.

### 8.4 The round-trip

**Brit → GitHub:** A `brit push` to a github remote is just a `git push` underneath. GitHub stores the commits with their trailers. PR UI shows the trailers in the commit message body. CI hooks see them as ordinary trailers. No GitHub-side software changes.

**GitHub → stock git clone:** Stock git pulls down commits with trailers. `git log` shows them. The `.brit/` directory is tracked content; it shows up. The user has a perfectly working git repo.

**Stock git → brit again:** Same user installs brit, runs `brit verify` on the cloned repo. Brit reads the commits, parses trailers, validates against the schema. If `.brit/doorway.toml` is present and reachable, it hydrates linked nodes. The user is now a participant.

**Stock-git developer makes a PR with no brit knowledge:** Their commits have no trailers. When the PR is reviewed, the steward (or their LLM agent) runs `brit attest` to add a `Reviewed-By:` trailer at merge time, and the merge commit's trailer pillars are written by the steward's tooling. The original committer's contribution is recorded in shefa with `actorKind: human, contributorAgent: <stock-git-user>` even though they never installed brit.

This is how the network grows past its own membership: every contribution is legible to brit even when the contributor isn't a brit user.

### 8.5 What's lost in the round-trip

These things degrade or vanish when a brit repo travels through stock git:

| Lost thing | Why | Mitigation |
|---|---|---|
| Linked-node enrichment | The CID points at content that isn't on the git host. | Doorway lookup recovers it when network is available. |
| Branch stewardship beyond `name` | git knows the branch name, not who stewards it. | Doorway's BranchContentNode recovers it. |
| Reflog as a log of witnessed updates | git's reflog is local, not pushed. | RefUpdateContentNode log is doorway-resolved. |
| Build attestations from peers | Live in the network, not the repo. | Doorway query for build attestations by commit CID. |
| Per-branch README distinct from `README.md` | git only sees the file. | PerBranchReadme via doorway. |
| Shefa event ledger (granular contributions) | Lives in the protocol layer. | Doorway query against the repo's shefa rail. |
| Qahal vote/consent history | Lives in the qahal layer. | Doorway query against the repo's governance rail. |

Crucially: **none of these losses break compilation, testing, or building from source.** A stock-git clone of a brit repo is a fully buildable software artifact. What is lost is the *witnessing* — the ability to see who agreed to what, who stewarded what, who attested to what. That is what the doorway restores.

### 8.6 GitHub PR UI behavior

When a stock-git developer (or a brit developer) opens a PR on GitHub against a brit repo:

- The commit messages display in GitHub's PR view with the trailers visible at the bottom of each message.
- GitHub doesn't render the trailers specially; they look like `Signed-Off-By:` does.
- If the repo has a CI hook configured (e.g., GitHub Actions running `brit verify`), the CI fails when trailers are missing or malformed. This is the "shift-left" enforcement path.
- The merge commit, when generated by GitHub's "merge" or "squash and merge" buttons, has no brit trailers — because GitHub doesn't know how to write them. **For brit repos, the recommended GitHub configuration is "rebase and merge" with a pre-merge required check that runs `brit verify`**, OR the steward merges from their CLI using `brit merge` and pushes the result.

### 8.7 The forges this contract covers

This contract is identical for: GitHub, GitLab (cloud and self-hosted), Codeberg, Gitea, sourcehut, Bitbucket, Azure DevOps, AWS CodeCommit, and any other git host that accepts standard git pushes. There is no per-forge integration in brit. The contract is exactly "git, with discipline about commit messages."

---

## 9. Signals brit emits into the protocol

Brit emits protocol-level signals when state changes. Signals are small, notifiable events (not ContentNodes themselves — they *point* at ContentNodes). They flow through the protocol's general signal bus. Brit produces; consumers decide.

Every signal names trigger, payload, and primary pillar. Pillar alignment determines routing priority — `qahal`-primary signals go to governance subscribers, `shefa`-primary to economic subscribers, `lamad`-primary to learning-feed subscribers.

### 9.1 Signal catalog

| Signal name | Trigger | Payload | Primary pillar | QoS |
|---|---|---|---|---|
| `brit.repo.created` | `RepoContentNode` published for the first time. | `{repo_cid, steward, genesis_commit_cid}` | shefa | best-effort |
| `brit.repo.registered` | `.brit/doorway.toml` is signed and accepted by the named doorway. | `{repo_cid, doorway_url, steward}` | qahal | best-effort |
| `brit.repo.stewardship.changed` | A repo's `stewardshipAgent` changes. | `{repo_cid, old_steward, new_steward, decision_cid}` | qahal | acknowledged (steward and constitutional layer ack) |
| `brit.repo.archived` | Repo marked no longer maintained. | `{repo_cid, reason_cid}` | qahal | best-effort |
| `brit.commit.witnessed` | A commit parsed, validated, and trailers valid. | `{commit_cid, git_object_id, repo_cid, pillar_summary}` | lamad | best-effort |
| `brit.commit.poisoned` | A commit's linked node fails schema validation. | `{commit_cid, key, cid, reason}` | qahal | best-effort |
| `brit.commit.signed` | A commit carries a valid signature. | `{commit_cid, signer, signature_kind}` | qahal | best-effort |
| `brit.branch.created` | A new `BranchContentNode` is published. | `{repo_cid, branch_id, steward, reach, readme_cid?}` | qahal | best-effort |
| `brit.branch.head.updated` | A branch's head advances via fast-forward or merge. | `{repo_cid, branch_id, from_commit, to_commit, update_cid}` | shefa | best-effort |
| `brit.branch.reach.elevated` | A branch's reach is elevated (e.g., `self → trusted`, `trusted → community`, `community → public`). Gossiped so peers in the new reach bracket begin seeing the branch. | `{repo_cid, branch_id, from_reach, to_reach, authorizing_proposal_cid}` | qahal | acknowledged |
| `brit.branch.reach.reduced` | A branch's reach is reduced (quarantine, withdrawal, abandonment to local). | `{repo_cid, branch_id, from_reach, to_reach, reason_cid, authorizing_proposal_cid?}` | qahal | acknowledged |
| `brit.branch.force-pushed` | A branch's head moves non-fast-forward. | `{repo_cid, branch_id, from_commit, to_commit, update_cid, authorizer}` | qahal | acknowledged |
| `brit.branch.stewardship.changed` | A branch's `steward` changes. | `{repo_cid, branch_id, old_steward, new_steward, decision_cid}` | qahal | acknowledged |
| `brit.branch.extra-protection.changed` | A branch's `extraProtectionRules` CID changes (the OPTIONAL extras layered on top of reach-change rules; reach changes have their own signals above). | `{repo_cid, branch_id, old_rules, new_rules}` | qahal | best-effort |
| `brit.branch.abandoned` | Steward marks branch abandoned. | `{repo_cid, branch_id}` | shefa | best-effort |
| `brit.ref.updated` | Any `RefContentNode` gets a new update. | `{ref_id, update_cid, reason}` | qahal | best-effort |
| `brit.tag.published` | A new `TagContentNode` is published. | `{repo_cid, tag_cid, target_commit, name}` | lamad | best-effort |
| `brit.tag.yanked` | A tag is yanked. | `{repo_cid, tag_cid, reason_cid}` | qahal | best-effort |
| `brit.fork.created` | A `ForkContentNode` is published. | `{parent_repo, fork_repo, fork_point, new_steward, reason}` | qahal | acknowledged |
| `brit.fork.healed` | A fork is merged back into its parent. | `{parent_repo, fork_repo, healing_commit_cid}` | shefa | best-effort |
| `brit.merge.proposed` | A `MergeProposalContentNode` (§5.13) is opened. | `{proposal_cid, repo_cid, target_ref, proposer, expiry_at, requirements_frozen}` | qahal | acknowledged |
| `brit.merge.tally.progress` | A tally step from the parent EPR's governance engine reports progress on an open proposal (e.g., one vote in a collective tally). | `{proposal_cid, requirement_index, signals_received, current_tally_state}` | qahal | best-effort |
| `brit.merge.requirement.satisfied` | One requirement (of possibly many) in a frozen requirement set has flipped to `satisfied`. | `{proposal_cid, requirement_index, requirement_ref, satisfying_agents}` | qahal | acknowledged |
| `brit.merge.consented` | All frozen requirements on a proposal have been satisfied — the merge is authorized. | `{proposal_cid, commit_cid, target_ref, decision_cid, consenting_kind, consenting_agents, dissent}` | qahal | acknowledged |
| `brit.merge.rejected` | A proposal's governance engine returns a denial (rather than simply running out of time). | `{proposal_cid, target_ref, reason, rejecting_agents}` | qahal | acknowledged |
| `brit.merge.expired` | A proposal's TTL has elapsed without terminal consent or rejection. | `{proposal_cid, target_ref, expired_at, last_progress}` | qahal | best-effort |
| `brit.merge.withdrawn` | The proposer cancels an open proposal. | `{proposal_cid, target_ref, withdrawn_reason, withdrawn_at}` | qahal | best-effort |
| `brit.merge.completed` | A consented proposal's merge commit has been written to the target ref. | `{proposal_cid, commit_cid, target_ref, ref_update_cid}` | shefa | best-effort |
| `brit.review.attested` | A `Reviewed-By:` trailer is published. | `{commit_cid, reviewer, capability_cid, decision}` | qahal | best-effort |
| `brit.attestation.published` | A standalone attestation ContentNode (review, build, etc.) is published. | `{target_cid, attestation_cid, kind, attester}` | qahal | best-effort |

### 9.2 Signal shape conventions

- Every signal has `{name, timestamp, producer_agent}` metadata.
- Payloads are flat; complex context delivered by CID reference, not inline.
- Idempotent keyed by `(name, primary_cid, event_timestamp)`.
- Every payload references at least one CID so the recipient can walk to full context.
- "Acknowledged" QoS means the producer expects an ack from at least one recipient (the steward, the constitutional layer, etc.) within a configurable window before retrying.

### 9.3 Subscription patterns

Phase 4+ wires brit signals into the protocol's subscription model (feed types: path, steward, community, layer — from EPR companion specs). For now, the catalog is the vocabulary; the delivery mechanism is downstream.

---

## 10. Doorway registration format

This section consolidates the `.brit/doorway.toml` file specification (also covered in §5.9 as a ContentNode type). It is duplicated here so an implementer can find it without hunting.

### 10.1 File location

`.brit/doorway.toml` at the repo root. Tracked under git. Travels with `git clone`. Edited via `brit register-doorway` or by hand.

### 10.2 Well-formed example

```toml
schema = "elohim-protocol/1.0.0"
repo_epr_id = "epr:brit"
primary_doorway = "https://doorway.elohim.host/repos/brit"
fallback_doorways = [
  "https://alt-doorway.example.org/repos/brit",
  "https://eu-doorway.example.org/repos/brit",
]
steward_agent = "agent:matthew"
steward_signing_key = "ed25519:zMq...base58..."
commons_fallback = true
created_at = "2026-04-11T20:00:00Z"
updated_at = "2026-04-11T20:00:00Z"

[rotation]
policy = "single-steward"

[[co_stewards]]
agent = "agent:matthew"
role = "primary"

# When policy = "quorum", more entries appear here with their roles and
# their public keys, and `signature` becomes a multi-signature blob.

[web2_mirrors]
github = "https://github.com/ethosengine/brit"
gitlab = "https://gitlab.com/ethosengine/brit"

[signature]
algorithm = "ed25519"
value = "base64-encoded-signature-over-canonical-bytes-above"
signed_at = "2026-04-11T20:00:00Z"
```

### 10.3 Canonical bytes for signing

The signature is computed over the file's contents with the `[signature]` section excluded. Specifically: serialize the file as TOML with the `[signature]` table removed, normalize line endings to LF, encode as UTF-8, and sign those bytes.

Verification: read the file, strip `[signature]`, recompute the bytes, verify against `steward_signing_key`. If verification fails, the file is treated as **unsigned** for the purposes of doorway resolution — `commons_fallback` (if true) kicks in; otherwise the verifier emits a hard warning and resolution proceeds in trailer-only mode.

### 10.4 Bootstrap flow for a new clone

1. After `git clone`, brit (or `brit clone` if used) checks for `.brit/doorway.toml`.
2. If absent: emit info "no doorway registration found; operating in trailer-only mode" and proceed.
3. If present: parse, verify signature.
   - On signature failure: warn loudly, fall back to commons mode if `commons_fallback = true`, else trailer-only.
4. On successful verification: try `primary_doorway` with a small timeout (5 s default). On 200, hydrate linked nodes for the current branch's head commit. On failure, walk fallbacks. On total failure, fall back as in step 3.
5. Cache the doorway URL in `.brit/.cache/doorway-state.json` (gitignored) so subsequent invocations don't re-verify on every command.

### 10.5 Steward rotation

**Single-steward policy.** The current steward updates `steward_agent` and `steward_signing_key`, recomputes the signature with the **new** key, commits the file. The commit's qahal trailer is `Qahal: steward | mechanism=self-rotation`. Old clients reading the new clone verify against the new key and accept it — but the *previous* commit (which still references the old key) provides the trust chain: `brit verify` walks history and confirms each rotation was authored by the previous steward.

**Quorum policy.** Any co-steward authors the rotation commit. The `[signature]` block becomes a **multi-signature** — N signature entries from N co-stewards. The validator rejects rotation commits whose signatures don't meet the threshold declared in `[rotation]`.

**Cold start (no previous signature to verify against).** The first commit that adds `.brit/doorway.toml` is a *root-of-trust* event. There is no chain to walk back to. The verifier accepts the first signature and trusts it. From that point forward, each rotation must be verifiable via the chain. This means a hostile steward who creates a brand new repo and signs its registration cannot be detected by signature alone — but the wider network can refuse to talk to that doorway based on agent reputation and constitutional policy. Trust chains start somewhere.

### 10.6 Key recovery is a protocol-substrate concern, not a brit concern

*(Resolved 2026-04-11.)* When a steward loses their signing key, recovery is **not** a brit-schema flow — it's the **Elohim Protocol social recovery substrate** operating one layer below brit. The steward's key material is stored as sharded blobs (Shamir-style or equivalent threshold scheme), and those shards are EPR-addressed to the steward's trust base: family, friends, institutions. Recovery is reconstituting N-of-M shards from that social graph via the protocol's existing sharding + addressing machinery.

What this means for brit:

- The `stewardSigningKey` field in `.brit/doorway.toml` is trust-rooted in the social recovery substrate, not in any brit-specific recovery procedure.
- After social-recovery completes (new shards reassembled, new key material in the steward's hands), the steward uses the normal rotation flow from §10 — authoring a rotation commit with the new `steward_agent` and `steward_signing_key`. The rotation is signed by the **recovered** key; the chain-of-trust from the previous registration commit to the new one is preserved.
- Co-steward quorum rotation (where N-of-M co-stewards can rotate out a lost key via multi-signature) remains valid as an *additional* safety mechanism layered on top of social recovery — not a replacement for it. Co-stewards protect against hostile capture; social recovery protects against accidental loss.
- Solo repos are NOT a special case. Every repo has a social recovery graph underneath because the protocol substrate guarantees one. There is no "solo repo recovery" in brit because there are no truly solo repos in the protocol's storage layer — "solo" is a UX term, not a substrate term.

### 10.7 Open questions (cross-referenced to §14)

- Should the file carry a co-signature from the constitutional layer for stronger trust? §14.
- Should `web2_mirrors` be authoritative or hint-only? Lean: hint-only.

---

## 11. Feature-module boundary (concrete plan)

This section is the concrete plan for the engine-vs-schema split from §2, expressed as crate layout, feature flags, and public surface area.

### 11.1 Default decision: one crate with feature, revisit at Phase 2

For Phase 0–1, the simplest shape:

- **`brit-epr`** — a single crate with:
  - Unconditional module `engine` — trailer parser, serializer, generic validator, schema dispatch trait `AppSchema`, CID utilities, signing adapter hooks.
  - `#[cfg(feature = "elohim-protocol")]` module `elohim` — the `AppSchema` implementation, ContentNode type ids, signal catalog constants, JSON-Schema-derived enums.
  - Default features: `["elohim-protocol"]`.

For Phase 2, when the ContentNode adapter grows substantially, the `elohim` module can be promoted to its own crate `brit-epr-elohim` with zero public-API changes to callers (re-export via the feature).

### 11.2 Crate layout (provisional)

| Crate | Purpose | Default features |
|---|---|---|
| `brit-epr` | Engine. Trailer parse/serialize, generic validator, schema dispatch, CID utilities. May contain the elohim module behind the feature flag at first. | `elohim-protocol` |
| `brit-epr-elohim` *(optional second crate, post-Phase-2)* | Pure app schema. Implements `AppSchema` for elohim-protocol. | — |
| `brit-cli` | `brit` binary with all subcommands from §3. Consumes `brit-epr` with default feature. | — |
| `brit-verify` | Standalone verify-only binary (Phase 0–1 only; folds into `brit-cli` later). | — |
| `brit-transport` *(future, Phase 3)* | libp2p wiring for `/brit/fetch/1.0.0`. Independent of `brit-epr`. | — |
| `brit-store` *(future, Phase 5)* | Local rust-ipfs blockstore wrapper. Independent. | — |

### 11.3 JSON Schema files inside brit

Following the convention of `elohim/sdk/schemas/v1/` in the parent monorepo (the brit project mirrors this layout inside its own tree, since brit ships standalone):

```text
brit/
  schemas/
    elohim-protocol/
      v1/
        _protocol.json                       # version envelope
        commit-trailer.schema.json           # the trailer-keys schema
        repo-content-node.schema.json
        commit-content-node.schema.json
        tree-content-node.schema.json
        blob-content-node.schema.json
        branch-content-node.schema.json
        tag-content-node.schema.json
        ref-content-node.schema.json
        ref-update-content-node.schema.json
        fork-content-node.schema.json
        doorway-registration.schema.json
        per-branch-readme.schema.json
        signal-catalog.schema.json
        enums/
          lamad-verb.schema.json
          shefa-actor-kind.schema.json
          shefa-contribution-kind.schema.json
          qahal-auth-kind.schema.json
        views/                               # HTTP wire shapes for doorway responses
          commit-view.schema.json
          branch-view.schema.json
          repo-view.schema.json
        CONVENTIONS.md                       # the 10 rules, mirrored from parent
```

Code generation pipeline:

- **Rust types** — generated from JSON Schema via a build-time codegen script (custom or via `schemars`/`typify`). Output goes into `brit-epr-elohim/src/generated/`. Hand-written types may wrap the generated ones for ergonomic APIs.
- **TypeScript types** — generated for any future doorway-app frontend that wants to consume brit content. Output goes into a sibling location or is published as a separate package.
- **Validation harness** — a Rust integration test in `brit-epr-elohim/tests/schema_contract.rs` that loads each `.schema.json`, asserts every published Rust struct serializes to a JSON instance that validates against its schema, and asserts every example in `schemas/elohim-protocol/v1/examples/` parses cleanly. Mirrors the convention used by `elohim-storage/tests/schema_contract.rs` in the parent monorepo.

### 11.4 Public surface (engine, unconditional)

- `AppSchema` trait — the dispatch contract from §2.3.
- `TrailerSet` — ordered, duplicate-aware map of `(key, value)` pairs with roundtrip-preserving display.
- `TrailerBlock` parser — given a commit body, locate and extract the trailer block.
- `ValidationError` — categorized variants (`ParseError`, `SchemaError`, `ResolutionWarning`).
- `Cid` newtype — thin wrapper over the `cid` crate, constrained to CIDv1 and brit's allowed codecs.
- `SignatureDescriptor` — opaque signing adapter hook.
- Engine error types via `thiserror`.

### 11.5 Public surface (elohim-protocol feature)

- `ElohimProtocolSchema` — the `AppSchema` implementor.
- `PillarTrailers` — strongly-typed wrapper around the six trailer keys (three inline, three Node).
- `ContentNodeTypeId` enum — `RepoContentNode`, `CommitContentNode`, … from §5.
- `Signal` enum — the catalog from §9.
- Free functions: `parse_pillar_trailers(body)`, `validate_pillar_trailers(&PillarTrailers)`, `render_pillar_trailers(&PillarTrailers) -> String`.
- Generated types from the JSON Schema (re-exported under `brit_epr::elohim::generated::*`).

### 11.6 What a downstream app schema replaces

Acme Carbon Ltd. wants to ship `brit-epr-acme`:

1. In their `Cargo.toml`: `brit-epr = { version = "...", default-features = false }`.
2. Their crate provides `AcmeSchema: AppSchema` declaring keys like `Carbon-Footprint:`, `Offset-Source:`, `Verification-Body:`.
3. Their CLI binary constructs `AcmeSchema` and passes it to brit-epr's engine APIs.
4. They ship JSON Schemas at `schemas/acme-protocol/v1/*.schema.json` mirroring the elohim layout.
5. They write their own skill file and template (`.acme/commit-template.yaml`).

They do not fork brit. They do not touch the engine. Their entire app schema is a focused crate that implements one trait, declares a catalog, and ships its schemas.

### 11.7 Boundary smells to watch for

During implementation, if any of these happen, the boundary is drifting and we fix it before shipping:

- Engine code references `Lamad`/`Shefa`/`Qahal` by name. **Boundary violation.**
- Engine code hard-codes CID codecs that elohim-protocol uses but others might not. **Make configurable at AppSchema construction.**
- The schema reaches into engine internals. **Expose a new engine extension point.**
- A "simple" feature needs `#[cfg(feature = "elohim-protocol")]` inside engine modules. **Move feature-gated logic into the schema module.**
- The skill file or template contains protocol-specific knowledge in engine code paths. **The CLI loads it through a schema-provided template loader, not directly.**

---

## 12. Alignment with the p2p-native build system roadmap

The p2p-native build system roadmap describes a four-stage arc — Seed, Root, Canopy, Forest — that takes the build system from "Jenkins running scripts trusted because Matthew built them" to "the build system builds itself through the protocol." Brit is the VCS substrate that makes this possible. This section enumerates the integration points.

### 12.1 Stage 0 — Seed (current state)

No integration. Brit doesn't exist yet; Jenkins runs scripts; artifacts are trusted because the maintainer built them.

### 12.2 Stage 1 — Root: BuildManifest as a file in a brit repo

The roadmap proposes a `BuildManifest` content type — a JSON file describing inputs (paths/CIDs), toolchain requirements, build command, output content type, hardware constraints. In brit's catalog, a BuildManifest is:

- A **tracked file** at `<artifact-dir>/.build-manifest.json` (or similar). Travels with `git clone`.
- A **BlobContentNode** (since it's a file) plus a **parsed projection** as `BuildManifestContentNode` (reserved type, §5.12).
- Linked from the introducing CommitContentNode via a `Lamad-Node:` trailer that points at the BuildManifestContentNode.
- The commit's `Lamad:` trailer summarizes: `documents build manifest for elohim-storage v0.4.7 | path=brit/build-system`.
- The commit's `Shefa:` trailer records: `human infra | effort=small | stewards=agent:matthew`.
- The commit's `Qahal:` trailer records: `steward | mechanism=manifest-publish`.

**What brit provides for Stage 1:**

- A way to address the manifest by CID (via the BlobContentNode).
- A way to link the commit to the manifest (via the trailer).
- A way to query "show me all build manifests in this repo" (via the doorway, walking trees for files matching `*.build-manifest.json`).
- The integrity guarantee: rewriting a build manifest changes its CID, which changes the commit hash that introduced it, which is detectable by every downstream consumer.

**What brit does not provide for Stage 1:** the manifest's *schema* — that's the build system roadmap's job. The schema lives in `genesis/protocol-schema/` (parent monorepo) or in brit's `schemas/` if the build manifest schema is brit-owned. Brit's catalog reserves the slot.

### 12.3 Stage 2 — Canopy: BuildAttestation as a trailer + linked node

Stage 2 introduces independent build + attestation by multiple steward nodes. In brit's catalog:

- `BuildAttestationContentNode` is a reserved type (§5.12).
- The reserved trailer key `Built-By:` (§6.3) carries one attestation per builder, repeatable.
- `Built-By:` records: `<builder-agent> capability=<capability-cid> output=<output-cid>`. The capability CID is the agent's hardware/toolchain profile; the output CID is what they built.
- A commit can carry many `Built-By:` lines — one per attesting builder. When two builders produce different output CIDs, both are recorded; downstream consumers see the divergence and decide what to do.
- The `brit.attestation.published` signal (§9.1) gossips each attestation as it's published.
- The doorway projects the set of attestations for any commit, so consumers can ask "how many builders agree on the output CID for commit X."

**Brit's job at Stage 2:** carry the attestations as commit trailers and gossip them as signals. The build system's job: schedule builds, run them on diverse hardware, publish the attestations.

### 12.4 Stage 3 — Forest: the build system builds itself

The roadmap describes Stage 3 as aspirational. The integration points are:

- The orchestrator becomes a coordinator zome that reads BuildManifestContentNodes from the network and assigns them to builders. Brit provides the network-addressable manifests.
- Build scheduling is governance-aware (qahal). Brit provides per-repo and per-ref qahal policies that the scheduler reads.
- Resource allocation follows shefa economics. Brit provides the per-commit shefa events that feed the economic ledger.
- The build graph IS the git graph. Brit provides the commit DAG, the tree structure, and the linked attestations — that's the build graph.
- Non-developers propose changes by forking. Brit provides ForkContentNode + the merge-back negotiation via qahal consent. A "feature request" becomes a fork with a manifest change and a merge-back proposal.
- Elohim agents are the natural reproducibility auditors. Brit provides the inputs (commits, attestations, manifests, signals); agents do the auditing.

### 12.5 Extension points reserved in this schema

To make Stage 1–3 possible without breaking changes, this schema reserves:

| Reservation | What it enables |
|---|---|
| `Built-By:` trailer key | Per-commit build attestations from multiple peers. |
| `CommitContentNode.buildAttestations` | Aggregated attestation references for a commit. |
| `BuildManifestContentNode` slot in §5.12 | The manifest itself as a first-class type when defined. |
| `BuildAttestationContentNode` slot in §5.12 | The attestation as a first-class type when defined. |
| `brit.attestation.published` signal | Network-wide notification when a new attestation lands. |
| TreeContentNode `subRepo` field | Sub-build-context boundaries inside a repo. |
| Tag `releaseNotes` CID | Release-level rollup of build attestations. |

If this schema were stable today, the build system roadmap could begin Stage 1 without asking brit for any new shapes. New ContentNode types are additive; new trailer keys are additive; new signals are additive.

---

## 13. Target-persona scenarios

These scenarios are the spec. If the schema can't support them, it's incomplete.

### 13.1 Scenario A — Dan and an LLM agent collaborate on a feature branch

**Setting.** Dan is a developer at EthosEngine (per his persona profile, public-reach, affinities include rust/holochain/distributed-systems/open-source/privacy). He's working in a brit repo cloned from `https://github.com/ethosengine/brit-feature-prototype`. The repo's `.brit/doorway.toml` points at a steward's doorway. Dan has the elohim-app open in another tab.

**Step 1.** Dan asks his LLM agent (running in a Claude Code session in his terminal) to add a new feature: per-hunk witness card rendering for merge conflicts. He gives a one-paragraph description.

**Step 2.** The LLM loads `.claude/skills/brit/SKILL.md` (it sees the file in the repo and recognizes the skill applies). It reads `.brit/commit-template.yaml`, sees that the active learning paths include `brit/merge-ui` and `brit/llm-authoring`, and that Dan's agent id is `agent:dan` with default actor-kind `human`.

**Step 3.** The LLM creates a feature branch by running `brit branch feature/per-hunk-witness-cards`. Brit creates the git ref AND a `BranchContentNode` with `lamad.audience = "reviewers"`, `lamad.primaryPath = "brit/merge-ui"`, `qahal.protectionRules = <inherited from dev>`, and emits `brit.branch.created`. Dan's elohim-app UI shows a new branch card.

**Step 4.** The LLM writes the implementation across several files. It then runs `brit add <files>` and `brit commit -m "Refactor merge conflict display to per-hunk witness cards" --lamad "demonstrates per-hunk witness card rendering | path=brit/merge-ui" --shefa "agent code | effort=medium | stewards=agent:dan" --qahal "self | ref=refs/heads/feature/per-hunk-witness-cards | mechanism=solo-author"`. Brit-epr parses the trailers, validates against the schema, writes the commit. The commit's CID is computed, and `brit.commit.witnessed` fires.

**Step 5.** The LLM iterates a few times, fixing tests, each iteration producing another commit with proper trailers. After 4 commits the feature is done. The LLM runs `brit push origin feature/per-hunk-witness-cards`. Brit pushes to GitHub (just `git push` underneath) AND emits `brit.commit.witnessed` signals to the doorway, which projects them into the elohim-app's feed.

**Step 6.** Dan switches to his elohim-app tab. He sees the new branch with 4 commits, each with three colored badges (one per pillar). He opens the branch's PerBranchReadme, which the doorway has rendered from the LLM's draft README content. Each commit expands to show the linked-node summaries (the LLM didn't set Lamad-Node CIDs because the path is already known from the trailer). Dan reads through, satisfied.

**Step 7.** Dan clicks "propose merge to dev" in the UI. The UI calls the doorway, which constructs a merge proposal as a `brit.merge.proposed` signal targeting `refs/heads/dev`. Because `dev`'s qahal protection requires `steward-accept`, the merge waits for Dan's consent (he is the steward of this repo).

**Step 8.** Dan reads the diff one more time in the UI, clicks "consent." The UI signs the consent decision with Dan's agent key, publishes a `MergeConsentContentNode` (reserved type, §5.12), and the doorway emits `brit.merge.consented` followed by `brit.merge.completed`. The doorway also performs the merge on the steward's behalf — running `brit merge` on the server side, producing a merge commit whose `Qahal-Node:` trailer points at the new MergeConsentContentNode.

**Step 9.** Dan's local clone, on next `brit pull`, sees the merge commit with the consent trailer. The trailer is fully self-contained (no doorway query needed to read it), but the linked qahal node is fetched from the doorway for full context. Dan's elohim-app shows the merge as completed.

**End state.** A 4-commit feature branch landed on dev with full pillar coverage, signal trail, and human consent. Dan never had to think about the trailer grammar — the LLM handled it via the skill file. The human's role was strategic (what to build) and consensual (approving the merge), not mechanical.

### 13.2 Scenario B — Stock git clone from GitHub, later upgraded to brit

**Setting.** A developer in São Paulo (call her Sofia) has never heard of the elohim protocol. She's looking at a Rust library on GitHub: `https://github.com/ethosengine/brit-managed-lib`. She wants to use it.

**Step 1.** Sofia runs `git clone https://github.com/ethosengine/brit-managed-lib`. Stock git, no brit installed. The clone succeeds.

**Step 2.** Sofia runs `git log --format=fuller`. She sees commits with messages that include trailers at the bottom:

```text
Lamad: demonstrates connection retry with exponential backoff | path=brit-lib/networking
Shefa: agent code | effort=small | stewards=agent:matthew
Qahal: steward | ref=refs/heads/main | mechanism=solo-accept
Reviewed-By: Jessica Example <agent:jessica> capability=bafkrei...
```

She wonders what these mean, but they don't break anything. Git treats them as ordinary trailers, like `Signed-Off-By:`. She reads a few — the commit messages are unusually disciplined.

**Step 3.** Sofia builds the library (`cargo build --release`). It works perfectly. The fact that brit metadata is on the commits is irrelevant to compilation. She integrates the library into her project and ships it.

**Step 4.** Two weeks later, Sofia sees a bug. She opens an issue on GitHub. Someone replies: "we use brit for governance — if you want to participate in the fix, install brit and point it at our doorway. Here's how." They link to a quickstart.

**Step 5.** Sofia installs brit (`cargo install brit-cli`). She runs `brit verify` in her clone. Brit reads `.brit/doorway.toml`, sees `primary_doorway = "https://doorway.elohim.host/repos/brit-managed-lib"`, verifies the signature against the steward's public key (also in the file), and connects.

**Step 6.** Brit hydrates linked ContentNodes for the recent commits. Sofia's terminal now shows rich pillar info: which contributor ages each commit belongs to, which learning paths the commits advance, which qahal decisions authorized the merges. The elohim-app (which Sofia also installed) shows a network view of the repo's recent activity.

**Step 7.** Sofia writes a fix. She runs `brit commit` with the trailer flags (the LLM in her dev environment helped — it loaded the skill file from the repo). She pushes. Her commit is now witnessed, attributed to her agent id (which she registered when installing brit), and the steward sees it in their elohim-app feed for review.

**Step 8.** The steward reviews and merges. Sofia's contribution is recorded in the repo's shefa ledger; she has earned standing as a contributor. None of this required GitHub to do anything special — it required Sofia to install brit and point it at the doorway. The web2 surface (GitHub) and the elohim surface (doorway) coexist.

**End state.** A developer with no prior elohim knowledge cloned, built, and contributed to a brit-managed repo, with the protocol layer activating only when she chose to participate. The onboarding flywheel works: the repo lives on GitHub for discoverability, the protocol layer lives in the doorway for governance, and the bridge between them is one config file.

---

## 14. Open questions

This section is the honest list of decisions the schema doesn't make. Each one needs human judgment before the implementation phase that depends on it.

### 14.1 Hard design decisions

1. **One crate or two?** §11.1 punts on whether `brit-epr-elohim` is a separate crate or a feature-gated module. Lean: one crate for Phase 0–1, split at Phase 2 if needed. Some reviewers will want the split immediately for legibility. Needs a call.

2. **Doorway signature trust model.** Is `.brit/doorway.toml` signed by the steward's agent key alone (as in §10), or should it carry a co-signature from the constitutional layer for stronger trust? Lean: steward-only at v1, with a future `constitutional_endorsement` field that the constitutional layer can populate later. The cold-start case (solo developer pre-network) makes constitutional co-signature impossible at first.

3. **Steward key recovery.** *(Resolved 2026-04-11.)* Recovery is **not** a brit-schema concern — it lives in the Elohim Protocol's **social recovery substrate**. A steward's signing key material is stored as sharded blobs (Shamir-style or equivalent threshold scheme), and those shards are EPR-addressed to the steward's trust base — family, friends, institutions — via the same content-addressing mechanism the protocol uses for all other storage. Recovery is reconstituting N-of-M shards from that social graph. The brit schema assumes this layer exists and does not define a parallel mechanism. Implications for §10 (DoorwayRegistration): the `stewardSigningKey` field and its rotation policy are trust-rooted in the social recovery graph, not in a brit-specific recovery flow. Co-steward quorum rotation (option b above) remains valid for repos that want it, but it's an *additional* safety mechanism layered on top of social recovery, not a replacement.

4. **`brit merge` blocking vs. async.** *(Resolved 2026-04-11 after critic pass at `docs/schemas/reviews/2026-04-11-merge-consent-critique.md`.)* **Async-by-default with a first-class `MergeProposalContentNode` lifecycle (§5.13).** `brit merge` opens a proposal, freezes the requirement set, prints JSON to stdout, exits 0. The proposal lives with a TTL. `--wait` is reframed as polling-with-cap, not indefinite blocking. See §3.9 for the command surface, §5.13 for the proposal type, §9 for the updated signal catalog (`brit.merge.tally.progress`, `brit.merge.expired`, `brit.merge.withdrawn`, `brit.merge.requirement.satisfied`).

   **Critical clarification: brit does not own governance.** The consent requirements for a protected ref — who can consent, what threshold, what mechanism, what TTL — come from the governance primitives of **the parent EPR that the repo is a part of**. Brit reads `protectionRules` (which points at a qahal_node CID in the parent EPR's governance context), fetches the rule, and freezes the resolved requirements into the proposal. The tally itself runs in the parent EPR's governance engine (governance gateway, constitutional council, collective voting mechanism — whichever the parent EPR declares). Brit consumes the resulting `brit.merge.consented` / `brit.merge.rejected` signal. This is analogous to GitHub branch protection rules today: GitHub doesn't invent governance, it *enforces* policies configured by the repo owner. Brit enforces policies configured by the parent EPR.

   Consequence: `MergeProposalContentNode` is a brit type (brit owns the proposal lifecycle, expiry, frozen requirements), but the consent mechanism and tally are NOT brit concerns. An adapter at the doorway projects the brit proposal into whatever the parent EPR's governance engine expects, and projects the engine's verdict back into brit signals.

   This resolution co-resolves partially with §14.1 #12 (protection rules DSL): the DSL must be expressive enough to reference a governance qahal_node in the parent EPR, name the consent mechanism, and declare TTL defaults — but it does NOT need to reimplement the mechanisms themselves.

   **Phase 1 scope impact: merge consent is explicitly OUT of Phase 1.** Phase 1 ships the trailer foundation (parser + validator + `brit verify`). `MergeProposalContentNode`, the `brit merge` flow, and the parent-EPR adapter are Phase 2+ work. The Phase 1 `brit verify` binary does NOT open proposals or gate merges.

5. **`Built-By:` trailer ergonomics.** §6.3 reserves `Built-By:` as a repeatable trailer for build attestations. With many builders, the trailer block could grow large. At what point do we move attestations out of the trailer and into a separate `refs/notes/brit-builds` log? Lean: trailer is fine for ≤5 attestations, log is required beyond that. Needs calibration against real attestation volumes.

6. **Pillar summary enums — closed or extensible?** *(Resolved 2026-04-11.)* **Closed.** The elohim-protocol app schema's vocabulary (verbs, actor-kinds, auth-kinds) is fixed. The extensibility axis is NOT "per-repo enum additions" — it's **the feature-module boundary itself**. If another app needs different primitives (e.g., a music-composition app, a carbon-accounting app, a biological-sequence app), it supplies its own EPR manifest via its own app schema that plugs into the same `brit-epr` engine. The brit-epr engine counts trailers without knowing which vocabulary it's counting; the app schema provides the vocabulary. This preserves round-trip interop (every brit-using-elohim-protocol repo has the same vocabulary), keeps per-repo `commit-template.yaml` honest (it configures *defaults and helpers*, not new enum values), and matches the "one engine, many app schemas" separation from §2. §6.5 and §4.3 should be read as closed-vocabulary specs. Per-repo templates that appear to extend enums are actually carrying **per-repo defaults** (e.g., "this repo prefers `refactors-no-lamad` when the commit touches only test files") — not new enum values.

7. **Agent-scoped vs. repo-scoped branch identity.** §5.5 uses a composite `{repo_cid, branch_name, owning_agent}` as the stable id. This means two agents with a `main` branch on the same repo have two different BranchContentNodes. Honors the per-steward view, but breaks the git intuition of "one main per repo." Lean: keep the composite, add tooling that hides it for the common case where there's only one steward.

8. **Notes as a distinct type.** §5.11 flags this. Needs a call before Phase 4. Lean: defer; treat notes as `AttestationContentNode` instances when the protocol layer defines that type.

9. **Sub-repos vs. submodules.** §5.3 sketches sub-repos as a TreeContentNode `subRepo` reference. How that interacts with gitoxide's existing submodule support is not worked out. Could be Phase 5+; flag for now.

10. **Skill file location.** §4.1 leaves open whether the skill lives in the repo (`.claude/skills/brit/SKILL.md`) or in the LLM harness (`~/.claude/skills/brit/`). Lean: in-repo, for discoverability and per-repo customization.

11. **Per-branch READMEs: derivation vs. publish.** §5.10 punts on whether `PerBranchReadme` is regenerated on every commit (derivation) or only on explicit publish. Lean: explicit publish, with a tooling hook that nudges when the source file in the tree has drifted.

12. **~~Force-push policy DSL.~~ Mostly dissolved by the reach reframe.** *(Resolved 2026-04-11.)* The original open question asked how to express a bespoke branch-protection DSL. After the reach reframe (§5.5), the central governance gate on a branch is its **reach** (from the protocol's existing reach enum — `private` → `self` → `intimate` → `trusted` → `familiar` → `community` → `public` → `commons`). Reach elevations are subject to the protocol's existing reach-change governance, which already exists for every ContentNode in the system. Brit does not invent a new DSL for this — it vendors the reach enum and consumes the reach-change consent rules the parent EPR already supplies. What remains of the original question is a small residual: `BranchContentNode.extraProtectionRules` is an optional CID pointer to "additional consent requirements layered on top of the reach-change rules" (e.g., "this public branch ALSO requires a security-audit attestation"). The shape of these extras is simple and composable — each extra is just a reference to a qahal_node in the parent EPR that adds a requirement to the frozen set of a MergeProposalContentNode. No DSL, no custom grammar. The entanglement with §14.1 #4 is thereby reduced: MergeProposalContentNode's `requirementsFrozen` array is the union of (reach-change rules at target reach) + (extras from `extraProtectionRules`, if any). Both come from the same substrate (qahal_node references in the parent EPR), not a brit-specific language.

13. **What `brit fork` does to the new repo's history.** Does the fork replay every commit with new CIDs (because the fork's repo_id changed), or does it inherit the parent's commit CIDs unchanged? Lean: inherit unchanged — the fork is a new repo with the same commit DAG, not a new commit DAG. The ForkContentNode itself records the divergence; commit CIDs don't change. But there are subtleties around how `RefContentNode`s are scoped (per-repo) that need working out.

14. **Build attestation capability claim location.** §5.12 reserves `BuildAttestationContentNode`. Where does the agent's *capability claim* (e.g., "I am an arm64-musl builder with rust 1.85") live? In the trailer (`Built-By: ... capability=<cid>`)? In the linked attestation node? Both? Lean: both — capability CID in the trailer for fast inspection, full capability claim in the linked node.

15. **`.brit/` directory hidden by `.gitignore`?** What if a developer adds `.brit/` to `.gitignore`? Then the doorway registration doesn't travel via clone. This is a misconfiguration, but the verifier should warn loudly about it. Lean: warn, treat as unsigned/absent.

### 14.2 Areas where the hybrid (c) trailer+linked-node design feels stressed

Writing this document surfaced two places where the locked-in hybrid design needs another look. Neither requires abandoning the design, but both suggest a "calibration pass" before the schema is declared stable.

1. **The inline summary microgrammar is load-bearing and might be too narrow.** §6.5 forces the LLM (or human) to pick a verb from a fixed list (`demonstrates | teaches | corrects | documents | imports | refactors-no-lamad`). Real commits sometimes don't fit any of these — a refactor that *teaches* a pattern, a fix that *demonstrates* a debugging technique, a docs change that *corrects* a misconception. The first segment is required, and there's no escape hatch. Should we add a `mixed:` prefix that lets the value carry multiple verbs? Or accept that the choice is lossy and the linked node carries the nuance? Needs a writing-real-trailers exercise before locking the grammar.

2. **`Reviewed-By:` length cap is tight.** §6.4 caps `Reviewed-By:` at 1024 bytes, but the trailer is supposed to be the canonical surface. Code reviews often have substantive comments. The clean answer is that the trailer carries the *agent and capability and decision*, and the rich review (the actual prose) lives in a `ReviewAttestationContentNode` linked from the trailer's capability CID. But that means the canonical-surface principle ("trailer wins when it disagrees with the linked node") doesn't quite apply to reviews — the trailer can't disagree because it doesn't carry the reviewer's prose. May need a note in §6 acknowledging this asymmetry.

3. **Build attestations strain the trailer block.** Per §14.1 #5, repeatable `Built-By:` trailers with many builders bloat the commit message. The hybrid principle says the trailer is the witness; the linked node is the enrichment. With build attestations, the linked node may need to be the *primary* surface and the trailer just a count + summary. This is a design pressure on the hybrid model that we should resolve before Stage 2.

### 14.3 Things deliberately out of scope

- Transport (`/brit/fetch/1.0.0`, libp2p wiring) — Phase 3.
- DHT announcement and peer discovery — Phase 5.
- Per-branch README rendering tooling — Phase 4.
- Migration strategy for the elohim monorepo itself — needs its own design doc.
- Interaction with the lamad/shefa/qahal app schemas' own ContentNode vocabularies — that's the protocol layer's problem.
- Upstream-contribution shape for the engine half to gitoxide — TBD after Phase 1 stabilizes.
- The actual JSON Schema files — they will be drafted in the schema codegen task that follows this design doc.
- Build manifest schema details — owned by the build system roadmap.

### 14.4 LLM-first surprises

Two things surfaced from the LLM-first reframing that earlier drafts of this document hadn't fully internalized:

1. **The skill file is part of the schema, not just documentation.** Earlier drafts treated the skill as a "nice to have" — something an LLM might consult. The reframing makes it load-bearing: without a skill file in the repo, an LLM has to reason about pillar grammar from the trailer spec alone, and that's both expensive and error-prone. Treating the skill file as a *first-class artifact of the schema* (with its own format, location, and content requirements) is a shift this document tries to make explicit in §4.

2. **The per-repo template carries living state, not just defaults.** The template isn't static; it's enriched at commit time by querying the doorway for current learning paths, contributors, and protection rules (§4.3). This means the template is the *surface where the schema talks to the LLM*, and the doorway is the *surface where the schema talks to the network*. The two are coupled. An earlier draft of this document had the template as a static defaults file; the reframing exposed that as insufficient.

---

## 15. Cross-references

### 15.1 Documents this design touches

- **Brit roadmap:** `docs/plans/README.md` (within brit) — the seven-phase decomposition. This schema is the substrate for Phases 0–1 directly and Phases 2–6 by implication.
- **Phase 0+1 plan:** `docs/plans/2026-04-11-phase-0-epr-trailer-foundation.md` (within brit) — will be revised after this schema lands.
- **P2P-native build system roadmap:** the parent-monorepo document that defines the four-stage arc. §12 of this document is the integration plan; the build manifest schema itself stays in the build system roadmap's tree.
- **EPR developer guide:** the parent-monorepo user-facing explanation of three-pillar links. §1 and §6 in this document refer to the same metadata-envelope concept; brit's commit trailer IS the metadata envelope projected onto a git commit.
- **Doorway service:** the elohim doorway is the bridge brit consults via `.brit/doorway.toml`. §10 of this document specifies the brit-side contract; the doorway's response shape lives in the doorway service's own docs.

### 15.2 Which sections feed which phases

| Section | Primary consumer phase | Secondary consumers |
|---|---|---|
| §2 engine/schema split | Phase 0 | Phase 2 (adapter), upstream gitoxide contribution |
| §3 CLI command surface | Phase 0–1 (verify only); Phase 2+ (full CLI) | Every phase |
| §4 skill + template | Phase 1 onward | Every developer-facing phase |
| §5 ContentNode catalog | Phase 2 (adapter) | Phase 4 (branches), Phase 6 (forks) |
| §6 trailer spec | Phase 0 + Phase 1 | Every phase (trailers are forever) |
| §7 linked-node resolution | Phase 2 + Phase 3 | Phase 5 (DHT) |
| §8 backward compat | Always | Onboarding, CI integration |
| §9 signals | Phase 3 + Phase 5 | Phase 4 (branch signals) |
| §10 doorway registration | Phase 1 onward | Every clone-time scenario |
| §11 feature-module boundary | Phase 0 | Any downstream fork |
| §12 build system alignment | Stage 1 of the build roadmap (Future) | Stages 2, 3 |
| §13 scenarios | Every phase (acceptance test) | Documentation |
| §14 open questions | Every phase | Human reviewers before implementation |

### 15.3 Related conventions in the parent monorepo

This document mirrors several conventions used by the parent elohim monorepo. Brit ships standalone, so the conventions are recreated inside brit's tree, but the shape is the same:

- **JSON Schema source-of-truth pattern.** Mirrors `elohim/sdk/schemas/v1/` in the parent. Brit's version lives in `brit/schemas/elohim-protocol/v1/`. See §11.3.
- **`CONVENTIONS.md` for view schemas.** Mirrors `elohim/sdk/schemas/v1/views/CONVENTIONS.md`.
- **Schema contract test pattern.** Mirrors `elohim/elohim-storage/tests/schema_contract.rs`. Brit's version lives in `brit-epr-elohim/tests/schema_contract.rs`.
- **Protocol-vs-app-layer schema split.** The protocol monorepo has `lamad/manifest.json` for app vocabulary distinct from the protocol's enums. Brit's elohim-protocol app schema is to brit-epr what lamad/manifest.json is to the protocol schemas.

---

## Appendix A — Quick reference: trailer keys

| Key | Required | Repeatable | Value shape | Cap | Owner |
|---|:---:|:---:|---|---|---|
| `Lamad:` | yes | no | verb + claim + optional modifiers | 256B | elohim-protocol |
| `Shefa:` | yes | no | actor-kind + contribution-kind + modifiers | 256B | elohim-protocol |
| `Qahal:` | yes | no | auth-kind + modifiers | 256B | elohim-protocol |
| `Lamad-Node:` | no | no | CIDv1 + optional fragment | 512B | elohim-protocol |
| `Shefa-Node:` | no | no | CIDv1 + optional fragment | 512B | elohim-protocol |
| `Qahal-Node:` | no | no | CIDv1 + optional fragment | 512B | elohim-protocol |
| `Reviewed-By:` | no | yes | display + agent + capability | 1024B | elohim-protocol |
| `Built-By:` | no | yes | builder-agent + capability + output | 1024B | elohim-protocol (reserved for build roadmap) |
| `Signed-Off-By:` | no | yes | display + email (DCO) | 1024B | inherited |
| `Brit-Schema:` | no | no | schema id | 256B | engine |

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
| `DoorwayRegistration` | Repo-to-doorway bridge config. | Phase 1 (file format) / Phase 2 (ContentNode projection) |
| `PerBranchReadme` | Branch-scoped README ContentNode. | Phase 4 |
| `NoteContentNode` (provisional) | Retroactive attestation. | Phase 4 or deferred to protocol layer |
| `BuildManifestContentNode` (reserved) | Build recipe. | Build system roadmap Stage 1 |
| `BuildAttestationContentNode` (reserved) | Peer-attested build result. | Build system roadmap Stages 1–2 |
| `ReviewAttestationContentNode` (reserved) | Standalone review prose + decision. | Phase 4 + governance gateway |
| `MergeConsentContentNode` (reserved) | Qahal authorization for protected merge. | Phase 4 + governance gateway |
| `StewardshipTransferContentNode` (reserved) | Qahal authorization for stewardship rotation. | Phase 6 + governance gateway |

## Appendix C — Quick reference: signals (grouped by emitting phase)

- **Phase 1 (from trailers only):** `brit.commit.witnessed`, `brit.commit.poisoned`, `brit.commit.signed`, `brit.review.attested`.
- **Phase 1+2 (registration):** `brit.repo.registered`.
- **Phase 2 (adapter):** `brit.repo.created`, `brit.tag.published`.
- **Phase 4 (branches/merges):** `brit.branch.created`, `brit.branch.head.updated`, `brit.branch.force-pushed`, `brit.branch.stewardship.changed`, `brit.branch.protection.changed`, `brit.branch.abandoned`, `brit.ref.updated`, `brit.merge.proposed`, `brit.merge.consented`, `brit.merge.rejected`, `brit.merge.completed`.
- **Phase 6 (forks):** `brit.fork.created`, `brit.fork.healed`, `brit.repo.stewardship.changed`, `brit.repo.archived`, `brit.tag.yanked`.
- **Build roadmap Stages 1–2:** `brit.attestation.published`.

---

*End of Brit — Elohim Protocol App Schema Manifest v0.2.*
