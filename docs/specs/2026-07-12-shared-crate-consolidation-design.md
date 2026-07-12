---
id: shared-crate-consolidation
cites:
  - epr-meta-composition-snapshot | the snapshot slice whose envelope this consolidates onto shared derivation | sha256:ba304e4bfc6d337d
  - canonical-epr-meta-git-bridge | the master design whose CID engine + EprMeta this shares outward | sha256:c1283ad6e6bda687
---

# Shared-Crate Consolidation — one addressing/verdict/lineage contract, defined once

**Date:** 2026-07-12
**Status:** Draft
**Author:** Matthew Dowell + Claude Opus 4.8
**Extends:** `docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md` (the parity-supersession slice) into its **structural end-state.**
**Integrates the 2026-07-12 lesson set:** the parent monorepo specs
`genesis/docs/superpowers/specs/2026-07-12-cite-fingerprint-cid-convergence-design.md`
(one digest, two renderings; codec rule 0x55 raw / 0x71 dag-cbor) and
`genesis/docs/superpowers/specs/2026-07-12-epr-meta-kinship-lineage-reconciliation-design.md`
(lineage edges, export envelope with `exportedFrom` + boundary, the `remote` verdict,
chunk-genome similarity, authority-anchored head declaration).
**Consumes:** `eprfs-core` (`BlobCid`, published to Nexus from `dev` as of parent `4c033062e`) · `elohim-epr` 0.1 (the codec canon, already an optional dep).

---

## 0. The mandate this serves

> **Prefer shared crates over parallel reimplementations. Define each interface
> once so that changing it produces a compile-error cascade that enumerates every
> integration point. Parity fixtures were the bootstrap; structural sharing is the
> end state.** — operator, 2026-07-12

Parity-supersession (the composition-snapshot spec §2) proved brit's addressing and
verdict logic *reproduce* the parent oracle's — by **golden vectors and dual-authored
fixtures.** A fixture proves equality at one instant; it does not *prevent* the two
implementations from drifting apart on the next edit. This spec converts the proven
equalities into **structural single-sourcing**: the shared bytes-level contract moves
to one crate, and the duplicates become thin, compile-checked delegations.

The discipline is inverted from the bootstrap: *there, brit re-derived to prove it
could; here, brit consumes so it cannot diverge.*

---

## 1. Duplication audit — every primitive defined in ≥2 places

Locations are `crate::path` (brit paths relative to `elohim/brit/`, eprfs relative to
`elohim/eprfs/`, parent relative to repo root).

| # | Primitive | Definitions (≥2) | Current equality guarantee | Consolidation target |
|---|-----------|------------------|----------------------------|----------------------|
| 1 | **Digest + codec math** (`Sha2_256.digest`; codec `0x55` raw / `0x71` dag-cbor; `Cid::new_v1`) | `brit-epr/src/engine/cid.rs::BritCid::{compute,compute_raw}` · `eprfs-core/src/address.rs::BlobCid::{compute,compute_raw}` · `elohim-epr/src/cid.rs::compute_cid` (codec canon) | `eprfs-core` test `raw_codec_matches_brit_vector` pins the `bafkrei…` "hello world" vector byte-identical to brit; brit `empty_cbor_map_is_bafyrei` + `raw_blob_is_bafkrei` pin the same format against `elohim-epr` golden vectors | **`eprfs-core`** exposes the derivation; `BritCid` computation delegates. Codec constants defined once. |
| 2 | **Cite short-form fingerprint** (`"sha256:" + hex(sha2-256 digest)[:16]`) | `brit-epr/src/engine/frontmatter.rs::drift_fingerprint` · `brit-epr/.../cid.rs::BritCid::short_fingerprint` · `eprfs-core/.../address.rs::BlobCid::short_fingerprint` · parent `_lib/cite_graph.py::fingerprint_text` | brit `cid_fingerprint_derivation` (drift == body-CID short-form); eprfs `short_fingerprint_equals_python_cite_fingerprint` (== `sha256:4e72cded3d6affd3`); Python is the cross-language oracle | **`eprfs-core`** owns `short_fingerprint(digest)`; `drift_fingerprint` = `BlobCid::compute_raw(canonical_body).short_fingerprint()`. Python stays the *decode-only* oracle (single-encoder invariant: only `eprfs cid` CLI encodes). |
| 3 | **Canonical-body extraction** (frontmatter split, trim, `errors="replace"`) | `brit-epr/.../frontmatter.rs::{split_frontmatter,canonical_body}` · `eprfs-agent/.../canonical.rs` (agent-specific fence split) · parent `_lib/cite_graph.py::body_of` | brit body-extraction conformance vectors (composition-snapshot spec §7.1) vs the Python recipe | **Stays domain-local** initially (brit's markdown-doc recipe ≠ eprfs-agent's frontmatter recipe). A shared `canonical_body` helper in `eprfs-core` is a *stage-3* option once the recipes are proven identical; not first-stage. |
| 4 | **Verdict enum** (`ok`/`held`/`stale`/`dead` + **`remote`**) | `brit-epr/src/engine/verdict.rs::Verdict` · parent `_lib/cite_graph.py::envelope_verdict` return vocabulary | precedence `dead>held>stale`; `remote` added today on both sides (brit `f6544964c0`, Python) | **New tiny shared vocab crate** (`eprfs-verdict`, see §3) OR an `eprfs-core` module — the *label set* is single-sourced; the *evaluation* (filesystem-aware in brit, index-aware in Python) stays per-runtime. |
| 5 | **Fingerprint-slot recognition** (`sha256:` \| full-CID token) | `brit-epr/src/engine/interface_ref.rs::is_fingerprint` (**`bafy` only — divergent**) · `brit-epr/.../verdict.rs` (`baf`) · parent `_lib/cite_graph.py::_is_fingerprint` (`baf`) | **NONE — this is a live divergence, not an equality** (see §2) | Single predicate in the shared vocab; **first mechanical fix lands the `baf` correction now** (no new deps). |
| 6 | **Cite-line parsing** (pipe split, desc/fp/status/path slots) | `brit-epr/src/engine/interface_ref.rs::parse_cite_line` · parent `_lib/cite_graph.py::parse_cite` | fixture-level dual-authoring (composition-snapshot spec §2); no structural pin | **Stays brit-local** (parser feeds brit's `InterfaceRef`); shares only the fingerprint predicate (#5) and, later, the verdict vocab (#4). |
| 7 | **CID newtype + serde wire** | `brit-epr/.../cid.rs::BritCid` (`#[serde(transparent)]` → **tag-42 IPLD link** in dag-cbor) · `eprfs-core/.../address.rs::BlobCid` (`#[serde(into="String")]` → **cbor text string**) | none — **deliberately different wire forms** (§4) | **NOT unified naively.** The newtypes stay per-domain; only the *derivation* (#1, #2) is shared. Full type-alias is a gated stage-4 requiring a tag-42 serde on `BlobCid` + byte-diff golden verification. |

**The load-bearing discovery is row 7.** `BritCid` and `BlobCid` are not two spellings of
one type — they carry **different serde contracts on purpose**, and the difference lives
exactly in the canonical bytes that define identity. Consolidating the *type* (the task's
first-cut "BritCid → re-export of BlobCid") would silently change every `EprMeta`'s
dag-cbor encoding and re-CID the whole snapshot. Consolidating the *derivation* (rows 1–2)
achieves the mandate's compile-cascade without touching wire bytes. This spec chooses the
derivation path and defends it in §4.

---

## 2. The live divergence (row 5) — fix now, no deps required

`interface_ref.rs::is_fingerprint` still reads:

```rust
s.starts_with("sha256:") || s.starts_with("bafy")
```

Both the parent oracle (`_is_fingerprint` → `("sha256:", "baf")`) and brit's own
`verdict.rs` (`d.starts_with("baf")`) were broadened to **`baf`** so that a **raw-codec
body CID** (`bafkrei…`, codec `0x55`) is recognized in the fingerprint slot. brit's
*parser* was not. The consequence is a real, silent gap in the `remote`-verdict path
that landed today:

- A cite line pins a raw-codec full CID: `slug | desc | bafkrei…`.
- `parse_cite_line` calls `is_fingerprint("bafkrei…")` → **false** (`bafk` ≠ `bafy`).
- The token falls through to the `desc` slot; `drift` stays `None`.
- `verdict()` sees `drift: None` on an absent slug → **`Dead`**, never `Remote`.

So the raw-codec rendering of the convergence — the one the cite-fingerprint spec makes
the *default* body address — cannot produce a `Remote` verdict through brit's parser. The
fix is a one-token broadening that makes three surfaces agree (parser, verdict, oracle):

```rust
s.starts_with("sha256:") || s.starts_with("baf")
```

This needs **no new dependency**, is `cargo fmt --check`-clean, and is pinned by a new
unit test (`parses_raw_codec_full_cid_into_drift`). It ships as the mechanical first step
(§6). It is *also* the smallest possible instance of the mandate: the predicate wants to
be defined once (§3, row 5) precisely so this class of drift cannot recur.

---

## 3. Ownership decisions (decided + defended)

### 3.1 Addressing → `eprfs-core` (not `elohim-epr`, not a new crate)

**Decision: `eprfs-core::BlobCid` is the addressing owner; `brit-epr` gains an (ungated)
`eprfs-core` dependency and delegates its digest/codec/short-form derivation to it.**

Defence:

- **Not `elohim-epr`.** `elohim-epr` is the *codec canon* (the byte-parity source), but it
  is a **feature-gated, heavy** dep in brit (`elohim-protocol = ["dep:elohim-epr"]`). The
  engine boundary is load-bearing: *disabling `elohim-protocol` must leave a working
  real-CID git tool* (brit CLAUDE.md; composition-snapshot spec §3.3). Addressing therefore
  **cannot** sit behind that feature. `elohim-epr` stays what it is — the conformance
  oracle brit's shared vectors are pinned against — but it does not own the generic engine's
  CID.
- **`eprfs-core` fits exactly.** It is already lean (over `cid` + `multihash-codetable`, the
  same base crates `BritCid` uses — *not* the heavy atom codec), already runtime-agnostic
  (eprfs CLAUDE.md: "core crate must stay small, pure, runtime-agnostic"), already published
  to Nexus from `dev`, and already **byte-parity-pinned to brit's own vectors**
  (`raw_codec_matches_brit_vector`). It can be an **ungated** dep, so the generic engine
  keeps building with `elohim-protocol` off. This mirrors the existing `elohim-epr`
  Nexus-registry precedent the task names.
- **No dependency cycle.** `eprfs-core` depends only on `cid`/`multihash`/`serde`; it never
  depends on brit. The eprfs boundary rule ("do not embed git semantics in `eprfs-core`") is
  respected — brit *consumes* the addressing primitive, it does not push git meaning into
  eprfs. Direction `brit-epr → eprfs-core` is acyclic and boundary-clean.
- **Not a fourth crate.** Minting `eprfs-address` would add a hop for zero gain — `eprfs-core`
  is already the addressing home for the eprfs layer.

### 3.2 Verdict / envelope vocabulary → a small shared vocab in the eprfs workspace

**Decision: the *verdict label set* (`ok`/`held`/`stale`/`remote`/`dead`) and the
*fingerprint-slot predicate* become a small shared module — `eprfs-core::verdict` (a leaf
module, no new crate unless a non-eprfs consumer appears). The *evaluation* stays
per-runtime.**

Defence: the **vocabulary** is what must not drift (the `remote` addition today had to be
made in two places by hand — exactly the failure the mandate targets). But the **evaluator**
is legitimately different per runtime: brit's `verdict()` reads the live filesystem; the
Python oracle reads a slug-index; a future Layer-2 evaluator reads DHT heads. Sharing the
label enum + the `is_fingerprint`/`cid_digest_matches` predicates single-sources the
contract while leaving each runtime its own resolution. Python cannot import a Rust enum, so
Python remains the **decode-only oracle** pinned by cross-language corpus tests (the
single-encoder invariant: only `eprfs cid` CLI encodes a CID; the convergence spec §"Python
never encodes CIDs").

### 3.3 What stays brit-local (git-domain semantics)

- **`BritCid` the newtype** and its **tag-42 IPLD-link serde** — this is git-object-store
  meaning (a `MetaEntry.cid` is a real dag-cbor link into the object store), not a generic
  addressing concern (§4).
- **Cite-line parsing → `InterfaceRef`**, frontmatter splitting, the `EprMeta`/`NodeSeed`
  composition envelope, `EprGraph`, `seal`/`status`/`verify` verbs, the advisory post-hook.
- **The parity harness** (imports the Python oracle) — brit's conformance surface.

---

## 4. Why the CID *type* is not unified (the tag-42 hazard)

`BritCid` is `#[serde(transparent)] struct BritCid(Cid)`. Under `serde_ipld_dagcbor`, a
`cid::Cid` serializes through its magic serde marker to a **tag-42 IPLD link** — a native
CBOR CID link. `MetaEntry.cid` documents exactly this: *"a real IPLD CID link (tag-42 in
dag-cbor), never a string FK, so it cannot dangle across versions."*

`BlobCid` is `#[serde(into="String", try_from="String")] struct BlobCid(Cid)`. It always
serializes as the base32 **CBOR text string**, in every format including dag-cbor.

| Format | `BritCid` bytes | `BlobCid` bytes | Equal? |
|--------|-----------------|-----------------|--------|
| JSON | `"bafk…"` (Cid human-readable path) | `"bafk…"` | yes |
| **dag-cbor (canonical identity!)** | **tag-42 link** | **text string** | **NO** |

The canonical identity of an `EprMeta` *is* its dag-cbor bytes. Replacing `BritCid` with a
re-export of `BlobCid` would rewrite every embedded CID from a tag-42 link to a text string,
**moving every `EprMeta`/`NodeSeed`/`InterfaceRef` instance CID** and breaking the
"real IPLD link" contract. This is the canonical example of the risk the task flags: *a
shared-crate cascade that breaks brit's CI in a way local tooling cannot predict* — here it
would not even fail to compile; it would silently re-CID the snapshot, and only a golden
dag-cbor byte-diff would catch it.

**Therefore:** consolidate the **derivation** (the sha256+codec+truncation math and the
golden vectors — rows 1–2, which have no serde surface), keep the **newtypes** per-domain.
The compile-cascade mandate is satisfied at the derivation layer: change the shared
`compute_raw`/`short_fingerprint` and every consumer recompiles against it. Full type
unification is a *possible* stage-4 (§5) and requires either (a) a tag-42 serde mode on
`BlobCid` (an added `BlobLink` variant) or (b) brit adopting string-wire CIDs (a deliberate
change to `MetaEntry`'s link contract) — each gated behind a byte-identical dag-cbor golden
diff. Neither is first-stage; neither is free.

---

## 5. The versioning-primitive integrations (from the kinship/lineage spec)

The 2026-07-12 kinship-lineage spec names four mechanisms. Their brit homes:

### 5.a Lineage edges + ancestry-set kinship → `brit-graph`

`brit-graph` already *is* the content-addressed DAG (`EprGraph<N,E>`: `DiGraph` keyed by
`BritCid`, cycle-checked). Fork/revert/merge is its home turf. The kinship spec's
**`parents: [cid]` / `derivedFrom`** fields belong **inside the canonical hashed bytes** of
the versioned node (tamper-evident by construction — you cannot restate parentage without
changing your own CID). The eprfs layer already shipped the shape to copy:
`eprfs-agent::CanonicalAgent` carries `parents: Vec<BlobCid>` + `derived_from: Option<BlobCid>`
as **append-only, support-only** fields (absent-lineage bytes are byte-identical to the
pre-lineage output — its golden test `absent_lineage_canonical_bytes_and_cid_are_unchanged`
pins that no existing CID moved). brit mirrors that discipline:

- add `parents` / `derived_from` to the **head-able commit-like node** — *not* to `EprMeta`,
  which the composition-snapshot spec §4 keeps **content-pure** (a `parent` field on the
  tree would make `seal` impure and re-CID on prior state). Lineage rides the commit node
  (git's tree/commit split), consistent with that spec's deferral.
- `brit-graph` gains **ancestry-set helpers**: `ancestors(cid) -> BTreeSet<BritCid>`, and
  `kinship(a, b) -> Kinship { Parent | Child | Sibling(common: BritCid) | Unrelated }` by
  set intersection (shared ancestor → sibling; appears-in-parents → child/parent). Exact,
  offline-checkable, no new entry type (P2P-gate: **A2 derived** — fields in existing hashed
  bytes / links).
- **Chunk-genome similarity** (fuzzy kin for *unrecorded* forks) is explicitly
  research-flavored/optional in the source spec; it lands as a **derived cache** (`brit-graph`
  MinHash over content-defined chunks) *after* the exact-lineage floor, never before.

### 5.b `exportedFrom` + boundary set → the `EprMeta`/`NodeSeed` export envelope

The kinship spec §3b: an export ships root CID + **`exportedFrom`** (source `EprRef` + snapshot
CID + timestamp — provenance) + an **explicit boundary set** (CIDs referenced-but-not-included
= declared holes). The composition-snapshot spec already notes brit's snapshot is "~90% of this
artifact." The stanza is **additive**, so — like eprfs lineage — it must be **append-only with a
stable empty encoding** (composition-snapshot spec §7.2: `#[serde(default)]`, no
`skip_serializing_if`, stable `None`/empty encodings) or it re-CIDs every existing snapshot.
Concretely a new `ExportEnvelope { root: BritCid, exported_from: Option<Provenance>, boundary:
Vec<BritCid> }` wrapping (not mutating) the sealed `EprMeta`/`NodeSeed`; the sealed inner node's
CID is unchanged. P2P-gate: content-derived artifact with its own CID, **local until shared**.

### 5.c Authority-anchored head declaration → design-level shape for `brit-graph`

The keystone (kinship spec §4): **kinship never confers the right to declare the merged head.**
Heads move only by a **new head DECLARATION carrying judgment provenance**, whose claim-chain
**terminates at a socially-trusted anchor** (an EPR with community-backstopped earned standing —
explicitly *not* key possession, *not* self-sovereign apex; the identity-sovereignty ontology
guard applies). This is the **same shape as the live substrate rule that canonical channels alone
move declared heads**, generalized from content heads to graph reconciliation. In brit's Layer-1
snapshot this is **design-level only** (Layer-2 / DHT runtime enforces; composition-snapshot spec
§4/§6 defers governance):

- a head is a **ref that moves**; the version-DAG **objects are immutable and never destroyed**
  (git refs-vs-objects — kinship spec §4(i)). `brit-graph` already models the immutable object
  DAG; a `HeadDeclaration { new_head: BritCid, judgment_provenance: EprRef, claim_chain:
  Vec<BritCid> }` is the *shape* that moves a ref, **validated only when connected** (offline: the
  carried lineage + export envelope ARE the trust snapshot; connected: the anchor's `EprRef`
  resolves to deep-validated standing — the floor/ceiling of kinship spec §4(iii)).
- encountering kin writes a **fingerprint-deduped kinship finding** (the existing *flag → agent →
  canon* ledger pattern); the graft is a **governance act, never an automatic merge** (§4(ii)).
- **the anchor must be community-grounded** — the spec must never let authority derive from
  lineage alone or from a self-custody apex (cite `stewardship-over-sovereignty`; the memory guard
  *identity-sovereignty-ontology-guard*). This is the one place the design *must* say "no" to the
  obvious crypto framing.

Full head-election, signature, gossip, and the judgment runtime (the "elohim" ceiling) are
**out of scope** here exactly as the source spec scopes them out — this section fixes the *shape*
so the Layer-2 slice composes onto it without a format migration.

---

## 6. Staged migration order (CI is the validation surface)

Local tooling here **cannot resolve brit's Nexus deps** (auth-required registry, no token — §probe).
So every stage that touches deps is **written to be correct-by-construction and validated in CI**,
not locally. Ordering is chosen so each stage is independently revertable and the compile-cascade
is contained.

- **Stage 0 (lands now, no deps):** the `is_fingerprint` → `baf` parity fix (§2) + its unit test.
  `cargo fmt --check`-clean; obviously correct; closes the live `remote`-verdict gap. **This is the
  only code that lands this pass** (§7).
- **Stage 1 (CI-validated):** add `eprfs-core` as an **ungated** `brit-epr` dependency; introduce a
  private `mod address_core` re-exporting `eprfs_core::BlobCid`'s derivation surface. **No public API
  change yet.** Gate: the whole crate still builds with `--no-default-features` (engine boundary).
- **Stage 2 (CI-validated):** `BritCid::{compute,compute_raw,short_fingerprint}` and
  `drift_fingerprint` **delegate** to the shared derivation; the codec constants `0x55`/`0x71` are
  deleted from `cid.rs` and imported from `eprfs-core`. Gate: **byte-identical golden vectors** —
  `empty_cbor_map_is_bafyrei`, `raw_blob_is_bafkrei`, `cid_fingerprint_derivation` unchanged; a new
  cross-crate test asserts `BritCid::compute_raw(b).short_fingerprint() ==
  BlobCid::compute_raw(b).short_fingerprint()`.
- **Stage 3 (CI-validated):** move the **verdict label enum + fingerprint predicate** to
  `eprfs-core::verdict`; `brit-epr`'s `Verdict` and `is_fingerprint` become re-exports. Gate: the
  parity harness (Python oracle) stays green; `#[serde(rename_all="kebab-case")]` wire labels
  byte-identical.
- **Stage 4 (deferred, gated on a decision):** the `BritCid` *type* unification — only if a tag-42
  `BlobLink` serde lands in `eprfs-core` (§4). Requires a dag-cbor golden byte-diff proving no
  `EprMeta` CID moved. **Do not attempt without that diff.**
- **Lineage / export-envelope / head-declaration (§5):** their own specs + plans, sequenced after
  the addressing consolidation (they depend on the shared `BlobCid`-family). Append-only encodings,
  golden "absent-feature bytes unchanged" pins mandatory (mirroring `eprfs-agent`'s lineage golden).

The compile-cascade guarantee the mandate wants appears at **Stage 2**: after delegation, deleting
or changing a shared derivation fn in `eprfs-core` produces a compile error in `brit-epr` (and every
other consumer) that *names* the integration point — the structural replacement for the fixture that
merely *observed* equality.

---

## 7. What lands this pass vs. deferred

- **Lands (code):** Stage 0 only — the `is_fingerprint` → `baf` fix + test in
  `brit-epr/src/engine/interface_ref.rs`. Committed path-scoped on the current branch; the
  integrator may cherry-pick.
- **Lands (design):** this spec.
- **Deferred to CI:** Stages 1–3 (dep wire + delegation + verdict-vocab move) — designed here,
  *implemented against CI* because the Nexus registry is unauthenticated locally. Attempting them
  blind (no build, no test) is the exact CI-breaking risk the task warns against.
- **Deferred to design:** Stage 4 (type unification behind a tag-42 serde) and the §5 versioning
  primitives (lineage, export envelope, head declaration) — each its own spec + plan, sequenced
  after addressing consolidation.

## 8. Risks

1. **The tag-42 re-CID (§4)** — the headline. A well-meaning "just alias BritCid to BlobCid" does
   not fail to compile; it silently rewrites canonical dag-cbor and moves every snapshot CID. Any
   type-level consolidation MUST be gated behind a dag-cbor golden byte-diff. This is documented so a
   future agent does not take the naive path.
2. **Ungated `eprfs-core` dep and the engine boundary** — if `eprfs-core` ever transitively pulls a
   heavy or protocol-coupled dep, adding it ungated would violate "works with `elohim-protocol` off."
   `eprfs-core` is lean today; Stage 1's `--no-default-features` build gate is the guard, and it can
   only be checked **in CI** here.
3. **Nexus resolvability of `eprfs-core` from brit's registry** — `eprfs` crates publish from `dev`
   (parent `4c033062e`); this is unverified locally (auth). The version pin and feature set can only
   be confirmed by the first CI build of Stage 1. Treat Stage 1 as a probe.
4. **Verdict-vocab wire drift (Stage 3)** — the `kebab-case` labels are a serialized contract read by
   the parity harness and any consumer; moving the enum must keep the rename byte-identical, verified
   by the harness, not assumed.
5. **Append-only regressions (§5)** — lineage / export-envelope / head fields that forget the
   stable-empty encoding re-CID every existing node. The `eprfs-agent` lineage golden is the pattern
   to copy verbatim; without an equivalent brit golden, the regression is invisible until a clone
   diverges.

## 9. Cross-references

- Parent lesson set: `genesis/docs/superpowers/specs/2026-07-12-cite-fingerprint-cid-convergence-design.md`,
  `genesis/docs/superpowers/specs/2026-07-12-epr-meta-kinship-lineage-reconciliation-design.md`.
- brit master + slice: `docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md`,
  `docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md`.
- Addressing canon: `elohim/eprfs/eprfs-core/src/address.rs` (`BlobCid`),
  `elohim/eprfs/eprfs-agent/src/canonical.rs` (lineage golden),
  `brit-epr/src/engine/cid.rs` (`BritCid`).
- Today's brit landings: `1df2fff23f` (derivation pin), `f6544964c0` (remote verdict).
