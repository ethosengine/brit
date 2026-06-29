---
id: epr-meta-composition-snapshot
cites:
  - canonical-epr-meta-git-bridge | the master design this slice refines | sha256:c1283ad6e6bda687
---

# EPR-Meta as Composition Snapshot — Canonical Cites & the Parity-Supersession Slice

**Date:** 2026-06-29
**Status:** Draft
**Author:** Matthew Dowell + Claude Opus 4.8
**Extends:** `docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md` (master) — refines §4 (entities), §6 (world contract) and §9 (discipline supersession) into the first *usable* discipline slice.
**Companion:** `docs/specs/2026-04-27-build-contract-before-push-design.md` (the push-time sibling of this seed-time contract).
**Consumes:** `elohim-epr` 0.1 (the CID engine, already wired) · the parent monorepo's cite engine (`/projects/elohim/.claude/scripts/_lib/cite_graph.py`) **as the parity oracle** — imported and called directly, never ported.
**Adversarially reviewed:** 2026-06-29 (5-lens red-team → SHIP-WITH-CHANGES; the five blockers below are resolved in this text).

---

## TL;DR

brit is mastering the **Snapshot**: how to *seed and bootstrap the composition + relationship graph* of the protocol as a static, content-addressed artifact. The next-generation `epr-meta` is that snapshot — a recursive, WIT-`world`-shaped import/export contract grounded at (nearly) the filesystem level, where **cites are imports, a doc's `id` is an export, an `EprMeta` is a subtree world, and a `NodeSeed` is a node world**, composing up to the protocol as the top-level world.

The strategy is **parity-supersession**: run the parent monorepo's cite/discipline tooling *on brit's own corpus* as a **golden reference**, build the brit-native canonical tooling to reproduce its verdicts, prove parity, then deprecate the parent toolchain for brit — the discipline-level analog of the CID golden-vector conformance brit already passes.

The first slice is **canonical cites + a parity harness**, dogfooded on brit's own `docs/` + `.claude/memory/`. It is deliberately *Layer-1 only*: a static snapshot with no enforcement. Governance, power-graduation, head-election and the human-scale constitutional model live in **Layer-2 (the Holochain DHT runtime), which brit does not have yet** and which this slice does not build.

---

## 1. Scope — the Snapshot (Layer-1), not the runtime (Layer-2)

brit's job *right now* is the **Snapshot**: seed/bootstrap the relationship graph as a frozen, content-addressed artifact. It is explicitly **not** the governance runtime.

**The constitutional model the snapshot must be able to represent (but does NOT enforce).** In the Elohim Protocol the composition hierarchy is **not** a control hierarchy:

- Every bottom edge is **human-scale governable on the Holochain DHT**.
- A child node keeps **full agency within constitutional limits** — bounded below by the **deterministic floor** (a guaranteed minimum that cannot be taken away) and above by the **elohim ceiling** (the constitutional maximum no power may exceed).
- Parents hold **no absolute control** over children. Power **graduates** through successive layers — parent-managed and collective-managed EPRs — up to elohim at the ceiling, and even the ceiling is constitutionally bound (the imago-dei backstop; the law binds even the lawgiver).
- It is **subsidiarity with a protected floor**, never top-down override.
- **The ceiling has a human-scale trigger, not a fixed cap.** Graduation from individual to *collective/elohim-stewarded* resource begins where maintaining the resource becomes **frictionful for a human to steward** — at **Dunbar-scale** capacity limits. Stewardship of the epr-meta nodes *themselves* graduates with that friction, respecting the **variability of human capacity** (not a one-size threshold). This capacity-bounded graduation **is the anti-systemic-capture mechanism**: power cannot concentrate beyond what a human can actually steward — past that point it *must* become collectively stewarded, so no single agent captures the system.

**Why the ceiling — the intellectual lineage (the deferred governance's north star).** The ceiling is *context-dependent*, not a universal cap — appropriate limits differ by society (Robeyns' *limitarianism*). The specific capture it prevents is **substrate-rent**: the Georgist observation that beyond a point, owning the *substrate* others depend on stops rewarding production and starts conferring rent-extraction power — and the epr-meta graph is exactly such a substrate. Graduation is how the **logarithmic/systemic influences** in the protocol's political economy land with a steward that can hold them in tension faithfully *because it is not bound to self-interest* ("the elohim"). The aspiration — *putting the fruit back on the tree of knowledge* — is to keep knowledge and power re-shareable rather than privately captured, treating the human-scale **hearth** as the right-shaped seed for how AI develops, propagates, and is used. This grounds the deferred §6 governance; the snapshot's only obligation is to *not foreclose* it (no fixed single steward, no baked-in capture surface).

**Consequence for the format:** the snapshot must be *capable of representing* child-agency-within-bounds and graduated/subsidiary power. It must **never encode parent-fiat** (a parent "exports a contract its children must obey" is a protocol violation, not a design shortcut). Enforcement, power-graduation, head-election, and the contract circuit-breaker ("tag the contract off if it misbehaves") are all **Layer-2 / DHT runtime concerns, deferred** (§4, §6).

**What this slice delivers:** a faithful static snapshot of bottom-edge relationships (cites) plus the proof (parity) that the snapshot's drift/health verdicts match the proven reference. That is the bootstrap the future runtime governs.

## 2. Strategy — parity-supersession (the oracle)

The parent monorepo's cite engine is the **golden reference**; brit-native tooling is "done" when it reproduces the reference verdicts on brit's corpus; then the parent toolchain is deprecated for brit (and, eventually, the brit-native form returns to the whole).

- **The oracle is `envelope_verdict`, reached by import — not by shelling a CLI.** `cite-gen --verify` emits only *migration debt*; the real four-state verdict logic is `envelope_verdict()` in `_lib/cite_graph.py`, and `cite-propagate.py`'s paths are pinned to the parent tree with no override. The harness therefore **imports `_lib.cite_graph` and calls `build_slug_index(<brit roots>)` + `envelope_verdict(...)` directly** (the library functions take roots as arguments; only the CLIs are path-pinned).
- **Parity is verdict-label equivalence, not byte-identical cite lines.** The parent's parser accepts a `bafy…` token *syntactically*, but `envelope_verdict` recomputes a `sha256:hex16` and string-compares — so a CID pin would read *stale-always* and a `bafkrei` pin would be misparsed as `desc` (*ok-always*). "Forward-compatible cite line" is **false at the verdict layer**; do not rely on it. The harness instead **dual-authors** (parent `sha256:hex16` cites + brit cites) and compares the **verdict label** (`ok`/`held`/`stale`/`dead`) per edge, under a documented slug↔target mapping.
- **Verdict equivalence is a probabilistic regression gate, stated honestly** — the parent truncates to 64 bits; a collision is astronomically unlikely and we do not engineer around it.

## 3. The format — the recursive composition envelope

Everything is a CID-addressed `ContentNode`; the same import/export-edge envelope holds at every level, so composition is recursion. **Decision A (locked): converge the full envelope now; populate only `kind: doc-cite` in this slice.** Functional kinds light up later with no format migration — which requires the encoding discipline in §7.

### 3.1 The edge — `InterfaceRef`
```
InterfaceRef {
  kind:  doc-cite | content | schema-version | capability | contract | legacy | external
  role:  import | export
  ref:   String            // stable identity: the target's `id`/slug (move-survivable)
  cid:   Option<BritCid>   // OPTIONAL: the addressable content version, when one exists
  drift: Option<String>    // "sha256:hex16" non-address fingerprint of the canonical body (doc-cite)
  desc:  Option<String>    // directional relationship hint (imports only)
}
```
- `cid` is **`Option`**: legacy path-cites and cross-repo/external targets have no resolvable brit CID (§3.4). Only `kind: doc-cite` (and later, resolvable kinds) carry a `cid`.
- **`drift` is a fingerprint, not a CID.** It addresses the *frontmatter-excluded canonical body*, which `seal` never stores as a blob — so it cannot be dressed as a `bafkrei…` CID (that would be "a fingerprint wearing a CID costume," violating brit's own *does-anything-resolve-it?* discriminator). It is a `sha256:hex16` fingerprint chosen to **byte-match the oracle's recipe** (§7.1); this is the one sanctioned non-address `sha256` use, distinct from BLAKE3 dedup fingerprints and from CIDs.

### 3.2 The nodes (extend what's built; keep content-purity)
- **blob** — file bytes, `compute_raw` (codec `0x55`); the leaf / content export. (Storing blobs needs a `LocalObjectStore::put_raw`, deferred — see §5/§6.)
- **`EprMeta`** (subtree world) — `{ epr_meta_version, subtree, entries[], imports[], exports[] }`. `entries` (existing `path`→blob-CID) stays = the subtree's content. `imports`/`exports` are its world face. **`EprMeta` is a content-pure *tree*: its CID is a pure function of the sealed subtree bytes — there is NO `parent` field** (see §4). Governance/contract edges arrive later as `kind: contract` (§6), not as a side field.
- **`NodeSeed`** (node world) — `{ epr_meta_version, repo, epr_metas[], sub_seeds[], imports[], exports[] }`. The rollup. `sub_seeds[]` (child `NodeSeed` CIDs — e.g. a submodule's seed) is what **closes the recursion** (world-of-worlds); frozen into the format now (Decision A), unpopulated this slice. A node's *external* import surface = imports not satisfied by its own exports.

### 3.3 Engine-boundary placement (generic, not feature-gated)
The composition envelope is the **generic substrate**, so `InterfaceRef`, the `doc-cite` kind, and `EprMeta`/`NodeSeed`-as-composition **live in `brit-epr::engine`** (multiformats only) — disabling feature `elohim-protocol` must still leave a working real-CID git tool *that can snapshot a relationship graph*. The **elohim covenant kinds** (`capability`, `contract` bound to `Mishpat::Commitment`, the `Reach`/`Coupling`/`EprKind` vocabulary) stay **behind the feature**. (Today `EprMeta` is re-exported only under `elohim-protocol`; this slice lifts the generic core into `engine` and leaves the covenant vocabulary gated.)

### 3.4 Legacy & cross-repo edges (parity-critical)
brit docs already cite parent-repo docs/code (grep-confirmed). The parent oracle resolves those through its monorepo-wide slug index; a brit-only index reads them `dead`. To keep parity meaningful:
- `kind: legacy` — a legacy path-string cite to an id-bearing target (the parent's CITE-FORMAT-CANDIDATE); verdict `ok` unconditionally, no `cid`.
- `kind: external` — a cross-repo target outside brit's snapshot; classified via a **pinned parent `NodeSeed`** or a documented allow-list, not read as `dead`. (This is also the seam the recompose/consume-check completes — §6.)

## 4. Layer-1 / Layer-2 — the honest principle

**Named principle: `epr-meta` is a Layer-1 *static* artifact; currency, election, signature, and governance are Layer-2 runtime.** Stated honestly (correcting an earlier overstatement):

- A frozen file has concrete state, so a seal is a frozen snapshot carrying content, structure, declared imports/exports, and (doc-cite) drift fingerprints.
- An **`export` is an unsigned head binding** and **resolving an import pin is a currency lookup** — these are *currency-adjacent* and they exist at Layer-1. What is deferred is the **signature/election** over them (IPNS/UCAN-style signed naming records, fork head-election, multiple-parent-head reconciliation, gossip) and **all enforcement**. ("NO forward pointer at all" was too strong.)
- **Lineage stays out of the content-pure tree.** A `parent`-CID on `EprMeta` would make `seal` impure (the same bytes would seal to different CIDs depending on prior state, regressing the foundation's byte-reproducibility) and a last-seal marker would become a hidden source of truth that doesn't survive a clone. So — git-faithfully — **`EprMeta` is the *tree* (pure); the lineage/version-DAG moves to a separate commit-like node that carries `parent` and is the head-able thing — deferred to the Layer-2 slice.** "EPRs are heads with a git-like shape" survives; it's git's own tree/commit split.

## 5. Slice 1 — canonical cites + parity harness (the decomposition)

Scope honesty: this is **~12–16 tasks**, not "extend `EprMeta` + add `status`." brit's corpus is currently **empty of the discipline** (zero `id:`, zero `cites:`, no `held/` tree — its docs use `Extends:`/`Consumes:` frontmatter). So slice 1 splits into three sub-slices:

**Sub-slice A — corpus + conformance foundation.**
- **Task 0: author the seed corpus.** Add `id:` + `cites:` frontmatter to brit's own `docs/**` + `.claude/memory/**` (this *is* the operator's "author seed data"), and construct a deliberate **verdict fixture** carrying one of each: `ok`, `stale`, `dead`, `held` (with a `held/` subtree). Parity is **fixture-driven first, organic dogfood second.**
- **Body-extraction conformance (the first parity test).** Spec and test the exact canonical-body recipe (§7.1) against a vendored vector set mirroring the CID golden vectors — *before* any fingerprint is trusted.

**Sub-slice B — the verdict engine (generic `engine`).**
- A frontmatter parser, a cite-line parser (pipe-delimited, into `InterfaceRef`), a brit slug-index builder over brit roots, and an `envelope_verdict`-equivalent: precedence **`dead` > `held` > `stale`** (`dead` = ref not in index; `held` = in index but path under `held/`; `stale` = recomputed drift ≠ pinned; else `ok`). Held docs must be index members.

**Sub-slice C — verbs, hook, harness.**
- `brit epr-meta seal <dir>` (extend): per doc compute the content CID (raw blob) and the drift fingerprint (canonical body); record `id`→export; parse `cites:`→import edges pinned to each target's current drift fingerprint; store the content-pure `EprMeta`.
- `brit epr-meta status [<doc>|<repo>]`: resolve each import's verdict by re-reading the target from current filesystem bytes ("current = filesystem" while there is a single static tree). **`status` recomputes the *drift* fingerprint (canonical body), never the entry/full-file CID** — else every frontmatter/cite stamp trips false-stale (guarded by a test).
- **A thin advisory post-hook** (the felt "wire into the post-hook" deliverable): a PostToolUse `Edit|Write` nudge that surfaces cite debt / drift, **advisory, never blocking** — the brit analog of the parent's `cite-seal-signal`. (The *governance/contract-enforcing* hook is deferred — §6.)
- **The parity harness** (lives in `xtask`/CI, not `just test` — it needs Python + the parent `_lib` + a monorepo checkout): import `_lib.cite_graph`, build the index over brit roots, collect `envelope_verdict` labels on the fixture; run `brit epr-meta status`; assert per-edge label equivalence.

## 6. Deferred composition map (each its own spec + plan)

- **Govern — constitutional contract.** `kind: contract` edges: a node declares the constitutional bounds it answers to (**deterministic floor ↔ elohim ceiling**), children self-govern within them, power graduates (subsidiarity, never parent-fiat). **The ceiling triggers on human-capacity friction (Dunbar-scale): when a resource outgrows what a human can steward, its stewardship must graduate from individual to collective/elohim forms — the anti-systemic-capture mechanism. The contract kind must therefore represent graduated, capacity-variable stewardship (individual ↔ collective, respecting that human capacity varies), never a fixed single steward.** Composes from the gospel-tier **`Mishpat::Commitment`** primitive (bounded reciprocity + standing + revocation + audit). **Enforced only at Layer-2 (the DHT runtime).** Re-enters the **p2p-design-gate** and the justice-as-capability / identity-sovereignty guards when designed.
- **Post-hook — enforcement.** The contract executor: a self-executing edge bound to its node, **executed at Layer-2** with a **circuit-breaker "tag-off"** (bounded/total execution → trip on recursion/fault → suspend → degrade to nudge-only/fail-open), the trip recorded as a signed suspension edge so accountability survives the fault. The slice-1 hook is the thin *advisory* precursor.
- **`NodeSeed`-as-world + consume-check + `lock`** — compose `EprMeta`s (+ `sub_seeds`) into the node world; implement "can A consume B?" (A.imports ⊆ B.exports, fail-open on additive). NB: a `(slug, cid)` doc-cite carries no type, so until typed functional kinds exist the consume-check **degenerates to referential integrity** — the world-linking *semantics* are deferred with the kinds.
- **Functional kinds** — `content` · `schema-version` · `capability` `InterfaceRef`s.
- **Layer-2 runtime head + DHT governance** — signed naming records, successor/head-election, gossip, and the human-scale governance model of §1; "current" stops being "the filesystem."
- **Git-bridge lift** — the original master-spec §5 bridge: the same CIDs through `gix` objects → the snapshot lifts to git and the P2P dataplane unchanged.
- **Recompose → deprecation** — ingest the parent's `.epr-meta` + cites + MEMORY into canonical artifacts (master-spec §9.1 completeness test); the run proving no construct is lost is what **triggers parent-toolchain deprecation** for brit.

## 7. Conformance & determinism discipline

### 7.1 Canonical-body extraction (a conformance burden, not a `.strip()`)
The drift fingerprint must hash **the same bytes the oracle hashes**. The oracle recipe (`cite_graph.fingerprint`): `sha256( body.strip().encode("utf-8","replace") ).hexdigest()[:16]`, where `body` is the **frontmatter-excluded** content body. Reproducing this in Rust is exact work, not approximate: the **frontmatter boundary** detection, line-ending normalization (Python `splitlines` → `\n`), the **whitespace set of Python `str.strip()` vs Rust `str::trim()`** (they differ on some Unicode), and `errors="replace"`. The recipe is pinned here and **a vendored body-extraction conformance vector set** (mirroring the CID golden vectors) is the **first** test in the slice; nothing downstream is trusted until it passes.

### 7.2 Encoding stability (Decision A's "no migration" rests on this)
The frozen envelope only stays migration-free if its DAG-CBOR encoding is stable: **no `skip_serializing_if`**, `#[serde(default)]` on every added field, and **stable empty-collection / `Option::None` encodings** — otherwise a later optional field silently re-CIDs every node (and `LocalObjectStore::get` is schema-rigid). Adding `imports`/`exports`/`sub_seeds` shifts `EprMeta`/`NodeSeed` *instance* CIDs (expected; sequence the schema change before any dogfood seal) but **does not** touch the CID-engine `0xa0` golden vector or the `elohim-epr` byte-parity conformance (red-team-confirmed safe).

### 7.3 Canonical ordering
`imports[]` / `exports[]` / `epr_metas[]` / `sub_seeds[]` are sorted for deterministic encoding by a pinned key `(role, kind, ref)` (cites are projected, not authored-order-bearing, in the seal). This requires **deriving `Ord` on `BritCid`** (today only `PartialEq`/`Eq`/`Hash`, so `.sort()` won't compile) — a mechanical foundation fix.

## 8. Done criteria

1. **Body-extraction conformance:** brit reproduces the oracle's canonical body byte-for-byte across the vendored vector set.
2. **Verdict parity:** on the fixture corpus, `brit epr-meta status` produces `ok`/`held`/`stale`/`dead` labels equal to `_lib.cite_graph.envelope_verdict` for every edge, precedence `dead`>`held`>`stale`.
3. **Content-purity preserved:** sealing identical subtree bytes yields an identical `EprMeta` CID (no `parent`, no hidden marker); the `elohim-epr` byte-parity + `0xa0` golden vectors still pass.
4. **Two-track identity correct:** a doc carries a resolvable content CID (raw blob) and a non-address drift fingerprint; `status` recomputes the drift fingerprint (not the entry CID) — a frontmatter/cite stamp does not trip `stale`.
5. **Generic substrate:** the snapshot verbs build and pass with feature `elohim-protocol` **off**.
6. **Felt deliverable:** the advisory post-hook surfaces cite debt/drift on an edit to a brit doc, non-blocking.
7. **Cross-repo honesty:** brit→parent cites classify as `external` (not `dead`) and the divergence from the monorepo-wide oracle is documented, not silently failing parity.

## 9. Open questions

1. **External classification source** — pinned parent `NodeSeed` vs documented allow-list for cross-repo cite targets (§3.4); the former is the recompose-grade answer.
2. **Imports order** — sort by `(role, kind, ref)` (chosen, for determinism) vs preserve authored cite order; revisit if authored order turns out to carry meaning.
3. **Fixture location** — in-repo `tests/fixtures/` vs a generated temp tree; the `held/` subtree must be a real path for the `held` verdict.

## 10. Cross-references

- Master design: `docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md` (§4 entities, §5 git bridge, §6 world contract, §7 two-layer notarization, §9 supersession).
- Foundation plan (built): `docs/plans/2026-06-29-canonical-epr-meta-foundation.md` (CID engine + `EprMeta`/`NodeSeed` + `seal`/`verify`).
- Parity oracle (consumed, not ported): `/projects/elohim/.claude/scripts/_lib/cite_graph.py` (`fingerprint` ~L82, `build_slug_index` ~L87, `envelope_verdict` ~L234); `/projects/elohim/.claude/hooks/cite-seal-signal.py` (advisory-hook prior art).
- Constitutional frame: the Elohim deterministic-floor / elohim-ceiling model and `Mishpat::Commitment` (REA compute-commitment primitive); justice-as-restored-capability and identity-sovereignty-subordination guards.
- Research grounding (master-spec §14): IPLD/CAR · Cambria lenses · WIT worlds · UCAN · Unison · AT-Proto Lexicon · SHACL.
