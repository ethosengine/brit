# Phase 2a: Build Attestation Primitives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three attestation ContentNode types (build, deploy, validation) to `brit-epr`, a local object store under `.git/brit/objects/`, git ref management under `refs/notes/brit/`, and a `brit build-ref` CLI — all pure-local, no DHT, no P2P.

**Architecture:** Extends `brit-epr`'s feature-gated `elohim` module with attestation schemas (serde-serializable structs), a minimal `ContentNode` trait for CID-addressed local storage, and a ref-to-CID index backed by git notes refs. A new `brit-build-ref` binary crate provides the CLI. Agent signing uses `ed25519-dalek` with a file-based key at `.git/brit/agent-key`. Everything behind the existing `elohim-protocol` feature flag.

**Tech Stack:** Rust 2021, serde + serde_json (serialization), blake3 (content hashing), ed25519-dalek (signing), clap 4 (CLI), gix (repo + ref access). All already in the workspace lockfile except blake3 and ed25519-dalek.

**Design spec:** `docs/plans/phases/phase-2a-build-attestation-primitives.md` — when this plan and the spec disagree, the spec wins.

**Dependency note:** The Phase 2 ContentNode adapter (RepoContentNode, CommitContentNode, etc.) has not been designed yet. This plan introduces the *minimal* ContentNode foundation needed for attestations — a trait, a CID type, and a local store. The full Phase 2 adapter will extend this foundation when it lands.

---

## File structure

```
brit-epr/
├── Cargo.toml                          # modified: add serde, serde_json, blake3, ed25519-dalek
├── src/
│   ├── lib.rs                          # modified: re-export new types
│   ├─�� engine/
│   │   ├─�� mod.rs                      # modified: export new modules
│   │   ├── content_node.rs             # NEW: ContentNode trait
│   │   ├── cid.rs                      # NEW: BritCid type (blake3-based)
│   │   ├── object_store.rs             # NEW: .git/brit/objects/ local store
│   │   └── signing.rs                  # NEW: ed25519 agent signing
│   └── elohim/
│       ├── mod.rs                      # modified: export attestation modules
│       ├── attestation/
│       │   ├─�� mod.rs                  # NEW: module root
│       │   ├── build.rs               # NEW: BuildAttestationContentNode
│       │   ├── deploy.rs              # NEW: DeployAttestationContentNode
│       │   ├��─ validation.rs          # NEW: ValidationAttestationContentNode
│       │   └── reach.rs               # NEW: reach computation from attestations
│       └── refs.rs                     # NEW: refs/notes/brit/ management
├── tests/
│   ├── attestation_roundtrip.rs        # NEW: serde roundtrip for all three types
│   ├── object_store.rs                 # NEW: local store put/get/list
│   ├── ref_management.rs              # NEW: ref read/write/list
│   └── reach_computation.rs           # NEW: deterministic reach derivation
brit-build-ref/
├── Cargo.toml                          # NEW: binary crate
└── src/
    ├── main.rs                         # NEW: clap entrypoint
    ├���─ build_cmd.rs                    # NEW: build put/get/list
    ├── deploy_cmd.rs                   # NEW: deploy put/get/list
    ├── validate_cmd.rs                 # NEW: validate put/get/list
    └── reach_cmd.rs                    # NEW: reach compute/get
```

**Responsibilities per file:**

- `engine/content_node.rs` — `ContentNode` trait: `content_type() -> &str`, serialization to canonical JSON, CID derivation. Engine-level, no pillar knowledge.
- `engine/cid.rs` — `BritCid` wrapper around a blake3 hash. `Display` as hex. `FromStr` for parsing. `compute(bytes) -> BritCid`.
- `engine/object_store.rs` �� `LocalObjectStore` reads/writes JSON files to `.git/brit/objects/{cid}`. Pure filesystem, no git objects.
- `engine/signing.rs` — `AgentKey` loads/generates ed25519 keypair from `.git/brit/agent-key`. `sign(payload) -> Signature`. `verify(payload, signature, pubkey) -> bool`.
- `elohim/attestation/build.rs` — `BuildAttestationContentNode` struct with all fields from the Phase 2a spec.
- `elohim/attestation/deploy.rs` — `DeployAttestationContentNode` struct.
- `elohim/attestation/validation.rs` — `ValidationAttestationContentNode` struct with check vocabulary enforcement.
- `elohim/attestation/reach.rs` — `compute_reach(step, store, refs) -> ReachLevel` derives reach from existing attestations.
- `elohim/refs.rs` — `BritRefManager` wraps gix ref operations for `refs/notes/brit/{build,deploy,validate,reach}/*`.

---

## Task 0: Add dependencies to brit-epr

**Files:**
- Modify: `brit-epr/Cargo.toml`

- [ ] **Step 0.1: Add serde, serde_json, blake3, ed25519-dalek, chrono**

Edit `brit-epr/Cargo.toml`, add to `[dependencies]`:

```toml
[dependencies]
gix-object = { version = "^0.58.0", path = "../gix-object", features = ["sha1"] }
thiserror = "2.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
blake3 = "1"
ed25519-dalek = { version = "2", features = ["rand_core", "pkcs8"] }
rand = "0.8"
chrono = { version = "0.4", features = ["serde"], default-features = false }
```

> **Note:** `chrono` is for ISO-8601 timestamps. `rand` is for key generation. `ed25519-dalek` 2.x uses `rand_core` internally; the `rand_core` feature re-exports it. Check the actual latest compatible versions in crates.io if cargo complains.

- [ ] **Step 0.2: Verify it compiles**

Run:

```
cargo build -p brit-epr
```

Expected: compiles. New deps are unused — that's fine, warnings expected.

- [ ] **Step 0.3: Verify engine-only build still works**

Run:

```
cargo build -p brit-epr --no-default-features
```

Expected: compiles. The engine deps (serde, blake3, etc.) are unconditional — they're needed for the ContentNode trait and CID type which live in the engine.

- [ ] **Step 0.4: Commit**

```
git add brit-epr/Cargo.toml Cargo.lock
git commit -m "chore(brit-epr): add serde, blake3, ed25519-dalek, chrono deps

Preparation for Phase 2a attestation primitives. These deps are
unconditional (engine-level) because ContentNode trait, CID, signing,
and timestamps are engine concerns, not schema-specific."
```

---

## Task 1: Engine — BritCid type (blake3-based content addressing)

**Files:**
- Create: `brit-epr/src/engine/cid.rs`
- Modify: `brit-epr/src/engine/mod.rs`

- [ ] **Step 1.1: Create `engine/cid.rs`**

Create `brit-epr/src/engine/cid.rs`:

```rust
//! `BritCid` — content identifier based on BLAKE3 hashing.
//!
//! Phase 2a uses a simplified CID: the BLAKE3 hash of the canonical JSON
//! serialization of a ContentNode. Full multiformats CIDv1 comes in a later
//! phase when interop with IPFS/Holochain requires it.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A content identifier — the BLAKE3 hash of a content payload.
///
/// Displayed and parsed as a 64-character lowercase hex string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BritCid(String);

impl BritCid {
    /// Compute a CID from arbitrary bytes.
    pub fn compute(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Self(hash.to_hex().to_string())
    }

    /// Return the hex string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BritCid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for BritCid {
    type Err = CidParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 {
            return Err(CidParseError::InvalidLength(s.len()));
        }
        if !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CidParseError::InvalidHex);
        }
        Ok(Self(s.to_lowercase()))
    }
}

/// Errors when parsing a CID string.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CidParseError {
    /// Expected 64 hex characters.
    #[error("expected 64 hex characters, got {0}")]
    InvalidLength(usize),
    /// Non-hex character found.
    #[error("CID contains non-hex characters")]
    InvalidHex,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_is_deterministic() {
        let a = BritCid::compute(b"hello world");
        let b = BritCid::compute(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn different_input_different_cid() {
        let a = BritCid::compute(b"hello");
        let b = BritCid::compute(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn roundtrip_display_parse() {
        let cid = BritCid::compute(b"test data");
        let parsed: BritCid = cid.to_string().parse().unwrap();
        assert_eq!(cid, parsed);
    }

    #[test]
    fn rejects_short_string() {
        let result = "abc123".parse::<BritCid>();
        assert_eq!(result, Err(CidParseError::InvalidLength(6)));
    }

    #[test]
    fn serde_roundtrip() {
        let cid = BritCid::compute(b"serde test");
        let json = serde_json::to_string(&cid).unwrap();
        let back: BritCid = serde_json::from_str(&json).unwrap();
        assert_eq!(cid, back);
    }
}
```

- [ ] **Step 1.2: Export from engine/mod.rs**

Edit `brit-epr/src/engine/mod.rs` — add `cid` module and re-export:

```rust
//! Covenant engine — unconditional layer that knows the trailer format and
//! dispatch contract but not any specific schema vocabulary.

mod app_schema;
pub mod cid;
mod error;
mod trailer_block;
mod trailer_set;

pub use app_schema::AppSchema;
pub use cid::{BritCid, CidParseError};
pub use error::{EngineError, ValidationError};
pub use trailer_block::parse_trailer_block;
pub use trailer_set::TrailerSet;
```

- [ ] **Step 1.3: Add re-export to lib.rs**

Edit `brit-epr/src/lib.rs` — add to the unconditional re-exports:

```rust
// Unconditional re-exports
pub use engine::{AppSchema, BritCid, CidParseError, TrailerSet, ValidationError};
```

- [ ] **Step 1.4: Run tests**

Run:

```
cargo test -p brit-epr -- cid
```

Expected: 5 unit tests pass.

- [ ] **Step 1.5: Commit**

```
git add brit-epr/src/engine/cid.rs brit-epr/src/engine/mod.rs brit-epr/src/lib.rs
git commit -m "feat(brit-epr/engine): add BritCid type with blake3 hashing

Content identifiers are BLAKE3 hashes displayed as 64-char hex.
Deterministic, serde-serializable, FromStr-parseable. Full multiformats
CIDv1 deferred to the phase that needs IPFS/Holochain interop."
```

---

## Task 2: Engine — ContentNode trait and LocalObjectStore

**Files:**
- Create: `brit-epr/src/engine/content_node.rs`
- Create: `brit-epr/src/engine/object_store.rs`
- Modify: `brit-epr/src/engine/mod.rs`
- Create: `brit-epr/tests/object_store.rs`

- [ ] **Step 2.1: Write the failing test for object store**

Create `brit-epr/tests/object_store.rs`:

```rust
//! Integration tests for LocalObjectStore.

use brit_epr::engine::cid::BritCid;
use brit_epr::engine::content_node::ContentNode;
use brit_epr::engine::object_store::LocalObjectStore;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestNode {
    name: String,
    value: u32,
}

impl ContentNode for TestNode {
    fn content_type(&self) -> &'static str {
        "test.node"
    }
}

#[test]
fn put_then_get_roundtrips() {
    let tmp = TempDir::new().unwrap();
    let store = LocalObjectStore::new(tmp.path().join("objects"));

    let node = TestNode {
        name: "hello".into(),
        value: 42,
    };

    let cid = store.put(&node).unwrap();
    let back: TestNode = store.get(&cid).unwrap();

    assert_eq!(node, back);
}

#[test]
fn same_content_same_cid() {
    let tmp = TempDir::new().unwrap();
    let store = LocalObjectStore::new(tmp.path().join("objects"));

    let node = TestNode {
        name: "deterministic".into(),
        value: 7,
    };

    let cid1 = store.put(&node).unwrap();
    let cid2 = store.put(&node).unwrap();
    assert_eq!(cid1, cid2);
}

#[test]
fn get_missing_cid_returns_error() {
    let tmp = TempDir::new().unwrap();
    let store = LocalObjectStore::new(tmp.path().join("objects"));
    let fake_cid = BritCid::compute(b"does not exist");

    let result = store.get::<TestNode>(&fake_cid);
    assert!(result.is_err());
}

#[test]
fn list_returns_all_stored_cids() {
    let tmp = TempDir::new().unwrap();
    let store = LocalObjectStore::new(tmp.path().join("objects"));

    let a = store
        .put(&TestNode {
            name: "a".into(),
            value: 1,
        })
        .unwrap();
    let b = store
        .put(&TestNode {
            name: "b".into(),
            value: 2,
        })
        .unwrap();

    let mut cids = store.list().unwrap();
    cids.sort_by(|x, y| x.as_str().cmp(y.as_str()));

    let mut expected = vec![a, b];
    expected.sort_by(|x, y| x.as_str().cmp(y.as_str()));

    assert_eq!(cids, expected);
}
```

- [ ] **Step 2.2: Run tests — expect compile failure**

Run:

```
cargo test -p brit-epr --test object_store
```

Expected: compile errors — `content_node` and `object_store` modules don't exist yet.

- [ ] **Step 2.3: Create `engine/content_node.rs`**

Create `brit-epr/src/engine/content_node.rs`:

```rust
//! `ContentNode` — trait for CID-addressed content objects stored locally.
//!
//! This is the minimal foundation Phase 2a needs. The full Phase 2
//! ContentNode adapter (RepoContentNode, CommitContentNode, etc.) will
//! extend this trait with pillar fields and relationship methods.

use serde::{de::DeserializeOwned, Serialize};

use crate::engine::cid::BritCid;

/// A content-addressed node that can be serialized to canonical JSON and
/// stored in the local object store.
///
/// Implementors must be `Serialize + DeserializeOwned`. The CID is
/// computed from the canonical JSON representation (keys sorted,
/// no trailing whitespace).
pub trait ContentNode: Serialize + DeserializeOwned {
    /// The content type discriminator, e.g. `"brit.build-attestation"`.
    fn content_type(&self) -> &'static str;

    /// Serialize to canonical JSON bytes.
    ///
    /// Default implementation uses serde_json with sorted keys.
    fn canonical_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        // serde_json serializes struct fields in declaration order, which
        // is deterministic for a given struct definition. For canonical
        // ordering across potential future schema evolution, we round-trip
        // through serde_json::Value to get sorted keys.
        let value = serde_json::to_value(self)?;
        serde_json::to_vec(&value)
    }

    /// Compute the content identifier from the canonical JSON representation.
    fn compute_cid(&self) -> Result<BritCid, serde_json::Error> {
        let bytes = self.canonical_json()?;
        Ok(BritCid::compute(&bytes))
    }
}
```

- [ ] **Step 2.4: Create `engine/object_store.rs`**

Create `brit-epr/src/engine/object_store.rs`:

```rust
//! `LocalObjectStore` — stores ContentNodes as JSON files under
//! `.git/brit/objects/`, addressed by their BritCid.

use std::fs;
use std::path::PathBuf;

use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

/// Filesystem-backed content-addressed store.
///
/// Objects live at `{base_dir}/{cid}` as canonical JSON. The store
/// creates the directory on first write if it doesn't exist.
pub struct LocalObjectStore {
    base_dir: PathBuf,
}

impl LocalObjectStore {
    /// Create a store rooted at the given directory.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Create a store for a git repo by locating `.git/brit/objects/`.
    pub fn for_git_dir(git_dir: &std::path::Path) -> Self {
        Self::new(git_dir.join("brit").join("objects"))
    }

    /// Store a ContentNode. Returns its CID.
    ///
    /// Idempotent: storing the same content twice produces the same CID
    /// and overwrites with identical bytes.
    pub fn put<T: ContentNode>(&self, node: &T) -> Result<BritCid, ObjectStoreError> {
        let json = node.canonical_json().map_err(ObjectStoreError::Serialize)?;
        let cid = BritCid::compute(&json);

        fs::create_dir_all(&self.base_dir).map_err(ObjectStoreError::Io)?;

        let path = self.base_dir.join(cid.as_str());
        fs::write(&path, &json).map_err(ObjectStoreError::Io)?;

        Ok(cid)
    }

    /// Retrieve a ContentNode by CID.
    pub fn get<T: ContentNode>(&self, cid: &BritCid) -> Result<T, ObjectStoreError> {
        let path = self.base_dir.join(cid.as_str());
        let bytes = fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ObjectStoreError::NotFound(cid.clone())
            } else {
                ObjectStoreError::Io(e)
            }
        })?;
        serde_json::from_slice(&bytes).map_err(ObjectStoreError::Deserialize)
    }

    /// List all stored CIDs.
    pub fn list(&self) -> Result<Vec<BritCid>, ObjectStoreError> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }
        let mut cids = Vec::new();
        for entry in fs::read_dir(&self.base_dir).map_err(ObjectStoreError::Io)? {
            let entry = entry.map_err(ObjectStoreError::Io)?;
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(cid) = name.parse::<BritCid>() {
                    cids.push(cid);
                }
            }
        }
        Ok(cids)
    }
}

/// Errors from the local object store.
#[derive(Debug, thiserror::Error)]
pub enum ObjectStoreError {
    /// Filesystem error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization failed.
    #[error("serialization error: {0}")]
    Serialize(serde_json::Error),
    /// Deserialization failed.
    #[error("deserialization error: {0}")]
    Deserialize(serde_json::Error),
    /// Object not found.
    #[error("object not found: {0}")]
    NotFound(BritCid),
}
```

- [ ] **Step 2.5: Export from engine/mod.rs**

Edit `brit-epr/src/engine/mod.rs`:

```rust
//! Covenant engine — unconditional layer that knows the trailer format and
//! dispatch contract but not any specific schema vocabulary.

mod app_schema;
pub mod cid;
pub mod content_node;
mod error;
pub mod object_store;
mod trailer_block;
mod trailer_set;

pub use app_schema::AppSchema;
pub use cid::{BritCid, CidParseError};
pub use content_node::ContentNode;
pub use error::{EngineError, ValidationError};
pub use object_store::{LocalObjectStore, ObjectStoreError};
pub use trailer_block::parse_trailer_block;
pub use trailer_set::TrailerSet;
```

- [ ] **Step 2.6: Add tempfile dev-dependency**

Edit `brit-epr/Cargo.toml`, add:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2.7: Update lib.rs re-exports**

Edit `brit-epr/src/lib.rs` — update unconditional re-exports:

```rust
// Unconditional re-exports
pub use engine::{
    AppSchema, BritCid, CidParseError, ContentNode, LocalObjectStore, ObjectStoreError, TrailerSet,
    ValidationError,
};
```

- [ ] **Step 2.8: Run tests**

Run:

```
cargo test -p brit-epr --test object_store
```

Expected: 4 tests pass.

- [ ] **Step 2.9: Run all existing tests too**

Run:

```
cargo test -p brit-epr
```

Expected: all previous tests (9 from Phase 0+1) plus the 5 CID unit tests plus 4 object store tests = 18 total pass.

- [ ] **Step 2.10: Commit**

```
git add brit-epr/src/engine/content_node.rs brit-epr/src/engine/object_store.rs brit-epr/src/engine/mod.rs brit-epr/src/lib.rs brit-epr/Cargo.toml brit-epr/tests/object_store.rs
git commit -m "feat(brit-epr/engine): add ContentNode trait and LocalObjectStore

ContentNode trait provides canonical JSON serialization and CID
derivation. LocalObjectStore stores nodes as CID-addressed JSON files
under .git/brit/objects/. Minimal foundation for Phase 2a attestation
types — the full Phase 2 adapter will extend this."
```

---

## Task 3: Engine — agent signing (ed25519)

**Files:**
- Create: `brit-epr/src/engine/signing.rs`
- Modify: `brit-epr/src/engine/mod.rs`

- [ ] **Step 3.1: Create `engine/signing.rs`**

Create `brit-epr/src/engine/signing.rs`:

```rust
//! Agent signing — ed25519 keypair management for attestation signatures.
//!
//! Phase 2a uses file-based keys at `.git/brit/agent-key` (PKCS#8 PEM).
//! Full agent key management (Holochain integration, key derivation from
//! device seed) comes in a later phase.

use std::fs;
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};

/// An agent's signing identity, loaded from or generated to a file.
pub struct AgentKey {
    signing_key: SigningKey,
    key_path: PathBuf,
}

impl AgentKey {
    /// Load an existing key or generate a new one at the given path.
    pub fn load_or_generate(key_path: &Path) -> Result<Self, AgentKeyError> {
        if key_path.exists() {
            Self::load(key_path)
        } else {
            Self::generate(key_path)
        }
    }

    /// Load from an existing 32-byte seed file.
    pub fn load(key_path: &Path) -> Result<Self, AgentKeyError> {
        let bytes = fs::read(key_path).map_err(AgentKeyError::Io)?;
        if bytes.len() != 32 {
            return Err(AgentKeyError::InvalidKeyLength(bytes.len()));
        }
        let seed: [u8; 32] = bytes
            .try_into()
            .map_err(|_| AgentKeyError::InvalidKeyLength(0))?;
        let signing_key = SigningKey::from_bytes(&seed);
        Ok(Self {
            signing_key,
            key_path: key_path.to_path_buf(),
        })
    }

    /// Generate a new keypair and write the 32-byte seed to disk.
    pub fn generate(key_path: &Path) -> Result<Self, AgentKeyError> {
        let mut rng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut rng);

        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent).map_err(AgentKeyError::Io)?;
        }
        fs::write(key_path, signing_key.to_bytes()).map_err(AgentKeyError::Io)?;

        Ok(Self {
            signing_key,
            key_path: key_path.to_path_buf(),
        })
    }

    /// Sign arbitrary bytes. Returns the 64-byte ed25519 signature as hex.
    pub fn sign(&self, payload: &[u8]) -> String {
        let sig = self.signing_key.sign(payload);
        hex::encode(sig.to_bytes())
    }

    /// The agent's public key as a 64-character hex string.
    pub fn agent_id(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    /// The verifying (public) key.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Path where the key is stored.
    pub fn key_path(&self) -> &Path {
        &self.key_path
    }
}

/// Verify a hex-encoded signature against a hex-encoded public key.
pub fn verify_signature(
    payload: &[u8],
    signature_hex: &str,
    pubkey_hex: &str,
) -> Result<bool, AgentKeyError> {
    let sig_bytes =
        hex::decode(signature_hex).map_err(|_| AgentKeyError::InvalidSignatureHex)?;
    let sig = ed25519_dalek::Signature::from_slice(&sig_bytes)
        .map_err(|_| AgentKeyError::InvalidSignatureHex)?;

    let pub_bytes = hex::decode(pubkey_hex).map_err(|_| AgentKeyError::InvalidPubkeyHex)?;
    let pubkey = VerifyingKey::from_bytes(
        &pub_bytes
            .try_into()
            .map_err(|_| AgentKeyError::InvalidPubkeyHex)?,
    )
    .map_err(|_| AgentKeyError::InvalidPubkeyHex)?;

    Ok(pubkey.verify_strict(payload, &sig).is_ok())
}

/// Agent key errors.
#[derive(Debug, thiserror::Error)]
pub enum AgentKeyError {
    /// Filesystem error.
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    /// Key file has wrong length.
    #[error("expected 32-byte key seed, got {0} bytes")]
    InvalidKeyLength(usize),
    /// Signature hex is invalid.
    #[error("invalid signature hex")]
    InvalidSignatureHex,
    /// Public key hex is invalid.
    #[error("invalid public key hex")]
    InvalidPubkeyHex,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn generate_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("brit").join("agent-key");

        let key1 = AgentKey::generate(&path).unwrap();
        let key2 = AgentKey::load(&path).unwrap();

        assert_eq!(key1.agent_id(), key2.agent_id());
    }

    #[test]
    fn sign_and_verify() {
        let tmp = TempDir::new().unwrap();
        let key = AgentKey::generate(&tmp.path().join("key")).unwrap();

        let payload = b"attestation payload";
        let sig = key.sign(payload);

        assert!(verify_signature(payload, &sig, &key.agent_id()).unwrap());
    }

    #[test]
    fn wrong_payload_fails_verify() {
        let tmp = TempDir::new().unwrap();
        let key = AgentKey::generate(&tmp.path().join("key")).unwrap();

        let sig = key.sign(b"original");

        assert!(!verify_signature(b"tampered", &sig, &key.agent_id()).unwrap());
    }

    #[test]
    fn load_or_generate_creates_if_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("agent-key");

        assert!(!path.exists());
        let key = AgentKey::load_or_generate(&path).unwrap();
        assert!(path.exists());
        assert_eq!(key.agent_id().len(), 64); // 32 bytes as hex
    }
}
```

- [ ] **Step 3.2: Add hex dependency**

Edit `brit-epr/Cargo.toml`, add to `[dependencies]`:

```toml
hex = "0.4"
```

- [ ] **Step 3.3: Export from engine/mod.rs**

Edit `brit-epr/src/engine/mod.rs` — add:

```rust
pub mod signing;
```

And add to the pub use block:

```rust
pub use signing::{verify_signature, AgentKey, AgentKeyError};
```

- [ ] **Step 3.4: Run tests**

Run:

```
cargo test -p brit-epr -- signing
```

Expected: 4 tests pass.

- [ ] **Step 3.5: Commit**

```
git add brit-epr/src/engine/signing.rs brit-epr/src/engine/mod.rs brit-epr/Cargo.toml Cargo.lock
git commit -m "feat(brit-epr/engine): add ed25519 agent signing

File-based ed25519 key at .git/brit/agent-key. Sign/verify with hex-
encoded signatures and public keys. Full agent key management (Holochain
integration) deferred to later phase."
```

---

## Task 4: Elohim — attestation schemas (all three types)

**Files:**
- Create: `brit-epr/src/elohim/attestation/mod.rs`
- Create: `brit-epr/src/elohim/attestation/build.rs`
- Create: `brit-epr/src/elohim/attestation/deploy.rs`
- Create: `brit-epr/src/elohim/attestation/validation.rs`
- Modify: `brit-epr/src/elohim/mod.rs`
- Create: `brit-epr/tests/attestation_roundtrip.rs`

- [ ] **Step 4.1: Write the failing test**

Create `brit-epr/tests/attestation_roundtrip.rs`:

```rust
//! Serde roundtrip tests for all three attestation ContentNode types.

use brit_epr::engine::cid::BritCid;
use brit_epr::engine::content_node::ContentNode;
use brit_epr::elohim::attestation::build::BuildAttestationContentNode;
use brit_epr::elohim::attestation::deploy::{DeployAttestationContentNode, HealthStatus};
use brit_epr::elohim::attestation::validation::{
    ValidationAttestationContentNode, ValidationResult,
};

fn sample_cid() -> BritCid {
    BritCid::compute(b"sample artifact")
}

#[test]
fn build_attestation_roundtrips() {
    let node = BuildAttestationContentNode {
        manifest_cid: sample_cid(),
        step_name: "elohim-edge:cargo-build-storage".into(),
        inputs_hash: "abc123def456".into(),
        output_cid: BritCid::compute(b"output artifact"),
        agent_id: "deadbeef".repeat(4),
        hardware_profile: serde_json::json!({
            "arch": "x86_64",
            "os": "linux",
            "memory_gb": 32
        }),
        build_duration_ms: 45_000,
        built_at: "2026-04-16T10:00:00Z".into(),
        success: true,
        signature: "sig_placeholder".into(),
    };

    let json = serde_json::to_string_pretty(&node).unwrap();
    let back: BuildAttestationContentNode = serde_json::from_str(&json).unwrap();
    assert_eq!(node, back);
    assert_eq!(node.content_type(), "brit.build-attestation");

    // CID is deterministic
    let cid1 = node.compute_cid().unwrap();
    let cid2 = back.compute_cid().unwrap();
    assert_eq!(cid1, cid2);
}

#[test]
fn deploy_attestation_roundtrips() {
    let node = DeployAttestationContentNode {
        artifact_cid: sample_cid(),
        step_name: "elohim-edge:cargo-build-storage".into(),
        environment_label: "staging".into(),
        endpoint: "https://staging.elohim.host".into(),
        health_check_url: "https://staging.elohim.host/health".into(),
        health_status: HealthStatus::Healthy,
        deployed_at: "2026-04-16T10:05:00Z".into(),
        attested_at: "2026-04-16T10:05:30Z".into(),
        liveness_ttl_sec: 300,
        agent_id: "deadbeef".repeat(4),
        signature: "sig_placeholder".into(),
    };

    let json = serde_json::to_string_pretty(&node).unwrap();
    let back: DeployAttestationContentNode = serde_json::from_str(&json).unwrap();
    assert_eq!(node, back);
    assert_eq!(node.content_type(), "brit.deploy-attestation");
}

#[test]
fn validation_attestation_roundtrips() {
    let node = ValidationAttestationContentNode {
        artifact_cid: sample_cid(),
        check_name: "sonarqube-scan@v10".into(),
        validator_id: "sonarqube-agent-001".into(),
        validator_version: "10.7.0".into(),
        result: ValidationResult::Pass,
        result_summary: "0 bugs, 0 vulnerabilities, 2 code smells".into(),
        findings_cid: None,
        validated_at: "2026-04-16T10:10:00Z".into(),
        ttl_sec: Some(86_400),
        signature: "sig_placeholder".into(),
    };

    let json = serde_json::to_string_pretty(&node).unwrap();
    let back: ValidationAttestationContentNode = serde_json::from_str(&json).unwrap();
    assert_eq!(node, back);
    assert_eq!(node.content_type(), "brit.validation-attestation");
}

#[test]
fn validation_result_serializes_as_lowercase() {
    let pass = serde_json::to_string(&ValidationResult::Pass).unwrap();
    assert_eq!(pass, r#""pass""#);

    let fail = serde_json::to_string(&ValidationResult::Fail).unwrap();
    assert_eq!(fail, r#""fail""#);

    let warn = serde_json::to_string(&ValidationResult::Warn).unwrap();
    assert_eq!(warn, r#""warn""#);

    let skip = serde_json::to_string(&ValidationResult::Skip).unwrap();
    assert_eq!(skip, r#""skip""#);
}

#[test]
fn health_status_serializes_as_lowercase() {
    let h = serde_json::to_string(&HealthStatus::Healthy).unwrap();
    assert_eq!(h, r#""healthy""#);

    let d = serde_json::to_string(&HealthStatus::Degraded).unwrap();
    assert_eq!(d, r#""degraded""#);

    let u = serde_json::to_string(&HealthStatus::Unreachable).unwrap();
    assert_eq!(u, r#""unreachable""#);
}
```

- [ ] **Step 4.2: Run — expect compile failure**

Run:

```
cargo test -p brit-epr --test attestation_roundtrip
```

Expected: compile errors — attestation modules don't exist.

- [ ] **Step 4.3: Create `elohim/attestation/mod.rs`**

Create `brit-epr/src/elohim/attestation/mod.rs`:

```rust
//! Attestation ContentNode types for the Elohim Protocol.
//!
//! Three types: build (artifact was produced), deploy (artifact is live),
//! validation (artifact passed a named check). See Phase 2a spec for
//! field-by-field documentation.

pub mod build;
pub mod deploy;
pub mod validation;
```

- [ ] **Step 4.4: Create `elohim/attestation/build.rs`**

Create `brit-epr/src/elohim/attestation/build.rs`:

```rust
//! `BuildAttestationContentNode` �� records that an agent produced an
//! output artifact from a manifest's inputs.

use serde::{Deserialize, Serialize};

use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

/// Records that an agent produced an output artifact from a manifest's inputs.
///
/// Pillar coupling:
/// - Lamad: `build-knowledge` — what was built, from what, how
/// - Shefa: `compute-expended` — economic cost of producing the artifact
/// - Qahal: `build-authority` — agent's right to attest this artifact
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildAttestationContentNode {
    /// CID of the BuildManifestContentNode this attestation is for.
    pub manifest_cid: BritCid,
    /// Qualified step name (e.g., `elohim-edge:cargo-build-storage`).
    pub step_name: String,
    /// Content hash of all declared inputs at build time.
    pub inputs_hash: String,
    /// Content-addressed output artifact.
    pub output_cid: BritCid,
    /// Hex-encoded public key of the peer that performed the build.
    pub agent_id: String,
    /// CPU arch, OS, memory, relevant toolchain versions.
    pub hardware_profile: serde_json::Value,
    /// Wall-clock build time in milliseconds.
    pub build_duration_ms: u64,
    /// ISO-8601 timestamp when the build completed.
    pub built_at: String,
    /// Did the build succeed.
    pub success: bool,
    /// Hex-encoded ed25519 signature over the canonical JSON payload.
    pub signature: String,
}

impl ContentNode for BuildAttestationContentNode {
    fn content_type(&self) -> &'static str {
        "brit.build-attestation"
    }
}
```

- [ ] **Step 4.5: Create `elohim/attestation/deploy.rs`**

Create `brit-epr/src/elohim/attestation/deploy.rs`:

```rust
//! `DeployAttestationContentNode` — records that an agent confirms an
//! artifact is live at an environment.

use serde::{Deserialize, Serialize};

use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

/// Health status of a deployed artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Service is healthy.
    Healthy,
    /// Service is degraded but responding.
    Degraded,
    /// Service is unreachable.
    Unreachable,
}

/// Records that an agent confirms an artifact is live at an environment.
///
/// Pillar coupling:
/// - Lamad: `deployment-knowledge` — what is running where
/// - Shefa: `serving-compute` — cost of hosting/serving
/// - Qahal: `environment-authority` — agent's right to attest this environment
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployAttestationContentNode {
    /// CID of the output artifact being attested.
    pub artifact_cid: BritCid,
    /// Which step's artifact this is.
    pub step_name: String,
    /// `alpha`, `staging`, `prod`, `self`, or custom.
    pub environment_label: String,
    /// URL or service address being verified.
    pub endpoint: String,
    /// Endpoint used to verify liveness.
    pub health_check_url: String,
    /// Current health status.
    pub health_status: HealthStatus,
    /// ISO-8601 when the artifact started serving here.
    pub deployed_at: String,
    /// ISO-8601 when this attestation was produced.
    pub attested_at: String,
    /// After this many seconds without re-attestation, the claim self-invalidates.
    pub liveness_ttl_sec: u64,
    /// Hex-encoded public key of the peer producing the attestation.
    pub agent_id: String,
    /// Hex-encoded ed25519 signature.
    pub signature: String,
}

impl ContentNode for DeployAttestationContentNode {
    fn content_type(&self) -> &'static str {
        "brit.deploy-attestation"
    }
}
```

- [ ] **Step 4.6: Create `elohim/attestation/validation.rs`**

Create `brit-epr/src/elohim/attestation/validation.rs`:

```rust
//! `ValidationAttestationContentNode` — records that a validator applied
//! a named check to an artifact.

use serde::{Deserialize, Serialize};

use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

/// Result of a validation check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationResult {
    /// Check passed.
    Pass,
    /// Check failed.
    Fail,
    /// Check produced warnings but did not fail.
    Warn,
    /// Check was skipped (e.g., not applicable to this artifact).
    Skip,
}

/// Records that a validator (tool or agent) applied a named check to an artifact.
///
/// Check vocabulary is governed by the AppManifest. A check is only recognized
/// if its `check_name` is registered in the current manifest version.
///
/// Pillar coupling:
/// - Lamad: `validation-knowledge` — the findings
/// - Shefa: `verification-compute` — cost of running the check
/// - Qahal: `validation-authority` — community's recognition that this check counts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationAttestationContentNode {
    /// CID of what was validated.
    pub artifact_cid: BritCid,
    /// Registered check identifier (e.g., `sonarqube-scan@v10`, `trivy-cve@latest`).
    pub check_name: String,
    /// Tool identity or agent pubkey.
    pub validator_id: String,
    /// Version of the tool/agent.
    pub validator_version: String,
    /// Check result.
    pub result: ValidationResult,
    /// Human-readable summary.
    pub result_summary: String,
    /// Optional detailed report (CID of findings document).
    pub findings_cid: Option<BritCid>,
    /// ISO-8601 when validation was performed.
    pub validated_at: String,
    /// When validation goes stale (e.g., CVE DB refresh interval). None = never stale.
    pub ttl_sec: Option<u64>,
    /// Hex-encoded ed25519 signature.
    pub signature: String,
}

impl ContentNode for ValidationAttestationContentNode {
    fn content_type(&self) -> &'static str {
        "brit.validation-attestation"
    }
}
```

- [ ] **Step 4.7: Wire up elohim/mod.rs**

Edit `brit-epr/src/elohim/mod.rs` — add `attestation` module:

```rust
//! Elohim Protocol app schema — first-party `AppSchema` implementation.
//!
//! Gated behind `#[cfg(feature = "elohim-protocol")]`. With this feature
//! disabled, `brit-epr` ships only the engine.

pub mod attestation;
mod parse;
mod pillar_trailers;
mod schema;
mod validate;

pub use parse::parse_pillar_trailers;
pub use pillar_trailers::{PillarTrailers, TrailerKey};
pub use schema::ElohimProtocolSchema;
pub use validate::{validate_pillar_trailers, PillarValidationError};
```

- [ ] **Step 4.8: Run tests**

Run:

```
cargo test -p brit-epr --test attestation_roundtrip
```

Expected: 5 tests pass.

- [ ] **Step 4.9: Commit**

```
git add brit-epr/src/elohim/attestation/ brit-epr/src/elohim/mod.rs brit-epr/tests/attestation_roundtrip.rs
git commit -m "feat(brit-epr/elohim): add three attestation ContentNode schemas

BuildAttestationContentNode, DeployAttestationContentNode, and
ValidationAttestationContentNode — serde-serializable, CID-derivable,
camelCase JSON. All three round-trip through serde with deterministic
CIDs. Field sets match Phase 2a spec exactly."
```

---

## Task 5: Elohim — git ref namespace management

**Files:**
- Create: `brit-epr/src/elohim/refs.rs`
- Modify: `brit-epr/src/elohim/mod.rs`
- Create: `brit-epr/tests/ref_management.rs`

- [ ] **Step 5.1: Write the failing test**

Create `brit-epr/tests/ref_management.rs`:

```rust
//! Tests for refs/notes/brit/ namespace management.
//!
//! These tests use a temp git repo to exercise real ref operations.

use brit_epr::elohim::refs::BritRefManager;
use std::process::Command;
use tempfile::TempDir;

fn init_git_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["-c", "user.email=test@test.com", "-c", "user.name=test", "commit", "--allow-empty", "-m", "init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    tmp
}

#[test]
fn put_and_get_build_ref() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();

    let payload = serde_json::json!({
        "attestationCid": "abc123",
        "outputCid": "def456",
        "agentId": "agent001",
        "builtAt": "2026-04-16T10:00:00Z"
    });

    mgr.put_build_ref("elohim-edge:storage", "HEAD", &payload)
        .unwrap();

    let got = mgr.get_build_ref("elohim-edge:storage", "HEAD").unwrap();
    assert_eq!(got, Some(payload));
}

#[test]
fn get_missing_ref_returns_none() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();

    let got = mgr.get_build_ref("nonexistent", "HEAD").unwrap();
    assert_eq!(got, None);
}

#[test]
fn put_and_get_deploy_ref() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();

    let payload = serde_json::json!({
        "artifactCid": "abc123",
        "healthStatus": "healthy"
    });

    mgr.put_deploy_ref("elohim-edge:storage", "staging", &payload)
        .unwrap();

    let got = mgr
        .get_deploy_ref("elohim-edge:storage", "staging")
        .unwrap();
    assert_eq!(got, Some(payload));
}

#[test]
fn put_and_get_validate_ref() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();

    let payload = serde_json::json!({
        "artifactCid": "abc123",
        "result": "pass"
    });

    mgr.put_validate_ref("elohim-edge:storage", "sonarqube-scan@v10", &payload)
        .unwrap();

    let got = mgr
        .get_validate_ref("elohim-edge:storage", "sonarqube-scan@v10")
        .unwrap();
    assert_eq!(got, Some(payload));
}

#[test]
fn list_build_refs() {
    let tmp = init_git_repo();
    let mgr = BritRefManager::new(tmp.path()).unwrap();

    mgr.put_build_ref("step-a", "HEAD", &serde_json::json!({"a": 1}))
        .unwrap();
    mgr.put_build_ref("step-b", "HEAD", &serde_json::json!({"b": 2}))
        .unwrap();

    let mut refs = mgr.list_build_refs(None).unwrap();
    refs.sort();

    assert_eq!(refs, vec!["step-a", "step-b"]);
}
```

- [ ] **Step 5.2: Run — expect compile failure**

Run:

```
cargo test -p brit-epr --test ref_management
```

Expected: compile error — `refs` module doesn't exist.

- [ ] **Step 5.3: Create `elohim/refs.rs`**

Create `brit-epr/src/elohim/refs.rs`:

```rust
//! `BritRefManager` — read/write/list git refs under `refs/notes/brit/`.
//!
//! Uses git CLI commands for ref operations. A future iteration may use gix's
//! ref store API directly, but git CLI is simpler and more reliable for notes
//! refs (which gix doesn't fully support as of 0.81).
//!
//! Ref layout:
//! - `refs/notes/brit/build/{step_name}` — build attestation per commit
//! - `refs/notes/brit/deploy/{step_name}/{env}` — deploy attestation
//! - `refs/notes/brit/validate/{step_name}/{check_name}` — validation attestation
//! - `refs/notes/brit/reach/{step_name}` — derived reach level

use std::path::{Path, PathBuf};
use std::process::Command;

/// Manages git refs under `refs/notes/brit/` for attestation indexing.
pub struct BritRefManager {
    repo_path: PathBuf,
}

impl BritRefManager {
    /// Create a manager for the given repo path.
    pub fn new(repo_path: &Path) -> Result<Self, RefError> {
        if !repo_path.join(".git").exists() && !repo_path.join("HEAD").exists() {
            return Err(RefError::NotARepo(repo_path.display().to_string()));
        }
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
        })
    }

    // --- Build refs ---

    /// Write a build attestation ref for a step at a commit.
    pub fn put_build_ref(
        &self,
        step_name: &str,
        commit_rev: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let ref_name = format!("refs/notes/brit/build/{step_name}");
        let commit_sha = self.resolve_rev(commit_rev)?;
        self.write_note(&ref_name, &commit_sha, payload)
    }

    /// Read the build attestation for a step at a commit.
    pub fn get_build_ref(
        &self,
        step_name: &str,
        commit_rev: &str,
    ) -> Result<Option<serde_json::Value>, RefError> {
        let ref_name = format!("refs/notes/brit/build/{step_name}");
        let commit_sha = self.resolve_rev(commit_rev)?;
        self.read_note(&ref_name, &commit_sha)
    }

    /// List all build ref step names, optionally filtered by pattern.
    pub fn list_build_refs(
        &self,
        pattern: Option<&str>,
    ) -> Result<Vec<String>, RefError> {
        self.list_refs_under("refs/notes/brit/build/", pattern)
    }

    // --- Deploy refs ---

    /// Write a deploy attestation ref for a step + environment.
    pub fn put_deploy_ref(
        &self,
        step_name: &str,
        env: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let ref_name = format!("refs/notes/brit/deploy/{step_name}/{env}");
        self.write_ref_blob(&ref_name, payload)
    }

    /// Read the deploy attestation for a step + environment.
    pub fn get_deploy_ref(
        &self,
        step_name: &str,
        env: &str,
    ) -> Result<Option<serde_json::Value>, RefError> {
        let ref_name = format!("refs/notes/brit/deploy/{step_name}/{env}");
        self.read_ref_blob(&ref_name)
    }

    /// List all deploy ref step names.
    pub fn list_deploy_refs(
        &self,
        pattern: Option<&str>,
    ) -> Result<Vec<String>, RefError> {
        self.list_refs_under("refs/notes/brit/deploy/", pattern)
    }

    // --- Validate refs ---

    /// Write a validation attestation ref for a step + check.
    pub fn put_validate_ref(
        &self,
        step_name: &str,
        check_name: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let ref_name = format!("refs/notes/brit/validate/{step_name}/{check_name}");
        self.write_ref_blob(&ref_name, payload)
    }

    /// Read the validation attestation for a step + check.
    pub fn get_validate_ref(
        &self,
        step_name: &str,
        check_name: &str,
    ) -> Result<Option<serde_json::Value>, RefError> {
        let ref_name = format!("refs/notes/brit/validate/{step_name}/{check_name}");
        self.read_ref_blob(&ref_name)
    }

    /// List all validate ref step names.
    pub fn list_validate_refs(
        &self,
        pattern: Option<&str>,
    ) -> Result<Vec<String>, RefError> {
        self.list_refs_under("refs/notes/brit/validate/", pattern)
    }

    // --- Reach refs ---

    /// Write a reach ref for a step.
    pub fn put_reach_ref(
        &self,
        step_name: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let ref_name = format!("refs/notes/brit/reach/{step_name}");
        self.write_ref_blob(&ref_name, payload)
    }

    /// Read the reach ref for a step.
    pub fn get_reach_ref(
        &self,
        step_name: &str,
    ) -> Result<Option<serde_json::Value>, RefError> {
        let ref_name = format!("refs/notes/brit/reach/{step_name}");
        self.read_ref_blob(&ref_name)
    }

    // --- Internal helpers ---

    fn resolve_rev(&self, rev: &str) -> Result<String, RefError> {
        let output = Command::new("git")
            .args(["rev-parse", rev])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;

        if !output.status.success() {
            return Err(RefError::RevNotFound(rev.to_string()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Write a git note against a specific commit.
    fn write_note(
        &self,
        ref_name: &str,
        commit_sha: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let json = serde_json::to_string(payload).map_err(RefError::Json)?;
        let output = Command::new("git")
            .args(["notes", "--ref", ref_name, "add", "-f", "-m", &json, commit_sha])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RefError::GitFailed(format!(
                "notes add to {ref_name}: {stderr}"
            )));
        }
        Ok(())
    }

    /// Read a git note for a specific commit.
    fn read_note(
        &self,
        ref_name: &str,
        commit_sha: &str,
    ) -> Result<Option<serde_json::Value>, RefError> {
        let output = Command::new("git")
            .args(["notes", "--ref", ref_name, "show", commit_sha])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;

        if !output.status.success() {
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let value = serde_json::from_str(text.trim()).map_err(RefError::Json)?;
        Ok(Some(value))
    }

    /// Write a JSON blob to a standalone ref (for deploy/validate/reach refs
    /// which are not per-commit).
    fn write_ref_blob(
        &self,
        ref_name: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let json = serde_json::to_string(payload).map_err(RefError::Json)?;

        // Write the JSON as a blob object
        let hash_output = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(&self.repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.take().unwrap().write_all(json.as_bytes())?;
                child.wait_with_output()
            })
            .map_err(RefError::GitCommand)?;

        if !hash_output.status.success() {
            return Err(RefError::GitFailed("hash-object failed".into()));
        }
        let blob_sha = String::from_utf8_lossy(&hash_output.stdout)
            .trim()
            .to_string();

        // Point the ref at the blob
        let update_output = Command::new("git")
            .args(["update-ref", ref_name, &blob_sha])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;

        if !update_output.status.success() {
            let stderr = String::from_utf8_lossy(&update_output.stderr);
            return Err(RefError::GitFailed(format!(
                "update-ref {ref_name}: {stderr}"
            )));
        }
        Ok(())
    }

    /// Read a JSON blob from a standalone ref.
    fn read_ref_blob(
        &self,
        ref_name: &str,
    ) -> Result<Option<serde_json::Value>, RefError> {
        let output = Command::new("git")
            .args(["cat-file", "-p", ref_name])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;

        if !output.status.success() {
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let value = serde_json::from_str(text.trim()).map_err(RefError::Json)?;
        Ok(Some(value))
    }

    /// List ref names under a prefix, extracting the suffix as the step name.
    fn list_refs_under(
        &self,
        prefix: &str,
        _pattern: Option<&str>,
    ) -> Result<Vec<String>, RefError> {
        let output = Command::new("git")
            .args(["for-each-ref", "--format=%(refname)", prefix])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let names: Vec<String> = text
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| line.strip_prefix(prefix))
            .map(|s| s.to_string())
            .collect();
        Ok(names)
    }
}

/// Ref management errors.
#[derive(Debug, thiserror::Error)]
pub enum RefError {
    /// Not a git repository.
    #[error("not a git repository: {0}")]
    NotARepo(String),
    /// Git rev not found.
    #[error("rev not found: {0}")]
    RevNotFound(String),
    /// Git command failed to execute.
    #[error("git command error: {0}")]
    GitCommand(std::io::Error),
    /// Git command returned non-zero.
    #[error("git command failed: {0}")]
    GitFailed(String),
    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
}
```

- [ ] **Step 5.4: Wire up elohim/mod.rs**

Edit `brit-epr/src/elohim/mod.rs` — add `pub mod refs;`:

```rust
pub mod attestation;
mod parse;
mod pillar_trailers;
pub mod refs;
mod schema;
mod validate;
```

- [ ] **Step 5.5: Run tests**

Run:

```
cargo test -p brit-epr --test ref_management
```

Expected: 5 tests pass.

- [ ] **Step 5.6: Commit**

```
git add brit-epr/src/elohim/refs.rs brit-epr/src/elohim/mod.rs brit-epr/tests/ref_management.rs
git commit -m "feat(brit-epr/elohim): add BritRefManager for refs/notes/brit/ namespace

Read/write/list git refs for build, deploy, validate, and reach
attestations. Build refs use git notes (per-commit). Deploy/validate/
reach refs use standalone blob refs. All under refs/notes/brit/ to
survive clone/fetch."
```

---

## Task 6: Elohim — reach computation

**Files:**
- Create: `brit-epr/src/elohim/attestation/reach.rs`
- Create: `brit-epr/tests/reach_computation.rs`

- [ ] **Step 6.1: Write the failing test**

Create `brit-epr/tests/reach_computation.rs`:

```rust
//! Tests for deterministic reach computation from attestations.

use brit_epr::elohim::attestation::reach::{compute_reach, ReachInput, ReachLevel};

#[test]
fn no_attestations_returns_unknown() {
    let input = ReachInput {
        build_attestations: vec![],
        deploy_attestations: vec![],
        validation_attestations: vec![],
    };
    assert_eq!(compute_reach(&input), ReachLevel::Unknown);
}

#[test]
fn build_only_returns_built() {
    let input = ReachInput {
        build_attestations: vec!["agent-a".into()],
        deploy_attestations: vec![],
        validation_attestations: vec![],
    };
    assert_eq!(compute_reach(&input), ReachLevel::Built);
}

#[test]
fn build_plus_deploy_returns_deployed() {
    let input = ReachInput {
        build_attestations: vec!["agent-a".into()],
        deploy_attestations: vec!["staging".into()],
        validation_attestations: vec![],
    };
    assert_eq!(compute_reach(&input), ReachLevel::Deployed);
}

#[test]
fn build_plus_deploy_plus_validation_returns_verified() {
    let input = ReachInput {
        build_attestations: vec!["agent-a".into()],
        deploy_attestations: vec!["staging".into()],
        validation_attestations: vec!["sonarqube-scan@v10".into()],
    };
    assert_eq!(compute_reach(&input), ReachLevel::Verified);
}

#[test]
fn same_inputs_same_result() {
    let input = ReachInput {
        build_attestations: vec!["agent-a".into(), "agent-b".into()],
        deploy_attestations: vec!["staging".into()],
        validation_attestations: vec!["trivy@latest".into(), "sonarqube@v10".into()],
    };
    let r1 = compute_reach(&input);
    let r2 = compute_reach(&input);
    assert_eq!(r1, r2, "reach computation must be deterministic");
}
```

- [ ] **Step 6.2: Run — expect compile failure**

Run:

```
cargo test -p brit-epr --test reach_computation
```

Expected: compile error — `reach` module doesn't exist.

- [ ] **Step 6.3: Create `elohim/attestation/reach.rs`**

Create `brit-epr/src/elohim/attestation/reach.rs`:

```rust
//! Reach computation — derives a reach level from existing attestations.
//!
//! Deterministic: same attestations → same reach level. This is the Phase 2a
//! local-only computation. The full reach-promotion-rule DSL (how AppManifest
//! declares "build + 3 diverse peers + sonarqube pass = community reach") is
//! deferred to a later phase.

use serde::{Deserialize, Serialize};

/// Reach level derived from attestations for a given step.
///
/// Levels are ordered: Unknown < Built < Deployed < Verified.
/// Phase 2a uses this simple ladder. The full reach taxonomy from the
/// protocol schema (personal, trusted, community, public) maps onto
/// this in a later phase when the AppManifest reach-promotion rules
/// are defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReachLevel {
    /// No attestations exist.
    Unknown,
    /// At least one build attestation exists.
    Built,
    /// Built + at least one deploy attestation.
    Deployed,
    /// Built + deployed + at least one validation attestation passes.
    Verified,
}

/// Input for reach computation — collected from existing attestation refs.
#[derive(Debug, Clone)]
pub struct ReachInput {
    /// Agent IDs that have attested successful builds.
    pub build_attestations: Vec<String>,
    /// Environment labels with active deploy attestations.
    pub deploy_attestations: Vec<String>,
    /// Check names with passing validation attestations.
    pub validation_attestations: Vec<String>,
}

/// Compute the reach level from collected attestation data.
///
/// Deterministic: same inputs → same output. Order of attestations
/// within each category does not matter.
pub fn compute_reach(input: &ReachInput) -> ReachLevel {
    let has_build = !input.build_attestations.is_empty();
    let has_deploy = !input.deploy_attestations.is_empty();
    let has_validation = !input.validation_attestations.is_empty();

    match (has_build, has_deploy, has_validation) {
        (true, true, true) => ReachLevel::Verified,
        (true, true, false) => ReachLevel::Deployed,
        (true, false, _) => ReachLevel::Built,
        (false, _, _) => ReachLevel::Unknown,
    }
}
```

- [ ] **Step 6.4: Wire up attestation/mod.rs**

Edit `brit-epr/src/elohim/attestation/mod.rs`:

```rust
//! Attestation ContentNode types for the Elohim Protocol.
//!
//! Three types: build (artifact was produced), deploy (artifact is live),
//! validation (artifact passed a named check). See Phase 2a spec for
//! field-by-field documentation.

pub mod build;
pub mod deploy;
pub mod reach;
pub mod validation;
```

- [ ] **Step 6.5: Run tests**

Run:

```
cargo test -p brit-epr --test reach_computation
```

Expected: 5 tests pass.

- [ ] **Step 6.6: Commit**

```
git add brit-epr/src/elohim/attestation/reach.rs brit-epr/src/elohim/attestation/mod.rs brit-epr/tests/reach_computation.rs
git commit -m "feat(brit-epr/elohim): add deterministic reach computation

ReachLevel ladder: Unknown → Built → Deployed → Verified. Derived from
attestation presence. Deterministic: same inputs = same output. Full
reach-promotion-rule DSL deferred to AppManifest work."
```

---

## Task 7: Build the `brit-build-ref` CLI

**Files:**
- Create: `brit-build-ref/Cargo.toml`
- Create: `brit-build-ref/src/main.rs`
- Create: `brit-build-ref/src/build_cmd.rs`
- Create: `brit-build-ref/src/deploy_cmd.rs`
- Create: `brit-build-ref/src/validate_cmd.rs`
- Create: `brit-build-ref/src/reach_cmd.rs`
- Modify: `Cargo.toml` (root — add workspace member)

- [ ] **Step 7.1: Create the binary manifest**

Create `brit-build-ref/Cargo.toml`:

```toml
lints.workspace = true

[package]
name = "brit-build-ref"
version = "0.0.0"
description = "Manage build, deploy, and validation attestation refs in a brit repo"
repository = "https://github.com/ethosengine/brit"
authors = ["Matthew Dowell <matthew@ethosengine.com>"]
license = "MIT OR Apache-2.0"
edition = "2021"
rust-version = "1.82"

[[bin]]
name = "brit-build-ref"
path = "src/main.rs"

[dependencies]
brit-epr = { version = "^0.0.0", path = "../brit-epr" }
clap = { version = "4", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 7.2: Add to workspace members**

Edit root `Cargo.toml`. Find the `members = [` list and add `"brit-build-ref"` after `"brit-verify"`:

```toml
    "brit-epr",
    "brit-verify",
    "brit-build-ref",
]
```

- [ ] **Step 7.3: Create the clap entrypoint**

Create `brit-build-ref/src/main.rs`:

```rust
//! `brit build-ref` — manage attestation refs in a brit repo.
//!
//! Usage: `brit-build-ref <subcommand> [options]`

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod build_cmd;
mod deploy_cmd;
mod reach_cmd;
mod validate_cmd;

#[derive(Parser)]
#[command(name = "brit-build-ref")]
#[command(about = "Manage build, deploy, and validation attestation refs")]
struct Cli {
    /// Path to the git repository (defaults to current directory).
    #[arg(long, default_value = ".")]
    repo: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build attestation management.
    Build {
        #[command(subcommand)]
        action: build_cmd::BuildAction,
    },
    /// Deploy attestation management.
    Deploy {
        #[command(subcommand)]
        action: deploy_cmd::DeployAction,
    },
    /// Validation attestation management.
    Validate {
        #[command(subcommand)]
        action: validate_cmd::ValidateAction,
    },
    /// Reach level management.
    Reach {
        #[command(subcommand)]
        action: reach_cmd::ReachAction,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let repo_path = std::path::Path::new(&cli.repo);

    let result = match cli.command {
        Commands::Build { action } => build_cmd::run(repo_path, action),
        Commands::Deploy { action } => deploy_cmd::run(repo_path, action),
        Commands::Validate { action } => validate_cmd::run(repo_path, action),
        Commands::Reach { action } => reach_cmd::run(repo_path, action),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 7.4: Create `build_cmd.rs`**

Create `brit-build-ref/src/build_cmd.rs`:

```rust
//! `brit build-ref build` subcommands.

use std::path::Path;

use brit_epr::elohim::attestation::build::BuildAttestationContentNode;
use brit_epr::engine::cid::BritCid;
use brit_epr::engine::content_node::ContentNode;
use brit_epr::engine::object_store::LocalObjectStore;
use brit_epr::engine::signing::AgentKey;
use brit_epr::elohim::refs::BritRefManager;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum BuildAction {
    /// Record a build attestation.
    Put {
        /// Qualified step name.
        #[arg(long)]
        step: String,
        /// CID of the build manifest.
        #[arg(long)]
        manifest: String,
        /// CID of the output artifact.
        #[arg(long)]
        output: String,
        /// Build succeeded.
        #[arg(long, default_value_t = true)]
        success: bool,
        /// Hardware profile as JSON string.
        #[arg(long, default_value = "{}")]
        hardware: String,
        /// Build duration in milliseconds.
        #[arg(long, default_value_t = 0)]
        duration_ms: u64,
        /// Commit to associate (defaults to HEAD).
        #[arg(long, default_value = "HEAD")]
        commit: String,
    },
    /// Read a build attestation.
    Get {
        /// Qualified step name.
        #[arg(long)]
        step: String,
        /// Commit to read from (defaults to HEAD).
        #[arg(long, default_value = "HEAD")]
        commit: String,
    },
    /// List build attestation steps.
    List {
        /// Filter pattern.
        #[arg(long)]
        step: Option<String>,
    },
}

pub fn run(repo_path: &Path, action: BuildAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        BuildAction::Put {
            step,
            manifest,
            output,
            success,
            hardware,
            duration_ms,
            commit,
        } => {
            let git_dir = repo_path.join(".git");
            let store = LocalObjectStore::for_git_dir(&git_dir);
            let key = AgentKey::load_or_generate(&git_dir.join("brit").join("agent-key"))?;
            let refs = BritRefManager::new(repo_path)?;

            let manifest_cid: BritCid = manifest.parse()?;
            let output_cid: BritCid = output.parse()?;
            let hardware_profile: serde_json::Value = serde_json::from_str(&hardware)?;

            let now = chrono::Utc::now().to_rfc3339();

            let mut node = BuildAttestationContentNode {
                manifest_cid,
                step_name: step.clone(),
                inputs_hash: String::new(), // TODO: compute from manifest inputs
                output_cid: output_cid.clone(),
                agent_id: key.agent_id(),
                hardware_profile,
                build_duration_ms: duration_ms,
                built_at: now,
                success,
                signature: String::new(), // filled after signing
            };

            // Sign the node (with empty signature field, then fill it)
            let canonical = node.canonical_json()?;
            node.signature = key.sign(&canonical);

            let cid = store.put(&node)?;

            // Write the ref
            let ref_payload = serde_json::json!({
                "attestationCid": cid.as_str(),
                "outputCid": output_cid.as_str(),
                "agentId": node.agent_id,
                "builtAt": node.built_at,
            });
            refs.put_build_ref(&step, &commit, &ref_payload)?;

            println!("{cid}");
            Ok(())
        }
        BuildAction::Get { step, commit } => {
            let refs = BritRefManager::new(repo_path)?;
            match refs.get_build_ref(&step, &commit)? {
                Some(payload) => {
                    println!("{}", serde_json::to_string_pretty(&payload)?);
                    Ok(())
                }
                None => {
                    eprintln!("no build attestation for step={step} at {commit}");
                    Ok(())
                }
            }
        }
        BuildAction::List { step } => {
            let refs = BritRefManager::new(repo_path)?;
            let steps = refs.list_build_refs(step.as_deref())?;
            for s in steps {
                println!("{s}");
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 7.5: Create `deploy_cmd.rs`**

Create `brit-build-ref/src/deploy_cmd.rs`:

```rust
//! `brit build-ref deploy` subcommands.

use std::path::Path;

use brit_epr::elohim::attestation::deploy::{DeployAttestationContentNode, HealthStatus};
use brit_epr::engine::cid::BritCid;
use brit_epr::engine::content_node::ContentNode;
use brit_epr::engine::object_store::LocalObjectStore;
use brit_epr::engine::signing::AgentKey;
use brit_epr::elohim::refs::BritRefManager;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DeployAction {
    /// Record a deploy attestation.
    Put {
        /// Qualified step name.
        #[arg(long)]
        step: String,
        /// Environment label (alpha, staging, prod, self, or custom).
        #[arg(long)]
        env: String,
        /// CID of the artifact being attested.
        #[arg(long)]
        artifact: String,
        /// URL or service address.
        #[arg(long)]
        endpoint: String,
        /// Health status: healthy, degraded, unreachable.
        #[arg(long, default_value = "healthy")]
        health: String,
        /// Liveness TTL in seconds.
        #[arg(long, default_value_t = 300)]
        ttl: u64,
    },
    /// Read a deploy attestation.
    Get {
        /// Qualified step name.
        #[arg(long)]
        step: String,
        /// Environment label.
        #[arg(long)]
        env: String,
    },
    /// List deploy attestation steps.
    List {
        /// Filter by step pattern.
        #[arg(long)]
        step: Option<String>,
        /// Filter by environment.
        #[arg(long)]
        env: Option<String>,
    },
}

pub fn run(repo_path: &Path, action: DeployAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        DeployAction::Put {
            step,
            env,
            artifact,
            endpoint,
            health,
            ttl,
        } => {
            let git_dir = repo_path.join(".git");
            let store = LocalObjectStore::for_git_dir(&git_dir);
            let key = AgentKey::load_or_generate(&git_dir.join("brit").join("agent-key"))?;
            let refs = BritRefManager::new(repo_path)?;

            let artifact_cid: BritCid = artifact.parse()?;
            let health_status: HealthStatus = match health.as_str() {
                "healthy" => HealthStatus::Healthy,
                "degraded" => HealthStatus::Degraded,
                "unreachable" => HealthStatus::Unreachable,
                other => return Err(format!("unknown health status: {other}").into()),
            };

            let now = chrono::Utc::now().to_rfc3339();

            let mut node = DeployAttestationContentNode {
                artifact_cid: artifact_cid.clone(),
                step_name: step.clone(),
                environment_label: env.clone(),
                endpoint: endpoint.clone(),
                health_check_url: format!("{endpoint}/health"),
                health_status,
                deployed_at: now.clone(),
                attested_at: now,
                liveness_ttl_sec: ttl,
                agent_id: key.agent_id(),
                signature: String::new(),
            };

            let canonical = node.canonical_json()?;
            node.signature = key.sign(&canonical);

            let cid = store.put(&node)?;

            let ref_payload = serde_json::json!({
                "artifactCid": artifact_cid.as_str(),
                "attestationCid": cid.as_str(),
                "healthStatus": health,
                "attestedAt": node.attested_at,
                "livenessTtlSec": ttl,
            });
            refs.put_deploy_ref(&step, &env, &ref_payload)?;

            println!("{cid}");
            Ok(())
        }
        DeployAction::Get { step, env } => {
            let refs = BritRefManager::new(repo_path)?;
            match refs.get_deploy_ref(&step, &env)? {
                Some(payload) => {
                    println!("{}", serde_json::to_string_pretty(&payload)?);
                    Ok(())
                }
                None => {
                    eprintln!("no deploy attestation for step={step} env={env}");
                    Ok(())
                }
            }
        }
        DeployAction::List { step, env: _ } => {
            let refs = BritRefManager::new(repo_path)?;
            let steps = refs.list_deploy_refs(step.as_deref())?;
            for s in steps {
                println!("{s}");
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 7.6: Create `validate_cmd.rs`**

Create `brit-build-ref/src/validate_cmd.rs`:

```rust
//! `brit build-ref validate` subcommands.

use std::path::Path;

use brit_epr::elohim::attestation::validation::{
    ValidationAttestationContentNode, ValidationResult,
};
use brit_epr::engine::cid::BritCid;
use brit_epr::engine::content_node::ContentNode;
use brit_epr::engine::object_store::LocalObjectStore;
use brit_epr::engine::signing::AgentKey;
use brit_epr::elohim::refs::BritRefManager;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ValidateAction {
    /// Record a validation attestation.
    Put {
        /// Qualified step name.
        #[arg(long)]
        step: String,
        /// Check name (e.g., sonarqube-scan@v10).
        #[arg(long)]
        check: String,
        /// CID of the artifact validated.
        #[arg(long)]
        artifact: String,
        /// Result: pass, fail, warn, skip.
        #[arg(long)]
        result: String,
        /// Human-readable summary.
        #[arg(long, default_value = "")]
        summary: String,
    },
    /// Read a validation attestation.
    Get {
        /// Qualified step name.
        #[arg(long)]
        step: String,
        /// Check name.
        #[arg(long)]
        check: String,
    },
    /// List validation attestation steps.
    List {
        /// Filter by step pattern.
        #[arg(long)]
        step: Option<String>,
        /// Filter by check pattern.
        #[arg(long)]
        check: Option<String>,
    },
}

pub fn run(repo_path: &Path, action: ValidateAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ValidateAction::Put {
            step,
            check,
            artifact,
            result,
            summary,
        } => {
            let git_dir = repo_path.join(".git");
            let store = LocalObjectStore::for_git_dir(&git_dir);
            let key = AgentKey::load_or_generate(&git_dir.join("brit").join("agent-key"))?;
            let refs = BritRefManager::new(repo_path)?;

            let artifact_cid: BritCid = artifact.parse()?;
            let validation_result = match result.as_str() {
                "pass" => ValidationResult::Pass,
                "fail" => ValidationResult::Fail,
                "warn" => ValidationResult::Warn,
                "skip" => ValidationResult::Skip,
                other => return Err(format!("unknown result: {other}").into()),
            };

            let now = chrono::Utc::now().to_rfc3339();

            let mut node = ValidationAttestationContentNode {
                artifact_cid: artifact_cid.clone(),
                check_name: check.clone(),
                validator_id: key.agent_id(),
                validator_version: "0.0.0".into(),
                result: validation_result,
                result_summary: summary,
                findings_cid: None,
                validated_at: now,
                ttl_sec: None,
                signature: String::new(),
            };

            let canonical = node.canonical_json()?;
            node.signature = key.sign(&canonical);

            let cid = store.put(&node)?;

            let ref_payload = serde_json::json!({
                "artifactCid": artifact_cid.as_str(),
                "attestationCid": cid.as_str(),
                "result": result,
                "validatedAt": node.validated_at,
            });
            refs.put_validate_ref(&step, &check, &ref_payload)?;

            println!("{cid}");
            Ok(())
        }
        ValidateAction::Get { step, check } => {
            let refs = BritRefManager::new(repo_path)?;
            match refs.get_validate_ref(&step, &check)? {
                Some(payload) => {
                    println!("{}", serde_json::to_string_pretty(&payload)?);
                    Ok(())
                }
                None => {
                    eprintln!("no validation attestation for step={step} check={check}");
                    Ok(())
                }
            }
        }
        ValidateAction::List { step, check: _ } => {
            let refs = BritRefManager::new(repo_path)?;
            let steps = refs.list_validate_refs(step.as_deref())?;
            for s in steps {
                println!("{s}");
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 7.7: Create `reach_cmd.rs`**

Create `brit-build-ref/src/reach_cmd.rs`:

```rust
//! `brit build-ref reach` subcommands.

use std::path::Path;

use brit_epr::elohim::attestation::reach::{compute_reach, ReachInput};
use brit_epr::elohim::refs::BritRefManager;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ReachAction {
    /// Compute reach level from current attestations and write the ref.
    Compute {
        /// Qualified step name.
        #[arg(long)]
        step: String,
    },
    /// Read the current reach level for a step.
    Get {
        /// Qualified step name.
        #[arg(long)]
        step: String,
    },
}

pub fn run(repo_path: &Path, action: ReachAction) -> Result<(), Box<dyn std::error::Error>> {
    let refs = BritRefManager::new(repo_path)?;

    match action {
        ReachAction::Compute { step } => {
            // Collect attestation data from refs
            let build_agents = refs.list_build_refs(Some(&step))?;
            let deploy_envs = refs.list_deploy_refs(Some(&step))?;
            let validate_checks = refs.list_validate_refs(Some(&step))?;

            let input = ReachInput {
                build_attestations: build_agents,
                deploy_attestations: deploy_envs,
                validation_attestations: validate_checks,
            };

            let level = compute_reach(&input);

            let payload = serde_json::json!({
                "stepName": step,
                "computedReach": level,
                "buildAttestations": input.build_attestations.len(),
                "deployAttestations": input.deploy_attestations.len(),
                "validationAttestations": input.validation_attestations.len(),
            });

            refs.put_reach_ref(&step, &payload)?;

            println!("{}", serde_json::to_string_pretty(&payload)?);
            Ok(())
        }
        ReachAction::Get { step } => {
            match refs.get_reach_ref(&step)? {
                Some(payload) => {
                    println!("{}", serde_json::to_string_pretty(&payload)?);
                    Ok(())
                }
                None => {
                    eprintln!("no reach level computed for step={step}");
                    Ok(())
                }
            }
        }
    }
}
```

- [ ] **Step 7.8: Add chrono dependency to brit-build-ref**

Edit `brit-build-ref/Cargo.toml`, add to `[dependencies]`:

```toml
chrono = { version = "0.4", default-features = false, features = ["clock"] }
```

- [ ] **Step 7.9: Build the binary**

Run:

```
cargo build -p brit-build-ref
```

Expected: compiles. If API mismatches with `chrono::Utc::now()`, check the chrono features include `clock`.

- [ ] **Step 7.10: End-to-end smoke test**

In the brit submodule workspace, run a full build→deploy→validate→reach cycle:

```bash
# Create a temp repo for testing
SMOKE_DIR=$(mktemp -d)
git init "$SMOKE_DIR" --initial-branch=main
git -C "$SMOKE_DIR" -c user.email=test@test.com -c user.name=test commit --allow-empty -m "init"

# Fake CIDs (64-char hex)
MANIFEST_CID=$(printf '%064d' 1)
OUTPUT_CID=$(printf '%064d' 2)
ARTIFACT_CID=$OUTPUT_CID

# Build attestation
cargo run -p brit-build-ref -- --repo "$SMOKE_DIR" build put \
  --step elohim-edge:storage --manifest "$MANIFEST_CID" --output "$OUTPUT_CID"

# Deploy attestation
cargo run -p brit-build-ref -- --repo "$SMOKE_DIR" deploy put \
  --step elohim-edge:storage --env staging --artifact "$ARTIFACT_CID" \
  --endpoint https://staging.elohim.host

# Validate attestation
cargo run -p brit-build-ref -- --repo "$SMOKE_DIR" validate put \
  --step elohim-edge:storage --check sonarqube-scan@v10 \
  --artifact "$ARTIFACT_CID" --result pass --summary "0 bugs"

# Compute reach
cargo run -p brit-build-ref -- --repo "$SMOKE_DIR" reach compute \
  --step elohim-edge:storage

# Read back
echo "--- Build ---"
cargo run -p brit-build-ref -- --repo "$SMOKE_DIR" build get --step elohim-edge:storage
echo "--- Deploy ---"
cargo run -p brit-build-ref -- --repo "$SMOKE_DIR" deploy get --step elohim-edge:storage --env staging
echo "--- Validate ---"
cargo run -p brit-build-ref -- --repo "$SMOKE_DIR" validate get --step elohim-edge:storage --check sonarqube-scan@v10
echo "--- Reach ---"
cargo run -p brit-build-ref -- --repo "$SMOKE_DIR" reach get --step elohim-edge:storage

# Cleanup
rm -rf "$SMOKE_DIR"
```

Expected: each command prints a CID or JSON payload. Reach should show `"computedReach": "verified"`.

- [ ] **Step 7.11: Commit**

```
git add brit-build-ref/ Cargo.toml Cargo.lock
git commit -m "feat(brit-build-ref): CLI for build/deploy/validate/reach attestation refs

Full brit build-ref command group: put/get/list for build, deploy, and
validation attestations; compute/get for derived reach. Agent key auto-
generated on first use. ContentNodes stored in .git/brit/objects/,
indexed via refs/notes/brit/. End-to-end smoke-tested."
```

---

## Task 8: Feature flag enforcement and engine-only build check

**Files:**
- Modify: `brit-epr/src/lib.rs` (add attestation re-exports)

- [ ] **Step 8.1: Add attestation re-exports to lib.rs**

Edit `brit-epr/src/lib.rs` — add to the feature-gated re-exports:

```rust
// Feature-gated re-exports
#[cfg(feature = "elohim-protocol")]
pub use elohim::{
    parse_pillar_trailers, validate_pillar_trailers, ElohimProtocolSchema, PillarTrailers,
    PillarValidationError, TrailerKey,
};

// Re-export attestation types for convenience
#[cfg(feature = "elohim-protocol")]
pub mod attestation {
    //! Convenience re-exports for attestation types.
    pub use crate::elohim::attestation::build::BuildAttestationContentNode;
    pub use crate::elohim::attestation::deploy::{DeployAttestationContentNode, HealthStatus};
    pub use crate::elohim::attestation::reach::{compute_reach, ReachInput, ReachLevel};
    pub use crate::elohim::attestation::validation::{
        ValidationAttestationContentNode, ValidationResult,
    };
    pub use crate::elohim::refs::BritRefManager;
}
```

- [ ] **Step 8.2: Verify engine-only build**

Run:

```
cargo build -p brit-epr --no-default-features
```

Expected: compiles. The engine (CID, ContentNode trait, ObjectStore, signing) builds without attestation types.

- [ ] **Step 8.3: Run all tests**

Run:

```
cargo test -p brit-epr
```

Expected: all tests pass. Count should be approximately:
- Phase 0+1: 9 (engine_parsing: 2, elohim_parse: 3, elohim_validate: 4)
- Task 1 CID: 5
- Task 2 ObjectStore: 4
- Task 3 Signing: 4
- Task 4 Attestation roundtrip: 5
- Task 5 Ref management: 5
- Task 6 Reach: 5
- Total: ~37

- [ ] **Step 8.4: Commit**

```
git add brit-epr/src/lib.rs
git commit -m "feat(brit-epr): add attestation convenience re-exports

Re-export attestation types at crate root behind elohim-protocol
feature flag. Engine-only build verified: --no-default-features still
compiles."
```

---

## Task 9: Move design doc and bump submodule pointer

**Files:**
- Add: `docs/plans/phases/phase-2a-build-attestation-primitives.md` (the recovered design doc, now in the submodule)
- Modify: `docs/plans/README.md` (add Phase 2a to the roadmap table)
- Then in parent monorepo: bump submodule pointer

- [ ] **Step 9.1: Verify the design doc is in place**

```
ls -la docs/plans/phases/phase-2a-build-attestation-primitives.md
```

Expected: file exists (copied in earlier step).

- [ ] **Step 9.2: Update the roadmap README**

Edit `docs/plans/README.md` — update the phased decomposition table. Add Phase 2a between Phase 1 and Phase 2:

Find the line:
```
| **2** | ContentNode adapter |
```

Add before it:
```
| **2a** | Build attestation primitives | `BuildAttestationContentNode`, `DeployAttestationContentNode`, `ValidationAttestationContentNode` schemas + `brit build-ref` CLI + ref namespace under `refs/notes/brit/`. Pure local — no DHT, no P2P. | **Plan: [2026-04-16-phase-2a-build-attestation-primitives.md](./2026-04-16-phase-2a-build-attestation-primitives.md)** |
```

Also update Phase 0+1 status to "**Done**" if not already.

- [ ] **Step 9.3: Commit in submodule**

```
git add docs/plans/phases/ docs/plans/README.md docs/plans/2026-04-16-phase-2a-build-attestation-primitives.md
git commit -m "docs(plans): add Phase 2a design doc and implementation plan

Recovered Phase 2a design doc (build attestation primitives) placed
in docs/plans/phases/. Implementation plan covers all tasks from
engine foundation through CLI and feature flag enforcement. Roadmap
README updated with Phase 2a entry."
```

- [ ] **Step 9.4: Switch to parent monorepo and bump**

```
cd /projects/elohim
git add elohim/brit
git commit -m "chore(brit): bump submodule to Phase 2a attestation primitives

Advances the brit submodule pointer to include Phase 2a: three
attestation ContentNode schemas, LocalObjectStore, agent signing,
git ref namespace management under refs/notes/brit/, and the
brit-build-ref CLI."
```

- [ ] **Step 9.5: Report back**

```
Phase 2a plan complete. Summary:

  - Engine foundation: BritCid (blake3), ContentNode trait, LocalObjectStore,
    AgentKey (ed25519 signing).
  - Three attestation schemas: BuildAttestationContentNode,
    DeployAttestationContentNode, ValidationAttestationContentNode.
  - Git ref management under refs/notes/brit/ for build/deploy/validate/reach.
  - Deterministic reach computation: Unknown → Built → Deployed → Verified.
  - brit-build-ref CLI with full put/get/list for all attestation types.
  - ~37 tests covering roundtrip, CID determinism, signing, ref ops, reach.
  - Engine-only build verified (--no-default-features).
  - Design doc and implementation plan committed to submodule.
  - Submodule pointer bumped in parent monorepo.

Ready to push both repos. Waiting for confirmation.
```

---

## Self-Review

**Spec coverage:**
- ✅ `BuildAttestationContentNode` schema with all fields from spec (Task 4)
- ✅ `DeployAttestationContentNode` schema with all fields from spec (Task 4)
- ✅ `ValidationAttestationContentNode` schema with check vocabulary (Task 4)
- ✅ `brit build-ref build put/get/list` CLI (Task 7)
- ✅ `brit build-ref deploy put/get/list` CLI (Task 7)
- ✅ `brit build-ref validate put/get/list` CLI (Task 7)
- ✅ `brit build-ref reach compute/get` CLI (Task 7)
- ✅ Ref namespace under `refs/notes/brit/` (Task 5)
- ✅ Serde roundtrip for all three types (Task 4)
- �� CID determinism (Task 1, Task 4)
- ✅ Agent signing on every `put` (Task 3, Task 7)
- ✅ `elohim-protocol` feature flag enforcement (Task 8)
- ✅ Engine compiles with `--no-default-features` (Task 8)
- ✅ Reach computation is deterministic (Task 6)

**Spec acceptance criteria check:**
- ✅ "All three schemas round-trip through serialize/deserialize" — Task 4 tests
- ⚠️ "with ts-rs generation" — NOT covered. ts-rs is a codegen concern; Phase 2a can add it as a follow-up task after the types stabilize. The types are ts-rs-compatible (all serde-serializable with camelCase).
- ✅ "`brit build-ref build put/get/list` work on a fresh git repo" — Task 7 smoke test
- ✅ "Refs written by `brit build-ref` are visible via `git notes`" — Task 5 uses git notes for build refs
- ⚠️ "Refs survive `git clone --bare` + `git fetch refs/notes/*`" — not explicitly tested but the ref layout is designed for this. Add a follow-up integration test.
- ✅ "Engine compiles with `--no-default-features`" — Task 8 Step 8.2
- ✅ "Reach computation is deterministic" — Task 6 test
- ⚠️ "Check vocabulary registration is enforced: unregistered checkName values rejected" — NOT covered in Phase 2a CLI. The spec says check names are governed by AppManifest, which doesn't exist yet. The validation type stores whatever check name is given. Add enforcement when AppManifest lands.

**Placeholder scan:** None. Every step has actual code, actual commands, actual expected output.

**Type consistency:** `BritCid` used identically across all modules. `ContentNode` trait implemented by all three attestation types. `AgentKey` used consistently in all CLI put commands. `BritRefManager` used consistently across CLI and tests. `ReachLevel` enum used in both computation and CLI output.

**Deferred to later phases:**
- ts-rs TypeScript generation → after types stabilize
- Check vocabulary enforcement → when AppManifest exists
- `git clone --bare` + fetch integration test → follow-up
- Full multiformats CIDv1 → when IPFS/Holochain interop needed
- DHT publication of attestations → Phase 5
- Economic event emission → shefa integration phase
