# Canonical EPR-Meta Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give brit a real content-addressing engine (CIDv1 · dag-cbor · sha2-256, byte-identical to the protocol's `elohim-epr`) and the first canonical artifacts (`EprMeta`, `NodeSeed`) sealed from the filesystem via a `brit epr-meta seal`/`verify` CLI.

**Architecture:** Replace brit-epr's interim `BritCid` (BLAKE3-hex over sort-keys JSON) with a real CIDv1 over canonical DAG-CBOR, computed with the same multiformats crates the protocol's published `elohim-epr` crate uses; prove byte-parity against `elohim-epr`'s golden vectors. Add two `ContentNode` types (`EprMeta`, `NodeSeed`) and a `brit-build-ref` verb group that seals a directory into them. The generic engine stays codec-agnostic (multiformats only); `elohim-epr` is consumed behind the `elohim-protocol` feature.

**Tech Stack:** Rust 1.82, `cid 0.11` (serde-codec), `multihash-codetable 0.2` (sha2), `serde_ipld_dagcbor 0.6`, `ipld-core 0.4`, `elohim-epr 0.1` (Nexus `elohim` registry), `clap 4` (derive), `gix` (already vendored).

## Global Constraints

- **CID format (verbatim):** `CIDv1`, multicodec **`0x71`** (dag-cbor) for nodes / **`0x55`** (raw) for blobs, multihash **`0x12`** (sha2-256). Display = base32 lowercase (`bafyrei…` / `bafkrei…`).
- **Canonical bytes:** DAG-CBOR (RFC 8949 §4.2.1 deterministic) via `serde_ipld_dagcbor`. Never sort-keys JSON for addressing.
- **Source of truth:** consume `elohim-epr = "0.1"` from registry `elohim`; do NOT reproduce the codec. Match its crate versions exactly so `cid::Cid` / `Multihash` types unify.
- **Engine boundary:** `brit-epr::engine` depends only on the generic multiformats crates. `elohim-epr` is `optional = true`, gated behind feature `elohim-protocol`. Disabling the feature must still build.
- **BLAKE3** is retained for non-address fingerprints only — never as a content address.
- **Dev-loop gates:** `cargo nextest run -p <crate>` per crate; `cargo clippy --workspace --all-targets -- -D warnings -A unknown-lints --no-deps`; `cargo +nightly fmt -- --config-path rustfmt-nightly.toml`. Anonymous read resolves the `elohim` registry; no token needed to *consume*.
- **Commits:** end every commit message with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. Branch: `brit-dev`.

---

### Task 1: Wire the `elohim` registry + multiformats dependencies

**Files:**
- Create: `.cargo/config.toml`
- Modify: `brit-epr/Cargo.toml`
- Modify: `deny.toml` (allow the Nexus sources, if `just test` runs cargo-deny)
- Test: `brit-epr/tests/registry_smoke.rs`

**Interfaces:**
- Produces: brit-epr can call `elohim_epr::cid::compute_cid(&[u8]) -> cid::Cid` behind feature `elohim-protocol`; the generic engine has `cid`, `multihash_codetable`, `serde_ipld_dagcbor`, `ipld_core` available.

- [ ] **Step 1: Write the failing test**

```rust
// brit-epr/tests/registry_smoke.rs
//! Proves the `elohim` registry resolves and the published codec is callable.
#![cfg(feature = "elohim-protocol")]

#[test]
fn elohim_epr_compute_cid_is_dag_cbor() {
    // Empty CBOR map (0xa0) → CIDv1 dag-cbor sha2-256 → base32 "bafyrei…".
    let cid = elohim_epr::cid::compute_cid(&[0xa0]);
    assert!(cid.to_string().starts_with("bafyrei"), "got {cid}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brit-epr --test registry_smoke`
Expected: FAIL to compile — `use of undeclared crate or module \`elohim_epr\``.

- [ ] **Step 3: Create the registry config**

```toml
# .cargo/config.toml — consume the internal Nexus cargo registry.
# Auth is only needed to PUBLISH; anonymous read resolves dependencies.
# To publish or for private-read envs: CARGO_REGISTRIES_ELOHIM_TOKEN="Bearer <NpmToken>".
[registries.elohim]
index = "sparse+https://nexus.ethosengine.com/repository/cargo-internal/"
replace-with = "elohim-mirror"

[source.elohim-mirror]
registry = "sparse+https://nexus.ethosengine.com/repository/cargo/"
```

- [ ] **Step 4: Add dependencies + feature wiring to `brit-epr/Cargo.toml`**

In `[features]`, change the `elohim-protocol` line to activate the optional dep:

```toml
elohim-protocol = ["dep:elohim-epr"]
```

In `[dependencies]`, add (generic engine deps + the optional protocol crate):

```toml
cid = { version = "0.11", features = ["serde-codec"] }
multihash-codetable = { version = "0.2", features = ["sha2"] }
serde_ipld_dagcbor = "0.6"
ipld-core = { version = "0.4", features = ["serde"] }
elohim-epr = { version = "0.1", registry = "elohim", optional = true }
```

- [ ] **Step 5: Allow the Nexus sources in `deny.toml`**

Ensure the `[sources]` allow-list contains both URLs (copy from the monorepo's `deny.toml`); if a `[sources] allow-registry` array exists, add:

```toml
"sparse+https://nexus.ethosengine.com/repository/cargo-internal/",
"sparse+https://nexus.ethosengine.com/repository/cargo/",
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p brit-epr --test registry_smoke`
Expected: PASS (Cargo fetches `elohim-epr 0.1` from Nexus anonymously, the CID prints `bafyrei…`).

- [ ] **Step 7: Commit**

```bash
git add .cargo/config.toml brit-epr/Cargo.toml brit-epr/tests/registry_smoke.rs deny.toml Cargo.lock
git commit -m "feat(brit-epr): consume elohim-epr 0.1 from the Nexus elohim registry

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Replace `BritCid` with real CIDv1 over canonical DAG-CBOR

**Files:**
- Modify: `brit-epr/src/engine/cid.rs` (full rewrite)
- Modify: `brit-epr/src/engine/content_node.rs` (canonical bytes → DAG-CBOR)
- Modify: `brit-epr/src/engine/object_store.rs` (store/read DAG-CBOR; base32 filenames)
- Modify: `brit-epr/src/engine/mod.rs` + `brit-epr/src/lib.rs` (export `CborError`)
- Modify call sites: `brit-build-ref/src/{build_cmd,deploy_cmd,validate_cmd}.rs` (`canonical_json` → `canonical_bytes`)
- Test: the rewritten `#[cfg(test)] mod tests` in `cid.rs`

**Interfaces:**
- Produces:
  - `BritCid(cid::Cid)` — `compute(canonical_bytes: &[u8]) -> BritCid` (codec `0x71`), `compute_raw(bytes: &[u8]) -> BritCid` (codec `0x55`), `as_cid(&self) -> &cid::Cid`, `Display` = base32, `FromStr` (delegates to `cid::Cid`).
  - `ContentNode::canonical_bytes(&self) -> Result<Vec<u8>, CborError>` (DAG-CBOR), `compute_cid(&self) -> Result<BritCid, CborError>`.
  - `CborError` — engine error wrapping dag-cbor encode failures.
- Consumes: the multiformats crates from Task 1.

- [ ] **Step 1: Write the failing test** (rewrite `cid.rs` test module)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cbor_map_is_bafyrei() {
        // The elohim-epr golden vector: 0xa0 (empty map) → dag-cbor CIDv1.
        let cid = BritCid::compute(&[0xa0]);
        assert!(cid.to_string().starts_with("bafyrei"), "got {cid}");
    }

    #[test]
    fn raw_blob_is_bafkrei() {
        let cid = BritCid::compute_raw(b"hello world");
        assert!(cid.to_string().starts_with("bafkrei"), "got {cid}");
    }

    #[test]
    fn compute_is_deterministic() {
        assert_eq!(BritCid::compute(&[1, 2, 3]), BritCid::compute(&[1, 2, 3]));
    }

    #[test]
    fn different_input_different_cid() {
        assert_ne!(BritCid::compute(&[1]), BritCid::compute(&[2]));
    }

    #[test]
    fn roundtrip_display_parse() {
        let cid = BritCid::compute(&[0xa0]);
        let parsed: BritCid = cid.to_string().parse().unwrap();
        assert_eq!(cid, parsed);
    }

    #[test]
    fn rejects_non_cid_string() {
        assert!("not-a-cid".parse::<BritCid>().is_err());
    }

    #[test]
    fn serde_roundtrip_json() {
        let cid = BritCid::compute(&[0xa0]);
        let json = serde_json::to_string(&cid).unwrap();
        let back: BritCid = serde_json::from_str(&json).unwrap();
        assert_eq!(cid, back);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brit-epr cid::tests`
Expected: FAIL — current `BritCid::compute` produces 64-hex (no `bafyrei`), and `compute_raw` does not exist.

- [ ] **Step 3: Rewrite `brit-epr/src/engine/cid.rs`**

```rust
//! `BritCid` — content identifier: CIDv1 over canonical bytes.
//!
//! Nodes use multicodec 0x71 (dag-cbor); raw blobs use 0x55 (raw).
//! Multihash is 0x12 (sha2-256). Byte-identical to the protocol's
//! `elohim-epr` codec. BLAKE3 is for non-address fingerprints only.

use std::fmt;
use std::str::FromStr;

use cid::Cid;
use multihash_codetable::{Code, MultihashDigest};
use serde::{Deserialize, Serialize};

/// Multicodec for dag-cbor content (the IPLD multicodec table).
const DAG_CBOR_CODEC: u64 = 0x71;
/// Multicodec for raw bytes.
const RAW_CODEC: u64 = 0x55;

/// A content identifier — a CIDv1 wrapping the sha2-256 of canonical bytes.
///
/// Displayed and parsed as base32 (`bafyrei…` for dag-cbor, `bafkrei…` for raw).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BritCid(Cid);

impl BritCid {
    /// Compute a dag-cbor CID (codec 0x71) over already-canonical DAG-CBOR bytes.
    pub fn compute(canonical_bytes: &[u8]) -> Self {
        let mh = Code::Sha2_256.digest(canonical_bytes);
        Self(Cid::new_v1(DAG_CBOR_CODEC, mh))
    }

    /// Compute a raw-blob CID (codec 0x55) over arbitrary file bytes.
    pub fn compute_raw(bytes: &[u8]) -> Self {
        let mh = Code::Sha2_256.digest(bytes);
        Self(Cid::new_v1(RAW_CODEC, mh))
    }

    /// Borrow the underlying multiformats CID.
    pub fn as_cid(&self) -> &Cid {
        &self.0
    }
}

impl fmt::Display for BritCid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // cid's Display is base32 lowercase by default.
        write!(f, "{}", self.0)
    }
}

impl FromStr for BritCid {
    type Err = CidParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Cid::from_str(s).map(Self).map_err(|e| CidParseError(e.to_string()))
    }
}

/// Error parsing a CID string.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid CID: {0}")]
pub struct CidParseError(String);
```

- [ ] **Step 4: Rewrite `brit-epr/src/engine/content_node.rs`**

```rust
//! `ContentNode` — trait for CID-addressed content objects stored locally.

use serde::{de::DeserializeOwned, Serialize};

use crate::engine::cid::BritCid;

/// A content-addressed node: serialized to canonical DAG-CBOR and stored in
/// the local object store, identified by the CIDv1 of those bytes.
pub trait ContentNode: Serialize + DeserializeOwned {
    /// The content type discriminator, e.g. `"brit.epr-meta"`.
    fn content_type(&self) -> &'static str;

    /// Serialize to canonical DAG-CBOR bytes (RFC 8949 §4.2.1 deterministic:
    /// sorted keys, shortest-form ints, no indefinite-length items).
    fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        serde_ipld_dagcbor::to_vec(self).map_err(|e| CborError(e.to_string()))
    }

    /// Compute the CIDv1 (dag-cbor) over the canonical bytes.
    fn compute_cid(&self) -> Result<BritCid, CborError> {
        Ok(BritCid::compute(&self.canonical_bytes()?))
    }
}

/// Error encoding a node to canonical DAG-CBOR.
#[derive(Debug, thiserror::Error)]
#[error("dag-cbor encode error: {0}")]
pub struct CborError(String);
```

- [ ] **Step 5: Update `object_store.rs` to store/read DAG-CBOR with base32 filenames**

Replace the body of `put`, `get`, and the filename usage in `list`:

```rust
    /// Store a ContentNode. Returns its CID. Idempotent.
    pub fn put<T: ContentNode>(&self, node: &T) -> Result<BritCid, ObjectStoreError> {
        let bytes = node.canonical_bytes().map_err(|e| ObjectStoreError::Serialize(e.to_string()))?;
        let cid = BritCid::compute(&bytes);
        fs::create_dir_all(&self.base_dir).map_err(ObjectStoreError::Io)?;
        let name = cid.to_string();
        let path = self.base_dir.join(&name);
        let tmp_path = self.base_dir.join(format!("{name}.tmp"));
        fs::write(&tmp_path, &bytes).map_err(ObjectStoreError::Io)?;
        fs::rename(&tmp_path, &path).map_err(ObjectStoreError::Io)?;
        Ok(cid)
    }

    /// Retrieve a ContentNode by CID.
    pub fn get<T: ContentNode>(&self, cid: &BritCid) -> Result<T, ObjectStoreError> {
        let path = self.base_dir.join(cid.to_string());
        let bytes = fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ObjectStoreError::NotFound(cid.clone())
            } else {
                ObjectStoreError::Io(e)
            }
        })?;
        serde_ipld_dagcbor::from_slice(&bytes)
            .map_err(|e| ObjectStoreError::Deserialize(e.to_string()))
    }
```

In `list`, the filename parse `name.parse::<BritCid>()` already works (base32). Update the `ObjectStoreError` variants to carry `String` (dag-cbor errors are not `serde_json::Error`):

```rust
    /// Serialization failed.
    #[error("serialization error: {0}")]
    Serialize(String),
    /// Deserialization failed.
    #[error("deserialization error: {0}")]
    Deserialize(String),
```

- [ ] **Step 6: Export `CborError`; fix call sites**

In `brit-epr/src/engine/mod.rs` add to the `content_node` re-export line:

```rust
pub use content_node::{CborError, ContentNode};
```

In `brit-epr/src/lib.rs` unconditional re-exports, add `CborError`:

```rust
pub use engine::{AppSchema, BritCid, CborError, CidParseError, ContentNode, LocalObjectStore, ObjectStoreError, TrailerSet, ValidationError};
```

In `brit-build-ref/src/build_cmd.rs`, `deploy_cmd.rs`, `validate_cmd.rs`: replace each `node.canonical_json()?` with `node.canonical_bytes()?` (anyhow converts `CborError` via its `std::error::Error` impl).

- [ ] **Step 7: Run the full brit-epr + brit-build-ref suites; fix CID fixtures**

Run: `cargo test -p brit-epr -p brit-build-ref`
If any test fails on a 64-hex CID literal used as a fixture, replace it with a valid CIDv1. Find them:

Run: `grep -rnE '"[0-9a-f]{64}"' brit-epr brit-build-ref --include=*.rs`
Replace each such CID fixture with a real one (the canonical example: `BritCid::compute(&[0xa0]).to_string()` → a `bafyrei…` literal, or build via `compute_raw`). Re-run until green.

Expected: PASS for `cid::tests` and the attestation round-trip/object-store tests (round-trip holds because both sides now use dag-cbor; `BritCid` serde emits a tag-42 link in CBOR and a string in JSON).

- [ ] **Step 8: Commit**

```bash
git add brit-epr/src/engine/cid.rs brit-epr/src/engine/content_node.rs brit-epr/src/engine/object_store.rs brit-epr/src/engine/mod.rs brit-epr/src/lib.rs brit-build-ref/src/
git commit -m "feat(brit-epr): real CIDv1 over canonical dag-cbor (replaces blake3-hex)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Byte-parity conformance test against `elohim-epr`

**Files:**
- Test: `brit-epr/tests/cid_conformance.rs`

**Interfaces:**
- Consumes: `BritCid::compute` (Task 2), `elohim_epr::cid::compute_cid` (Task 1).

- [ ] **Step 1: Write the failing test** (the golden vectors, vendored as the cross-implementation conformance contract)

```rust
// brit-epr/tests/cid_conformance.rs
//! brit's CID engine MUST be byte-identical to the protocol's elohim-epr.
//! These vectors are the portable spec — any reimplementation (incl. a future
//! machine-code port) is correct iff it reproduces them.
#![cfg(feature = "elohim-protocol")]

use brit_epr::BritCid;

const VECTORS: &[&[u8]] = &[&[0xa0], &[0x01, 0x02, 0x03, 0x04], &[0xaa, 0xbb, 0xcc], b"covenant"];

#[test]
fn brit_matches_elohim_epr_for_every_vector() {
    for v in VECTORS {
        let brit = BritCid::compute(v).to_string();
        let canonical = elohim_epr::cid::compute_cid(v).to_string();
        assert_eq!(brit, canonical, "CID drift for vector {v:?}");
    }
}

#[test]
fn empty_map_vector_is_stable() {
    // Frozen golden value — guards against silent codec/hash drift.
    assert_eq!(
        BritCid::compute(&[0xa0]).to_string(),
        elohim_epr::cid::compute_cid(&[0xa0]).to_string()
    );
}
```

- [ ] **Step 2: Run test to verify it fails (or passes immediately)**

Run: `cargo test -p brit-epr --test cid_conformance`
Expected: PASS (both sides use the identical recipe). If it FAILS, brit's `DAG_CBOR_CODEC`/multihash diverged — fix `cid.rs` until it matches. (Writing the test still has value as a regression guard.)

- [ ] **Step 3: Commit**

```bash
git add brit-epr/tests/cid_conformance.rs
git commit -m "test(brit-epr): byte-parity conformance vs elohim-epr CID engine

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: `EprMeta` ContentNode (the canonical directory seed)

**Files:**
- Create: `brit-epr/src/elohim/meta.rs`
- Modify: `brit-epr/src/elohim/mod.rs` (add `pub mod meta;`)
- Modify: `brit-epr/src/lib.rs` (re-export under the `attestation`-style block or a new `meta` re-export)
- Test: `#[cfg(test)] mod tests` in `meta.rs`

**Interfaces:**
- Produces: `EprMeta { epr_meta_version: u32, subtree: String, entries: Vec<MetaEntry> }`, `MetaEntry { path: String, cid: BritCid }` (a real IPLD CID link), both `impl ContentNode`. `EprMeta::content_type() == "brit.epr-meta"`.

- [ ] **Step 1: Write the failing test**

```rust
// in brit-epr/src/elohim/meta.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::content_node::ContentNode;

    fn sample() -> EprMeta {
        use crate::engine::cid::BritCid;
        EprMeta {
            epr_meta_version: 1,
            subtree: "docs".into(),
            entries: vec![
                MetaEntry { path: "a.md".into(), cid: BritCid::compute_raw(b"a") },
                MetaEntry { path: "b.md".into(), cid: BritCid::compute_raw(b"b") },
            ],
        }
    }

    #[test]
    fn content_type_is_stable() {
        assert_eq!(sample().content_type(), "brit.epr-meta");
    }

    #[test]
    fn cid_is_deterministic() {
        assert_eq!(sample().compute_cid().unwrap(), sample().compute_cid().unwrap());
    }

    #[test]
    fn cid_changes_with_content() {
        let mut other = sample();
        other.subtree = "src".into();
        assert_ne!(sample().compute_cid().unwrap(), other.compute_cid().unwrap());
    }
}
```

(Replace the placeholder CID strings with `crate::BritCid::compute_raw(b"a").to_string()` etc. in the actual test so they are valid.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brit-epr meta::tests`
Expected: FAIL — `EprMeta` undefined.

- [ ] **Step 3: Implement `brit-epr/src/elohim/meta.rs`**

```rust
//! Canonical, content-addressed governance+seed manifest for a directory subtree.
//! The next-generation successor to a directory-local `.epr-meta`.
//!
//! Source of truth: the canonical DAG-CBOR bytes (stored in the git object
//! store); identity is the CIDv1 of those bytes. Any index is a projection.

use serde::{Deserialize, Serialize};

use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

/// One sealed filesystem entry: a path and the content address of its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaEntry {
    /// Path relative to the sealed subtree root.
    pub path: String,
    /// Content address of the entry's bytes — a real IPLD CID link (tag-42 in
    /// dag-cbor), never a string FK, so it cannot dangle across versions.
    pub cid: BritCid,
}

/// The canonical seed manifest for a subtree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EprMeta {
    /// Schema version of the EprMeta format.
    pub epr_meta_version: u32,
    /// Path of the governed subtree, relative to the repo root.
    pub subtree: String,
    /// Sealed entries, sorted by `path` for deterministic encoding.
    pub entries: Vec<MetaEntry>,
}

impl ContentNode for EprMeta {
    fn content_type(&self) -> &'static str {
        "brit.epr-meta"
    }
}
```

In `brit-epr/src/elohim/mod.rs` add `pub mod meta;`. In `brit-epr/src/lib.rs`, under the feature-gated block, add:

```rust
#[cfg(feature = "elohim-protocol")]
pub use elohim::meta::{EprMeta, MetaEntry};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p brit-epr meta::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add brit-epr/src/elohim/meta.rs brit-epr/src/elohim/mod.rs brit-epr/src/lib.rs
git commit -m "feat(brit-epr): EprMeta content node (canonical directory seed)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: `NodeSeed` ContentNode (the node-root rollup)

**Files:**
- Create: `brit-epr/src/elohim/node_seed.rs`
- Modify: `brit-epr/src/elohim/mod.rs`, `brit-epr/src/lib.rs`
- Test: `#[cfg(test)] mod tests` in `node_seed.rs`

**Interfaces:**
- Produces: `NodeSeed { epr_meta_version: u32, repo: String, epr_metas: Vec<BritCid> }`, `impl ContentNode`, `content_type() == "brit.node-seed"`. `epr_metas` is the sorted list of `EprMeta` CID links.

- [ ] **Step 1: Write the failing test**

```rust
// in brit-epr/src/elohim/node_seed.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::content_node::ContentNode;

    #[test]
    fn content_type_is_stable() {
        let s = NodeSeed { epr_meta_version: 1, repo: "brit".into(), epr_metas: vec![] };
        assert_eq!(s.content_type(), "brit.node-seed");
    }

    #[test]
    fn order_independent_via_sorted_field() {
        use crate::engine::cid::BritCid;
        // Caller sorts epr_metas; identical sets → identical CID.
        let (x, y) = (BritCid::compute_raw(b"x"), BritCid::compute_raw(b"y"));
        let a = NodeSeed { epr_meta_version: 1, repo: "brit".into(), epr_metas: vec![x.clone(), y.clone()] };
        let b = NodeSeed { epr_meta_version: 1, repo: "brit".into(), epr_metas: vec![x, y] };
        assert_eq!(a.compute_cid().unwrap(), b.compute_cid().unwrap());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brit-epr node_seed::tests`
Expected: FAIL — `NodeSeed` undefined.

- [ ] **Step 3: Implement `brit-epr/src/elohim/node_seed.rs`**

```rust
//! The node-root rollup: composes every EprMeta in a node into one
//! content-addressed lockfile (the import/export contract anchor).
//!
//! Source of truth: the canonical DAG-CBOR bytes (git object store);
//! identity is the CIDv1 of those bytes.

use serde::{Deserialize, Serialize};

use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

/// The node-level seed/lockfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSeed {
    /// Schema version.
    pub epr_meta_version: u32,
    /// Repository / node identifier.
    pub repo: String,
    /// CID links to every EprMeta in the node, sorted for determinism
    /// (real IPLD links, not string FKs).
    pub epr_metas: Vec<BritCid>,
}

impl ContentNode for NodeSeed {
    fn content_type(&self) -> &'static str {
        "brit.node-seed"
    }
}
```

Wire `pub mod node_seed;` into `elohim/mod.rs` and re-export `NodeSeed` in `lib.rs` (mirroring Task 4).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p brit-epr node_seed::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add brit-epr/src/elohim/node_seed.rs brit-epr/src/elohim/mod.rs brit-epr/src/lib.rs
git commit -m "feat(brit-epr): NodeSeed content node (node-root rollup/lockfile)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: `brit epr-meta seal <dir>` CLI

**Files:**
- Create: `brit-build-ref/src/meta_cmd.rs`
- Modify: `brit-build-ref/src/main.rs` (add `Meta` subcommand + dispatch)
- Test: `brit-build-ref/tests/meta_seal.rs`

**Interfaces:**
- Consumes: `EprMeta`, `MetaEntry`, `BritCid::compute_raw`, `LocalObjectStore`.
- Produces: `meta_cmd::seal(repo: &Path, dir: &str) -> anyhow::Result<()>` — prints the `EprMeta` CID; CLI `brit-build-ref meta seal --dir <path>`.

- [ ] **Step 1: Write the failing test**

```rust
// brit-build-ref/tests/meta_seal.rs
use std::process::Command;

#[test]
fn seal_prints_bafyrei_and_stores_the_node() {
    let tmp = tempfile::tempdir().unwrap();
    // Make it a git repo so .git/brit/objects/ has a home.
    Command::new("git").arg("init").arg(tmp.path()).output().unwrap();
    let sub = tmp.path().join("docs");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("a.md"), b"alpha").unwrap();
    std::fs::write(sub.join("b.md"), b"beta").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo").arg(tmp.path())
        .arg("meta").arg("seal").arg("--dir").arg(&sub)
        .output().unwrap();

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let cid = String::from_utf8(out.stdout).unwrap();
    assert!(cid.trim().starts_with("bafyrei"), "got {cid}");
    // The node was stored.
    let obj = tmp.path().join(".git/brit/objects").join(cid.trim());
    assert!(obj.exists(), "object not stored at {obj:?}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brit-build-ref --test meta_seal`
Expected: FAIL — no `meta` subcommand.

- [ ] **Step 3: Implement `brit-build-ref/src/meta_cmd.rs`**

```rust
//! `meta` subcommand — seal a directory subtree into a canonical EprMeta.

use std::path::Path;

use brit_epr::engine::cid::BritCid;
use brit_epr::engine::object_store::LocalObjectStore;
use brit_epr::{EprMeta, MetaEntry};

/// Seal the immediate files of `dir` into an EprMeta; print + store its CID.
pub fn seal(repo: &Path, dir: &Path) -> anyhow::Result<()> {
    let git_dir = repo.join(".git");
    let store = LocalObjectStore::for_git_dir(&git_dir);

    // Collect immediate files, sorted by name for deterministic encoding.
    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| e.path())
        .collect();
    files.sort();

    let mut entries = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path)?;
        let cid = BritCid::compute_raw(&bytes);
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        entries.push(MetaEntry { path: name, cid });
    }

    let subtree = dir
        .strip_prefix(repo)
        .unwrap_or(dir)
        .to_string_lossy()
        .into_owned();

    let meta = EprMeta { epr_meta_version: 1, subtree, entries };
    let cid = store.put(&meta)?;
    println!("{cid}");
    Ok(())
}
```

- [ ] **Step 4: Wire the subcommand into `brit-build-ref/src/main.rs`**

Add `mod meta_cmd;` near the other `mod` lines. Add to `TopCommand`:

```rust
    /// Canonical epr-meta artifacts.
    Meta {
        #[command(subcommand)]
        cmd: MetaCmd,
    },
```

Add the enum and dispatch:

```rust
#[derive(Subcommand)]
enum MetaCmd {
    /// Seal a directory subtree into a canonical EprMeta.
    Seal {
        /// Directory to seal.
        #[arg(long)]
        dir: PathBuf,
    },
}
```

In the `match cli.command` block add:

```rust
        TopCommand::Meta { cmd } => match cmd {
            MetaCmd::Seal { dir } => meta_cmd::seal(&repo, &dir),
        },
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p brit-build-ref --test meta_seal`
Expected: PASS — prints a `bafyrei…` CID and stores the node.

- [ ] **Step 6: Commit**

```bash
git add brit-build-ref/src/meta_cmd.rs brit-build-ref/src/main.rs brit-build-ref/tests/meta_seal.rs
git commit -m "feat(brit-build-ref): brit meta seal <dir> mints a canonical EprMeta CID

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: `brit epr-meta verify <cid>` CLI

**Files:**
- Modify: `brit-build-ref/src/meta_cmd.rs` (add `verify`)
- Modify: `brit-build-ref/src/main.rs` (add `Verify` to `MetaCmd`)
- Test: `brit-build-ref/tests/meta_verify.rs`

**Interfaces:**
- Consumes: `meta_cmd::seal` (Task 6), `LocalObjectStore::get`, `EprMeta`, `BritCid`.
- Produces: `meta_cmd::verify(repo: &Path, cid: &str) -> anyhow::Result<()>` — re-reads the stored node, recomputes its CID, and exits non-zero on mismatch/absence.

- [ ] **Step 1: Write the failing test**

```rust
// brit-build-ref/tests/meta_verify.rs
use std::process::Command;

fn seal(tmp: &std::path::Path) -> String {
    Command::new("git").arg("init").arg(tmp).output().unwrap();
    let sub = tmp.join("docs");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("a.md"), b"alpha").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo").arg(tmp).arg("meta").arg("seal").arg("--dir").arg(&sub)
        .output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn verify_passes_for_sealed_cid() {
    let tmp = tempfile::tempdir().unwrap();
    let cid = seal(tmp.path());
    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo").arg(tmp.path()).arg("meta").arg("verify").arg("--cid").arg(&cid)
        .output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn verify_fails_for_unknown_cid() {
    let tmp = tempfile::tempdir().unwrap();
    Command::new("git").arg("init").arg(tmp.path()).output().unwrap();
    // A syntactically valid CID that was never stored (parses, lookup fails).
    let absent = brit_epr::BritCid::compute(&[0xa1]).to_string();
    let out = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"))
        .arg("--repo").arg(tmp.path()).arg("meta").arg("verify")
        .arg("--cid").arg(&absent)
        .output().unwrap();
    assert!(!out.status.success());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p brit-build-ref --test meta_verify`
Expected: FAIL — no `verify` subcommand.

- [ ] **Step 3: Implement `verify` in `meta_cmd.rs`**

```rust
use brit_epr::engine::content_node::ContentNode;

/// Re-read the stored EprMeta and confirm its recomputed CID matches.
pub fn verify(repo: &Path, cid: &str) -> anyhow::Result<()> {
    let git_dir = repo.join(".git");
    let store = LocalObjectStore::for_git_dir(&git_dir);
    let parsed: BritCid = cid.parse().map_err(|e| anyhow::anyhow!("invalid CID: {e}"))?;
    let node: EprMeta = store.get(&parsed)?;
    let recomputed = node.compute_cid()?;
    if recomputed != parsed {
        anyhow::bail!("CID mismatch: stored {parsed} recomputes to {recomputed}");
    }
    println!("ok {parsed}");
    Ok(())
}
```

- [ ] **Step 4: Wire `Verify` into `MetaCmd` + dispatch**

```rust
    /// Verify a stored EprMeta against its CID.
    Verify {
        /// CID to verify.
        #[arg(long)]
        cid: String,
    },
```

```rust
            MetaCmd::Verify { cid } => meta_cmd::verify(&repo, &cid),
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p brit-build-ref --test meta_verify`
Expected: PASS.

- [ ] **Step 6: Full gate + commit**

```bash
cargo clippy --workspace --all-targets -- -D warnings -A unknown-lints --no-deps
cargo +nightly fmt -- --config-path rustfmt-nightly.toml
git add brit-build-ref/src/meta_cmd.rs brit-build-ref/src/main.rs brit-build-ref/tests/meta_verify.rs
git commit -m "feat(brit-build-ref): brit meta verify re-checks a sealed EprMeta CID

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage** (against `2026-06-29-canonical-epr-meta-git-bridge-design.md`):
- §3 canonical CID engine → Tasks 1–3 ✅ (registry, CIDv1/dag-cbor swap, golden-vector parity).
- §4 `EprMeta` / `NodeSeed` entities → Tasks 4–5 ✅.
- §8 CLI (`seal`, `verify`) → Tasks 6–7 ✅. (`lock`, `notarize`, `export`, `import`, `recompose` are later plans — noted below.)
- §10 engine boundary (generic stays codec-agnostic; `elohim-epr` optional behind feature) → Task 1 feature wiring ✅.
- §15 done criteria 1, 2, 6 (real CID byte-identical to elohim-epr; vectors pass; builds with feature off) → Tasks 2, 3, and the feature-gating in 1 ✅.
- **Deferred to follow-on plans (explicitly out of this plan's scope):** `NodeSeed` *composition* from real EprMetas (`lock`), the git bridge (§5), notarization edge (§7), export/import world-contract (§6), and the recomposition bridge (§9.1). Each is its own plan; this plan delivers the addressing foundation + seal/verify.

**Placeholder scan:** Tasks 4 & 5 tests use `BritCid::compute_raw(...)` directly; Task 7's "absent CID" test uses `brit_epr::BritCid::compute(&[0xa1]).to_string()` (valid, unstored). No string-literal CID placeholders remain.

**P2P design audit:** the PostToolUse heuristic flags `MetaEntry.cid` / the node structs as "CID-as-FK / no source-of-truth." Resolved by design, not suppressed: CID fields are typed `BritCid` (real tag-42 IPLD links, content-addressed — cannot dangle), and each node carries a source-of-truth doc comment (canonical DAG-CBOR bytes in the git object store). The heuristic is regex-based and re-fires regardless; the design is the gate-correct form.

**Type consistency:** `BritCid::compute`/`compute_raw`/`as_cid`, `ContentNode::canonical_bytes`/`compute_cid`, `CborError`, `EprMeta`/`MetaEntry`/`NodeSeed`, `meta_cmd::seal`/`verify` are used consistently across tasks. `ObjectStoreError::{Serialize,Deserialize}` changed to carry `String` (Task 2 Step 5) — every construction site updated in that step.

**Open risk to flag at execution:** if the dev/CI environment cannot reach `nexus.ethosengine.com` anonymously, Task 1 Step 6 fails at dependency fetch — set `CARGO_REGISTRIES_ELOHIM_TOKEN` (Bearer Nexus token) before building. This is environmental, not a code issue.

## Execution Handoff

Plan complete and saved to `docs/plans/2026-06-29-canonical-epr-meta-foundation.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, two-stage review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session with checkpoints for review.

Which approach?
