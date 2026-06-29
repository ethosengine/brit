---
id: canonical-epr-meta-git-bridge
---

# Canonical EPR-Meta & the Git Bridge — Next-Generation `epr-meta` Design

**Date:** 2026-06-29
**Status:** Draft (brainstorm)
**Author:** Matthew Dowell + Claude Opus 4.8
**Extends:** `docs/specs/2026-04-12-brit-design.md` (master) — this is the design session for **Phase 2 (ContentNode adapter)** plus its named-but-unspecified *schema-versioning* risk.
**Companion:** `docs/specs/2026-04-27-build-contract-before-push-design.md` (build-contract is the *push-time* sibling of this *seed-time* contract).
**Consumes:** `elohim-epr = "0.1"` — the protocol's canonical EPR codec, **published 2026-06-29** to the internal `elohim` Nexus registry (CIDv1 · dag-cbor · sha2-256 · ed25519). This crate is the source of truth for content addressing; brit does not reproduce it.

---

## TL;DR

Make brit the **canonical-EPR interoperability substrate**. The next-generation `epr-meta` is not a directory-local YAML governance file read by a Python hook (the parent monorepo's emergent form) — it is a **content-addressed artifact** (`EprMeta`) whose identity is a real CIDv1 over canonical DAG-CBOR bytes, composed into a node-level **import/export contract + lockfile** (`NodeSeed`), and blessed by a **separable runtime-supplied notarization** edge.

The load-bearing reframe: **canonical-first, git-as-bridge.** The protocol-canonical EPR form (via `elohim-epr`) is primary and P2P-native; git's object formats are reached through an **internal bridge**, not treated as the substrate. One content-addressing primitive runs coherently from **filesystem bytes → git bridge → P2P dataplane** — *from bits to apps* — and is byte-precise enough to **reimplement in machine code**, verified against shared golden CID vectors. This sets up a future **command-by-command migration** of brit's verbs to canonical-EPR-native operation.

These artifacts are authored using, and are the first instances of, a **next-generation memory/discipline** intended to be mastered here in brit and brought back to the whole monorepo — superseding the parent's `epr_meta.py` / `cite_graph.py` / `placement-audit` toolchain rather than porting it inward.

---

## 1. Problem

Two emergent forms have independently grown toward the same missing primitive and stopped short of it.

**1.1 The parent monorepo's `.epr-meta`** is "private-reach authored intent that hasn't earned cross-machine notarization" — stated in prose, unbuilt. Concretely: `id:` is a filesystem slug (resolved by live `rglob`), **nothing is actually CID-addressed**, ~3 of 9 rule predicates are declared-but-dead, there is no node-seed / lockfile / reproducible snapshot, and notarization is conflated with authoring (a `deny` binds the instant a file lands on disk, with no witness and no safety net).

**1.2 brit's own engine** mints `BritCid` as `BLAKE3(sort-keys-JSON) → 64-hex`. This is precisely the P2P design-gate's named anti-pattern ("a bare hash exposed as a content address"): no multihash, no multicodec, no version discriminator, and it will **not** interoperate with the rest of the protocol (IPFS/Holochain blob plane, the `elohim-epr` atom codec). The code comments concede it is interim.

**1.3 The interoperability gap.** There is no single, compiler-grade, content-addressed contract describing what an EPR node *is* and what it *accepts and emits* — no artifact that makes "import this node," "export this node," "back up this node," "verify this restored node," and "this node speaks schema versions X, Y, Z" into one reproducible, hash-verifiable operation. That artifact is what this design defines.

## 2. Insight

**Content-addressing makes identity = shape + content, which lets you separate the artifact from its blessing.** Three consequences drive the whole design:

1. **One primitive, bits to apps.** A single canonical-bytes→CID function addresses raw file bytes (blobs), directory structure (trees), the governance/seed manifest (`EprMeta`), and the node rollup (`NodeSeed`) — the *same* CID identifies that content on the local filesystem, inside the git bridge, and on the P2P dataplane. The function is small and fully specified (DAG-CBOR deterministic encoding + CIDv1), so it is reimplementable in any language or in machine code; **conformance is proven against shared golden vectors**, not against a single implementation.

2. **Git is a bridge target, not the substrate.** Git is already a Merkle DAG, so the bridge is a clean re-encoding (git's object-format + sha-1/sha-256 ↔ EPR's dag-cbor + sha-256), not an impedance mismatch. Treating git as *external* — the way the monorepo's `bridges/` crates treat ActivityPub or hREA — is what makes the future **command-by-command migration** tractable: each brit verb gains a canonical-EPR-native path with the git bridge as its compatibility layer.

3. **Two layers: static identity, then runtime blessing.** Layer 1 (the seed) is content-addressed, fully offline-constructible on a single device (the hub-optional floor), and carries *no authority*. Layer 2 (notarization) is a separable signed edge added *only when the network validates it* — locally an ed25519 attestation, in-protocol the DHT's `validate(op)` witness convergence (the "social and observation flows"). The seed bytes never change when blessing arrives.

## 3. The canonical CID engine (the foundation)

brit adopts the protocol's published codec verbatim. No re-implementation.

- **Identity:** `CIDv1`, multicodec **`0x71` (dag-cbor)**, multihash **`0x12` (sha2-256)** — `bafyrei…`. Raw blob bytes use multicodec **`0x55` (raw)** — `bafkrei…`. Both wrap the *same* sha2-256 the protocol already computes; the CID is the self-describing envelope, not a different hash.
- **Canonical bytes:** DAG-CBOR per RFC 8949 §4.2.1 (sorted map keys, shortest-form integers, no indefinite-length items), via `serde_ipld_dagcbor` + `ipld-core`. `elohim-epr::cbor::decode_strict` enforces round-trip canonicality (re-encode must be byte-identical).
- **Source of truth:** the **`elohim-epr` crate** (`elohim-epr = "0.1"`, registry `elohim`). The `elohim` feature layer of `brit-epr` calls `elohim_epr::cid::compute_cid` / `cbor::encode` and uses the EPR covenant vocabulary (`Epr`, `Envelope`, `Coupling`, `Reach`, `EprKind`, `proof`, `validation`) directly. The **generic** `brit-epr::engine` stays codec-agnostic by depending only on the standard multiformats crates (`cid`, `multihash-codetable`, `serde_ipld_dagcbor`, `ipld-core`) pinned to the protocol's exact versions — so a downstream fork that disables the `elohim-protocol` feature still has a working, real-CID git tool.
- **Conformance — the "rebuild in machine code" guarantee:** brit vendors `elohim-epr`'s golden vectors (`tests/cid_vectors.rs`, `tests/canonical_bytes.rs`) as **test data** (not code). Any implementation — brit's, the monorepo's, a future C/Zig/machine-code port — is correct iff it reproduces these vectors byte-for-byte. The vectors are the portable spec; `elohim-epr` is the reference implementation.
- **Setup cost:** brit's build environment must resolve the `elohim` registry — a `[registries.elohim]` entry in brit's `.cargo/config.toml` plus credentials in the devcontainer/CI (the cross-format Nexus token already provisioned for `ethosenginebot`). This is a tracked task in the plan, not an open question.

**Migration note:** the existing `BritCid` (blake3-hex) is replaced, not migrated through — it has no codec/version discriminator to migrate *within*. BLAKE3 is retained **only** for non-address fingerprints (dedup/index keys), per the design-gate's discriminator ("does anything resolve it?").

## 4. Entities (P2P design-gate output)

Gate applied in full; brit's substrate is git-as-CAS, so the gate's *principles* (content-addressing, source-of-truth declaration, projection-not-truth, canonical CID forms, identity-ontology guard) apply to brit's ContentNode model.

### Entity: `EprMeta`
- **Classification:** Notarized-analog (A). The protocol would be *lying* if a node's declared governance/seed silently changed. Source of truth = git object store; the `EprMeta` is a projection.
- **Address:** Content-Derived **CID** (`bafyrei…`, dag-cbor). No UPDATE semantics — new content = new CID = new version, chained through git history (the version DAG).
- **What it is:** the canonical, CID-addressed successor to a directory-local `.epr-meta` — declares the subtree's governance, schema bindings, and the content it seeds. Identity-pure and *unnotarized* on its own.

### Entity: `NodeSeed`
- **Classification:** Notarized-analog (A). The node-root rollup / lockfile.
- **Address:** Content-Derived **CID**.
- **What it is:** the **import/export contract** — the WIT-`world`-shaped boundary declaration. Composes every `EprMeta` CID + the schema-version CIDs the node speaks + every content/blob CID + (when bridged) the git-oid map. Append-only across versions → the git-like change tail today's flat `ContentManifest` lacks. This is the lockfile that pins "the whole node" reproducibly.

### Entity: `NotarizationAttestation`
- **Classification:** Notarized (A) — the **runtime-supplied blessing**, exactly brit's existing `build/deploy/validation` attestation pattern extended.
- **Address:** Content-Derived **CID**; ed25519-signed by `AgentKey`; indexed as a git-notes ref. **Stored outside the seed bytes** (content purity). In-protocol this becomes DHT `validate(op)` witness convergence; the SLSA-predicate *shape* (`subject` digest + `resolvedDependencies` + `builder.id`) informs the **seed-lock provenance** field set, but the live validation edge records validity, not build-provenance — do not conflate them.

### Entity: `UpgradeProposal` — **deferred** (re-enters the gate when designed)
The v1↔v2 consensus-to-upgrade primitive. Maps onto brit's `MergeProposalContentNode` async-consent lifecycle + the schema-version DAG. Out of scope for this slice (see §11).

**Identity-ontology guard:** clean. brit uses *stewardship* framing throughout; the `reach` enum (`private…commons`) has no sovereignty apex. This design introduces none.

## 5. The git bridge (the internal bridge)

git is modeled as an **external format reached through a bridge**, mirroring the monorepo's `bridges/` pattern turned inward.

- **Dual addressing.** Each bridged object carries both its native **git oid** (sha-1/sha-256 over git's object encoding, via `gix`) and its **EPR CID** (dag-cbor of the EPR projection, via `elohim-epr`). The git oid is the *bridged/legacy* address; the EPR CID is *canonical*.
- **Structural mapping** (same DAG shape, different canonical encoding + hash): `git blob ↔ BlobContentNode` (raw codec `0x55`), `git tree ↔ TreeContentNode`, `git commit ↔ CommitContentNode`. The bridge is bidirectional: **export** (gix objects → EPR ContentNodes) and **import** (EPR ContentNodes → gix objects), holding the oid↔CID mapping table (itself content-addressed).
- **Home:** a new `brit-bridge` module/crate. It depends on `gix` (git side) and `brit-epr`/`elohim-epr` (canonical side). It is the seam every future canonical-EPR-native verb will route git compatibility through — the concrete enabler of the command-by-command migration.

## 6. The import/export world-contract & node-seed lockfile

- **`NodeSeed` is the WIT-`world`-shaped contract:** it declares the node's *complete* inbound/outbound surface — vocabulary + wire shapes + schema-version CIDs — not just per-type drift detection. "Can this node consume that seed?" becomes a compiler-grade verdict.
- **Export = a CAR-like archive carried by a git packfile.** brit *is* git, so packfiles are the native content-addressed archive; **restore-by-CID-set** falls out for free (verify a restored node against its declared root digest). No bespoke tarball.
- **Import gate = Lexicon-style fail-open / open-union**, *not* uniformly-strict. A boundary between independently-upgrading peers must tolerate a newer peer's additive fields (forward-compat); reject only on genuine wire-incompatibility. Reuse the schema-contract validation engine for the strict-where-it-matters checks.
- **Determinism is load-bearing** (same property the build-contract spec requires): the canonical-bytes function + a *pinned* DAG-traversal order make the export byte-reproducible. CARv1 itself does not specify deterministic creation — brit fixes the traversal order explicitly.

## 7. Two-layer notarization

- **Layer 1 — static identity.** `EprMeta`, `NodeSeed`, every content/blob node: identity = CID of canonical bytes. Offline-constructible, byte-reproducible, authority-free. A seed exported here is real, verifiable, and *unnotarized* — the architect's "private-reach authored intent."
- **Layer 2 — runtime blessing.** A separate ed25519-signed `NotarizationAttestation` edge, outside the content bytes, indexed as a git-notes ref locally. In-protocol it *is* Holochain neighbor-validation: the DHT runs the integrity `validate(op)` wasm and converges — the DHT is the tamper-evident log (Rekor-equivalent). The discipline that joins the layers: **schema-tagged, attenuation-only edges** (each attestation records its writer-schema and may only restate-or-attenuate authority) → the notarization graph is locally checkable and monotone.

## 8. CLI surface

A `epr-meta` verb group, homed first in **`brit-build-ref`** (it already owns the `AgentKey` + `LocalObjectStore` + `ContentNode` + `BritRefManager` + CID stack), with a follow-on wiring into the main `brit` binary as `brit epr-meta …` for UX.

| Verb | Purpose |
|---|---|
| `brit epr-meta seal <dir>` | Mint the `EprMeta` for a subtree → canonical CID |
| `brit epr-meta lock <repo>` | Compose the node-level `NodeSeed` lockfile (schema + content + bridge CIDs) |
| `brit epr-meta verify <cid>` | Check a tree/seed against its declared CID (byte-reproducible) |
| `brit epr-meta notarize <cid>` | Sign + publish the `NotarizationAttestation` edge |
| `brit epr-meta export <repo>` | Emit the CAR-like packfile archive named by the `NodeSeed` root CID |
| `brit epr-meta import <archive>` | Reconstitute into git objects (fail-open gate) |
| `brit epr-meta recompose <parent-root>` | Re-compose the parent's emergent `.epr-meta` corpus + cites + MEMORY into canonical artifacts (§9.1) |

Conventions honored (per brit's existing idioms): implement `ContentNode` for canonical-JSON→CID + object store for free; index via `BritRefManager` under a fresh `refs/notes/brit/meta/…` prefix; JSON to stdout; clap-derive; `--repo` + `canonicalize()`.

## 9. Discipline supersession (the dogfood loop)

This work is meant to **supersede** the parent's memory/discipline machinery, not replicate it inward. We master the next-generation discipline here and bring it back to the whole. Concretely:

- This spec and its plan are authored as the **first canonical `EprMeta` artifacts** — once `epr-meta seal` exists, `docs/` is sealed and the docs become content-addressed nodes; cites become canonical content-addressed links (the successor to the rglob-slug cite).
- brit gets a **lightweight discipline scaffold now**: `.claude/memory/MEMORY.md` (index) + a brit `CLAUDE.md` framing brit-dev as the place the next-gen discipline is mastered. The Python enforcement toolchain (`epr_meta.py`, `cite_graph.py`, `placement-audit`) is **not** ported — it is the thing being superseded.

### 9.1 Supersession is by re-composition (the recomposition bridge)

Superseding the parent's discipline is only safe if **everything it currently holds can be migrated/re-composed into the canonical form by a script** — nothing is lost in the cutover, and the new form is provably a superset. So the design includes a **recomposition bridge**, structurally the same move as the git bridge (§5) but with the parent's *emergent `.epr-meta` corpus* as the external format being absorbed:

- A `brit epr-meta recompose <parent-root>` tool ingests the parent monorepo's existing discipline artifacts — the `.epr-meta` YAML manifests (governance/cascade), the `cites:` link graph, and the `MEMORY.md` index — and emits canonical `EprMeta` / `NodeSeed` artifacts with real CIDs.
- This makes the clean-sheet schema a **re-composition target**: if the canonical form cannot represent a parent construct, that is a schema gap to close, not a reason to keep the old form. **The recompose run is the acceptance test for "the clean sheet is complete."**
- The migration is therefore **one-shot and regenerable**, not a hand-port — and it doubles as the proof that the supersession loses nothing.

## 10. Crate / module architecture

| Layer | Crate / module | Change |
|---|---|---|
| Generic engine | `brit-epr::engine` | Replace `BritCid` (blake3-hex) with real CIDv1 via multiformats crates. Stays codec-agnostic. **The one invasive change.** |
| EPR vocabulary | `brit-epr::elohim` (feature `elohim-protocol`) | Depend on `elohim-epr = "0.1"`; add `EprMeta` + `NodeSeed` ContentNodes + their schema. |
| Git bridge | `brit-bridge` (new) | gix ↔ canonical-EPR translation; dual oid↔CID mapping. |
| Attestation | `brit-epr::elohim::attestation` | Add `NotarizationAttestation` alongside build/deploy/validation. |
| CLI | `brit-build-ref` (+ later `brit`) | The `epr-meta` verb group. |

The generic-engine boundary from the master spec is preserved: disabling `elohim-protocol` drops `elohim-epr` and leaves a working real-CID git tool.

## 11. Decomposition (TDD phases)

The implementation plan (`docs/plans/2026-06-29-canonical-epr-meta.md`) will decompose into:

1. **Registry + CID engine.** Wire the `elohim` registry into brit; replace `BritCid` with real CIDv1; vendor the golden vectors; prove byte-parity with `elohim-epr`.
2. **`EprMeta` + `NodeSeed` ContentNodes** + schema + validation.
3. **`brit-bridge`** — git↔EPR dual-address round-trip (export/import of blob/tree/commit).
4. **`NotarizationAttestation`** edge + seed-lock provenance fields.
5. **Export/import world-contract** — packfile archive + fail-open import gate + the lockfile.
6. **`brit epr-meta` CLI** verb group (homed in `brit-build-ref`).
7. **Discipline scaffold** + dogfooded docs (`seal` the design + plan).
8. **Recomposition bridge** — `recompose` the parent's `.epr-meta` corpus + cites + MEMORY into canonical `EprMeta`/`NodeSeed` artifacts; the run is the completeness test for the clean-sheet schema (§9.1).

## 12. Scope, non-goals, deferred

- **Deferred (architect's call):** the v1↔v2 **consensus-to-upgrade** primitive (`UpgradeProposal` + quorum + lineage) — its own spec, beside this one.
- **Deferred:** the **whole-corpus CID re-mint** — "not until we've got a workable model." This design produces the model; the re-mint is a downstream operation it enables.
- **Non-goal:** porting the parent's Python discipline toolchain (superseded, §9).
- **Non-goal:** git-parity maintenance with upstream gitoxide.

## 13. Resolved decisions & open questions

**Resolved (2026-06-29 review):**
- **CLI home** → `brit-build-ref` now (wired into the `brit` binary later). No dedicated `brit-meta` binary yet.
- **`EprMeta` schema shape** → **clean-sheet**, carrying forward only predicates that earned their keep — *with the hard requirement* that the clean-sheet form be **provably re-composable** from the parent's existing emergent corpus via the recomposition bridge (§9.1). Supersession is by re-composition, never by abandonment.

**Open:**
1. **Bridge persistence.** Where does the oid↔CID map live — git notes, a `LocalObjectStore` index, or both?
2. **Command-by-command migration ordering** — which brit verb goes canonical-EPR-native first?
3. **CI registry credential** plumbing for brit's devcontainer (mirrors the `ethosenginebot` Nexus setup).

## 14. Cross-references

- Master design: `docs/specs/2026-04-12-brit-design.md`
- Push-time sibling: `docs/specs/2026-04-27-build-contract-before-push-design.md`
- Canonical codec (consumed): `elohim-epr` 0.1.0 — monorepo `elohim/epr/` (`cid.rs`, `cbor.rs`, `tests/cid_vectors.rs`)
- Parent emergent form (superseded): monorepo `genesis/docs/superpowers/specs/2026-06-25-epr-meta-compose-gate-design.md`
- Raw-blob CID recipe: monorepo `elohim/elohim-storage/src/blob_store.rs` (`RAW_CODEC = 0x55`)
- Research synthesis grounding this design: the universal-interoperability prior-art pass (IPLD/CAR · Cambria lenses · WIT worlds · Avro/Buf · UCAN · Unison · AT-Proto Lexicon · SHACL).

## 15. Done criteria

1. `brit epr-meta seal <dir>` mints a CIDv1 (`bafyrei…`) byte-identical to what `elohim-epr` would produce for the same canonical bytes.
2. The vendored golden vectors pass in brit.
3. `brit epr-meta lock` produces a `NodeSeed` pinning schema + content + bridge CIDs, reproducibly (same input → byte-identical lock).
4. `export` → `import` round-trips the elohim monorepo's git objects with dual addressing and a fail-open gate (the Phase-2 "import the monorepo, validate round-trip" sprint).
5. `notarize` produces a verifiable ed25519 attestation edge outside the seed bytes.
6. The generic engine still builds and passes with `elohim-protocol` disabled.
7. `brit epr-meta recompose` regenerates canonical `EprMeta`/`NodeSeed` artifacts from the parent monorepo's existing `.epr-meta` corpus + cites + MEMORY with no construct lost (the clean-sheet completeness test).
