# Phase 0+1: EPR Trailer Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the `brit-epr` crate with an engine/app-schema split and ship the first working EPR primitive — a pillar-trailer parser/validator — such that any git commit carrying `Lamad:`, `Shefa:`, `Qahal:` trailers can be verified by a `brit-verify` binary.

**Architecture:** Single new crate `brit-epr` lives in the gitoxide workspace. It has two internal modules: an **unconditional `engine` module** (trailer parser, generic validator, `AppSchema` dispatch trait, CID types) and a **feature-gated `elohim` module** (the concrete `ElohimProtocolSchema` implementation, pillar trailer types, signal catalog constants). Default features include `elohim-protocol`. Zero modifications to existing `gix-*` crates — pure additive scaffolding. The binary `brit-verify` demonstrates end-to-end use: given a commit SHA in a local repo, parse its trailers, extract pillar fields, and exit 0 if all three pillars are present and well-formed.

**Schema source of truth:** `docs/schemas/elohim-protocol-manifest.md` is the normative reference for trailer shapes, validation rules, and the `AppSchema` trait signature. This plan links to it rather than duplicating spec text. When this plan and the schema doc disagree, the schema doc wins and this plan gets a follow-up edit.

**Tech stack:** Rust 2021, gitoxide's existing `gix-object` crate (for `BodyRef::trailers()` which parses RFC-822-style trailers), `thiserror` for error types. No `winnow`, `clap`, `serde_json`, or `cid` crate dependencies yet — those come in Phase 2+.

**Upstream compatibility:** Every commit a brit user creates round-trips through stock git. Pillar trailers follow RFC-822 "Key: value" syntax, indistinguishable from `Signed-off-by:` to any reader that doesn't know about them.

## Scope — what's IN Phase 1

- `brit-epr` crate in the workspace with the engine-vs-schema boundary established from Task 0.
- The unconditional `engine` module with:
  - `AppSchema` trait (the dispatch contract from schema doc §2.3).
  - `TrailerSet` — ordered, duplicate-aware map of `(key, value)` pairs preserving roundtrip order.
  - `TrailerBlock` parser — given a commit body, locate and extract the trailer block via `gix_object::commit::message::BodyRef::trailers()`.
  - `ValidationError` and engine error types.
- The feature-gated `elohim` module (behind `#[cfg(feature = "elohim-protocol")]`, default on) with:
  - `ElohimProtocolSchema` — the `AppSchema` implementor.
  - `PillarTrailers` — strongly-typed view over the six trailer keys (three canonical summaries, three linked-node CID slots).
  - `parse_pillar_trailers(body)` convenience function.
  - `validate_pillar_trailers(&PillarTrailers)` convenience function.
- `brit-verify` binary that opens a repo, resolves a commit rev, runs the elohim schema's parser + validator, and exits 0/1.
- Fixtures: happy-path commit body, missing-pillar body, malformed-node-ref body.
- Submodule pointer bump in the parent `elohim` monorepo.

## Scope — what's deliberately OUT of Phase 1

These are excluded not because they're unimportant but because they live in later phases and including them now would bloat the first commit cycle:

- **`MergeProposalContentNode`** and the async merge consent flow. Out. That's Phase 2+ and requires the parent-EPR governance adapter. See schema doc §5.13 and §14.1 #4.
- **Reach awareness** in `BranchContentNode`. Out. Phase 1 parses commit trailers only; branches come in Phase 2. The vendored `schemas/elohim-protocol/v1/enums/reach.schema.json` is already in the tree for future use.
- **ContentNode adapter** — no code that registers brit repos as ContentNodes in any external store. Out.
- **libp2p transport** (`/brit/fetch/1.0.0`). Out — Phase 3.
- **CID resolution** — the parser recognizes `Lamad-Node:`, `Shefa-Node:`, `Qahal-Node:` as CID-bearing trailer keys and stores the raw string, but does NOT parse it into a typed `Cid`, does NOT resolve it, does NOT check the target type. Phase 2.
- **JSON Schema codegen pipeline**. Phase 1 hand-writes types in Rust. Phase 2 introduces the codegen from `schemas/elohim-protocol/v1/*.schema.json` files when the ContentNode adapter work starts.
- **Signals emitted** (§9 catalog). Phase 1 doesn't emit any — it only parses and validates. Phase 2+ adds signal emission once there's something to emit them to.
- **`brit-cli` full binary**. Phase 1 ships only the minimum `brit-verify` example binary. The full brit subcommand surface from schema doc §3 is Phase 3+.

## File structure

```
brit/
├── brit-epr/
│   ├── Cargo.toml                    # new crate, member of workspace
│   ├── src/
│   │   ├── lib.rs                    # crate root, re-exports, feature-gated pub use
│   │   ├── engine/
│   │   │   ├── mod.rs                # module exports
│   │   │   ├── app_schema.rs         # AppSchema trait (the dispatch contract)
│   │   │   ├── trailer_set.rs        # TrailerSet type
│   │   │   ├── trailer_block.rs      # TrailerBlock parser — wraps gix-object
│   │   │   └── error.rs              # ValidationError, EngineError
│   │   └── elohim/
│   │       ├── mod.rs                # #[cfg(feature = "elohim-protocol")]
│   │       ├── schema.rs             # ElohimProtocolSchema (impl AppSchema)
│   │       ├── pillar_trailers.rs    # PillarTrailers strong type, TrailerKey enum
│   │       ├── parse.rs              # parse_pillar_trailers
│   │       └── validate.rs           # validate_pillar_trailers
│   └── tests/
│       ├── engine_parsing.rs         # engine-level trailer block extraction
│       ├── elohim_parse.rs           # pillar trailer parsing (gated on feature)
│       ├── elohim_validate.rs        # pillar validation (gated on feature)
│       └── fixtures/
│           ├── happy_all_three_pillars.txt
│           ├── missing_qahal.txt
│           └── malformed_shefa_node.txt
├── brit-verify/
│   ├── Cargo.toml                    # new binary crate
│   └── src/
│       └── main.rs                   # CLI: brit-verify <commit-rev> [--repo <path>]
└── Cargo.toml                        # modified: add workspace members
```

**Responsibilities per file:**

- `brit-epr/src/engine/app_schema.rs` — the `AppSchema` trait. Engine only knows the contract, never which schema is plugged in.
- `brit-epr/src/engine/trailer_set.rs` — `TrailerSet` is a `Vec<(String, String)>`-backed structure preserving insertion order, with `get`, `get_all` (for repeatable keys), `iter`, and `Display` producing the canonical RFC-822 representation.
- `brit-epr/src/engine/trailer_block.rs` — one public function `parse_trailer_block(body: &[u8]) -> TrailerSet`. Uses `gix_object::commit::message::BodyRef::from_bytes` and `.trailers()`. No schema-specific knowledge.
- `brit-epr/src/engine/error.rs` — `EngineError`, `ValidationError` via `thiserror`.
- `brit-epr/src/elohim/schema.rs` — `ElohimProtocolSchema` zero-sized struct implementing `AppSchema`. The implementation names the six trailer keys, declares required keys, routes validation to the pair/set checkers.
- `brit-epr/src/elohim/pillar_trailers.rs` — `PillarTrailers` struct with `lamad/shefa/qahal` summary fields and `lamad_node/shefa_node/qahal_node` raw-CID-string fields. `TrailerKey` enum with `summary_token()` and `node_token()` accessors.
- `brit-epr/src/elohim/parse.rs` — `parse_pillar_trailers(body: &[u8]) -> PillarTrailers` convenience function that calls the engine's trailer-block parser and projects into the typed view.
- `brit-epr/src/elohim/validate.rs` — `validate_pillar_trailers(&PillarTrailers) -> Result<(), PillarValidationError>`. Structural validation only: all three summary trailers present and non-empty. No CID resolution, no cross-referential checks.
- `brit-epr/src/lib.rs` — re-exports `engine::*` unconditionally; re-exports `elohim::*` behind `#[cfg(feature = "elohim-protocol")]`.
- `brit-verify/src/main.rs` — minimal CLI (`std::env::args`, no clap), opens repo via `gix::discover`, reads commit, projects to body, calls `elohim::parse_pillar_trailers` + `elohim::validate_pillar_trailers`, prints summary + exits 0/1.

---

## Task 0: Scaffolding — add `brit-epr` crate with engine/elohim split

**Files:**
- Create: `brit-epr/Cargo.toml`
- Create: `brit-epr/src/lib.rs`
- Create: `brit-epr/src/engine/mod.rs`
- Create: `brit-epr/src/elohim/mod.rs`
- Modify: `Cargo.toml` (root — add to workspace members)

- [ ] **Step 0.1: Create the crate manifest**

Create `brit-epr/Cargo.toml`:

```toml
lints.workspace = true

[package]
name = "brit-epr"
version = "0.0.0"
description = "Elohim Protocol primitives (pillar trailers, dispatch trait, validation) for brit — an expansion of gitoxide with covenant semantics"
repository = "https://github.com/ethosengine/brit"
authors = ["Matthew Dowell <matthew@ethosengine.com>"]
license = "MIT OR Apache-2.0"
edition = "2021"
rust-version = "1.82"

[lib]
doctest = false

[features]
default = ["elohim-protocol"]
# Gates the elohim module — brit's first-party app schema implementation.
# With this feature off, brit-epr is the covenant engine alone: trailer
# parsing, the AppSchema dispatch trait, error types. No pillar-specific
# behavior. A downstream fork can disable this feature and ship their own
# app schema crate.
elohim-protocol = []

[dependencies]
gix-object = { version = "^0.52.0", path = "../gix-object" }
thiserror = "2.0"
```

> **Note:** Version `^0.52.0` is illustrative. Read the actual value in `gix-object/Cargo.toml` in this workspace checkout and use the current major.minor.

- [ ] **Step 0.2: Create the lib.rs crate root**

Create `brit-epr/src/lib.rs`:

```rust
//! Elohim Protocol primitives for brit.
//!
//! `brit-epr` has two layers:
//!
//! - **`engine`** — unconditional. The covenant engine: trailer parser,
//!   `AppSchema` dispatch trait, `TrailerSet`, validation errors. Does not know
//!   which schema is plugged in. A downstream fork can disable the default
//!   feature and ship its own app schema on this engine.
//! - **`elohim`** — feature-gated behind `elohim-protocol` (default on). The
//!   first-party Elohim Protocol app schema: pillar trailer types (Lamad,
//!   Shefa, Qahal), the concrete `ElohimProtocolSchema` implementor, parse
//!   and validate convenience functions.
//!
//! The normative specification for the trailer format, pillar meanings, and
//! validation rules lives in `docs/schemas/elohim-protocol-manifest.md` at
//! the root of the brit repository. When this crate and the schema doc
//! disagree, the schema doc wins.

#![deny(missing_docs, rust_2018_idioms)]
#![forbid(unsafe_code)]

pub mod engine;

#[cfg(feature = "elohim-protocol")]
pub mod elohim;

// Unconditional re-exports
pub use engine::{AppSchema, TrailerSet, ValidationError};

// Feature-gated re-exports
#[cfg(feature = "elohim-protocol")]
pub use elohim::{
    parse_pillar_trailers, validate_pillar_trailers, ElohimProtocolSchema, PillarTrailers,
    PillarValidationError, TrailerKey,
};
```

- [ ] **Step 0.3: Create the engine module stub**

Create `brit-epr/src/engine/mod.rs`:

```rust
//! Covenant engine — unconditional layer that knows the trailer format and
//! dispatch contract but not any specific schema vocabulary.

mod app_schema;
mod error;
mod trailer_block;
mod trailer_set;

pub use app_schema::AppSchema;
pub use error::{EngineError, ValidationError};
pub use trailer_block::parse_trailer_block;
pub use trailer_set::TrailerSet;
```

- [ ] **Step 0.4: Create the elohim module stub**

Create `brit-epr/src/elohim/mod.rs`:

```rust
//! Elohim Protocol app schema — first-party `AppSchema` implementation.
//!
//! Gated behind `#[cfg(feature = "elohim-protocol")]`. With this feature
//! disabled, `brit-epr` ships only the engine.

mod parse;
mod pillar_trailers;
mod schema;
mod validate;

pub use parse::parse_pillar_trailers;
pub use pillar_trailers::{PillarTrailers, TrailerKey};
pub use schema::ElohimProtocolSchema;
pub use validate::{validate_pillar_trailers, PillarValidationError};
```

- [ ] **Step 0.5: Add to workspace members**

Edit root `Cargo.toml`. Find the `members = [` list and add `"brit-epr"` as the last entry before the closing `]`:

```toml
# ... existing gix-* members ...
    "gix-shallow",
    "brit-epr",
]
```

- [ ] **Step 0.6: Verify workspace builds refuse to compile with missing modules**

Run:

```
cargo build -p brit-epr
```

Expected: compile error. The module files referenced in Steps 0.3 and 0.4 don't exist yet (`app_schema.rs`, `error.rs`, etc.). This is expected — Task 1 creates them. If the build somehow passes, go back and verify the `mod` declarations in Steps 0.3 / 0.4 are present.

- [ ] **Step 0.7: Commit**

```
git add brit-epr/Cargo.toml brit-epr/src/lib.rs brit-epr/src/engine/mod.rs brit-epr/src/elohim/mod.rs Cargo.toml
git commit -m "feat(brit-epr): scaffold crate with engine/elohim feature split

Establishes the engine-vs-app-schema boundary from day 0. The engine
module is unconditional; the elohim module is gated behind the
elohim-protocol cargo feature (default on). Subsequent tasks land the
trait, types, parser, and validator."
```

---

## Task 1: Engine — define `AppSchema` trait, `TrailerSet`, error types

**Files:**
- Create: `brit-epr/src/engine/error.rs`
- Create: `brit-epr/src/engine/app_schema.rs`
- Create: `brit-epr/src/engine/trailer_set.rs`

- [ ] **Step 1.1: Create `engine/error.rs`**

Create `brit-epr/src/engine/error.rs`:

```rust
//! Engine-level error types.

use thiserror::Error;

/// Errors raised by the covenant engine's generic layer.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Unable to extract a trailer block from a commit body.
    #[error("failed to parse trailer block: {0}")]
    TrailerBlockParse(String),
}

/// Errors emitted by schema validation. App schemas return this type from
/// `AppSchema::validate_pair` and `AppSchema::validate_set`.
///
/// Variants are intentionally broad because different app schemas will
/// express different failure modes. A richer error type can layer on top.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    /// A required trailer key was absent from the set.
    #[error("required trailer key missing: {0}")]
    MissingKey(String),

    /// A trailer value is present but empty or whitespace-only.
    #[error("trailer key {0} has empty value")]
    EmptyValue(String),

    /// A trailer value failed a format check (e.g., malformed CID).
    #[error("trailer key {0} malformed: {1}")]
    MalformedValue(String, String),

    /// Cross-field rule violated.
    #[error("trailer set failed cross-field rule: {0}")]
    CrossFieldRule(String),
}
```

- [ ] **Step 1.2: Create `engine/trailer_set.rs`**

Create `brit-epr/src/engine/trailer_set.rs`:

```rust
//! `TrailerSet` — ordered, duplicate-aware key/value pairs from a commit
//! trailer block. Preserves insertion order for roundtrip-compatible
//! rendering.

use std::fmt;

/// A commit trailer block, parsed into ordered key/value pairs.
///
/// Order is preserved because the engine must be able to re-render the
/// trailer block byte-identically for signing and round-trip use cases.
/// Duplicate keys are allowed (e.g., multiple `Signed-off-by:` or
/// repeatable app-schema keys like `Built-By:`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrailerSet {
    entries: Vec<(String, String)>,
}

impl TrailerSet {
    /// Create an empty set.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Append a trailer entry, preserving insertion order.
    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.push((key.into(), value.into()));
    }

    /// Return the first value for a given key, or `None` if absent.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Return all values for a given key (preserves order).
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// Iterate over all `(key, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for TrailerSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (k, v) in &self.entries {
            writeln!(f, "{k}: {v}")?;
        }
        Ok(())
    }
}
```

- [ ] **Step 1.3: Create `engine/app_schema.rs`**

Create `brit-epr/src/engine/app_schema.rs`:

```rust
//! `AppSchema` — the dispatch contract between the covenant engine and
//! specific app schemas (e.g., `elohim-protocol`).
//!
//! The normative specification is in `docs/schemas/elohim-protocol-manifest.md`
//! §2.3. This file is the Rust projection of that contract.

use crate::engine::{TrailerSet, ValidationError};

/// Dispatch contract that app schemas implement.
///
/// The engine consumes an `impl AppSchema` to do validation and rendering
/// without knowing the specific vocabulary (Lamad / Shefa / Qahal, or any
/// other app's keys). This is what keeps the engine/app-schema boundary
/// legible — see `elohim-protocol-manifest.md` §11.7 for boundary smells
/// that indicate the boundary is drifting.
pub trait AppSchema {
    /// Stable identifier for this schema, e.g. `"elohim-protocol/1.0.0"`.
    fn id(&self) -> &'static str;

    /// Does this schema recognize this trailer key?
    fn owns_key(&self, key: &str) -> bool;

    /// Required keys. Engine uses this to short-circuit validation when the
    /// commit message is missing the required surface entirely.
    fn required_keys(&self) -> &'static [&'static str];

    /// Which keys carry CID references? The resolver walks these in later
    /// phases. Phase 1 just records the list.
    fn cid_bearing_keys(&self) -> &'static [&'static str];

    /// Validate one `(key, value)` pair in isolation (no cross-field rules).
    fn validate_pair(&self, key: &str, value: &str) -> Result<(), ValidationError>;

    /// Validate the whole trailer set together (cross-field rules, e.g.
    /// "`Lamad-Node:` present requires `Lamad:` non-empty").
    fn validate_set(&self, trailers: &TrailerSet) -> Result<(), ValidationError>;
}
```

- [ ] **Step 1.4: Build-check the engine module**

Run:

```
cargo build -p brit-epr --no-default-features
```

Expected: compiles with warnings (unused imports on `trailer_block` which is stubbed but not yet created). If the build fails with "file not found for module trailer_block", stub it now by creating `brit-epr/src/engine/trailer_block.rs` containing:

```rust
//! Stubbed; Task 2 implements this.
use crate::engine::TrailerSet;

/// Parse a commit body into a `TrailerSet`. Stub — Task 2 replaces this.
pub fn parse_trailer_block(_body: &[u8]) -> TrailerSet {
    TrailerSet::new()
}
```

Then retry the build. The `--no-default-features` flag proves the engine compiles without the elohim module — this is the boundary check.

- [ ] **Step 1.5: Commit**

```
git add brit-epr/src/engine/
git commit -m "feat(brit-epr/engine): add AppSchema trait, TrailerSet, errors

Engine layer is now independently compilable with --no-default-features.
Proves the engine/app-schema boundary holds from day 0: the engine
knows nothing about Lamad/Shefa/Qahal specifically."
```

---

## Task 2: Engine — implement `parse_trailer_block` using `gix-object`

**Files:**
- Modify: `brit-epr/src/engine/trailer_block.rs` (replace stub)
- Create: `brit-epr/tests/engine_parsing.rs`

- [ ] **Step 2.1: Write the failing engine-level test**

Create `brit-epr/tests/engine_parsing.rs`:

```rust
//! Engine-level tests — trailer block extraction, no app-schema semantics.

use brit_epr::engine::{parse_trailer_block, TrailerSet};

#[test]
fn extracts_trailer_block_from_commit_body() {
    let body = b"\
Add pillar trailer parser

Wires gix-object into the covenant engine so trailer blocks can be
extracted into a schema-agnostic TrailerSet.

Signed-off-by: Matthew Dowell <matthew@ethosengine.com>
Lamad: introduces pillar trailer model
Shefa: stewardship by @matthew
Qahal: no governance review required
";

    let trailers: TrailerSet = parse_trailer_block(body);

    assert_eq!(trailers.len(), 4, "expected 4 trailers, got {}", trailers.len());
    assert_eq!(trailers.get("Signed-off-by"), Some("Matthew Dowell <matthew@ethosengine.com>"));
    assert_eq!(trailers.get("Lamad"), Some("introduces pillar trailer model"));
    assert_eq!(trailers.get("Shefa"), Some("stewardship by @matthew"));
    assert_eq!(trailers.get("Qahal"), Some("no governance review required"));
}

#[test]
fn empty_trailer_block_returns_empty_set() {
    let body = b"Commit with no trailers at all, just a body.";
    let trailers = parse_trailer_block(body);
    assert_eq!(trailers.len(), 0);
}
```

- [ ] **Step 2.2: Run the tests — expect failure**

Run:

```
cargo test -p brit-epr --test engine_parsing
```

Expected: one or both tests fail because the stub from Step 1.4 returns an empty `TrailerSet`. If you see "cannot find function `parse_trailer_block`", you may have forgotten to `pub use` it from `engine/mod.rs` — check Task 0 Step 0.3.

- [ ] **Step 2.3: Implement the real parser**

Replace `brit-epr/src/engine/trailer_block.rs` with:

```rust
//! `parse_trailer_block` — extract a commit's RFC-822-style trailer block
//! into a `TrailerSet`. Wraps `gix_object::commit::message::BodyRef::trailers()`.

use gix_object::commit::message::BodyRef;

use crate::engine::TrailerSet;

/// Parse a commit body's bytes into a `TrailerSet`.
///
/// The body is the message *after* the commit headers (author, committer,
/// tree, parent lines) — i.e., what gitoxide calls "the body" of a commit.
/// This function extracts the final trailing block of `Key: value` lines
/// (if any) and records each as an entry in a `TrailerSet`, preserving
/// insertion order.
///
/// Returns an empty `TrailerSet` if the body has no trailer block.
pub fn parse_trailer_block(body: &[u8]) -> TrailerSet {
    let body_ref = BodyRef::from_bytes(body);
    let mut set = TrailerSet::new();

    for trailer in body_ref.trailers() {
        // BStr → String via to_str_lossy.into_owned. Safe because commit
        // messages are conventionally UTF-8 and the lossy conversion
        // preserves whatever bytes we got.
        let key = trailer.token.to_str_lossy().into_owned();
        let value = trailer.value.to_str_lossy().into_owned();
        set.push(key, value);
    }

    set
}
```

- [ ] **Step 2.4: Run the tests — expect pass**

Run:

```
cargo test -p brit-epr --test engine_parsing
```

Expected: both tests pass. If `to_str_lossy` doesn't exist, the correct method on `bstr::BStr` in the vendored gitoxide version may be `to_str_lossy().to_string()` or similar — grep `gix-object/src/commit/message/body.rs` for `to_str_lossy` or `to_string` usage to confirm the idiom used in this workspace.

- [ ] **Step 2.5: Commit**

```
git add brit-epr/src/engine/trailer_block.rs brit-epr/tests/engine_parsing.rs
git commit -m "feat(brit-epr/engine): implement parse_trailer_block via gix-object

Wraps gix_object::commit::message::BodyRef::trailers() into a
schema-agnostic TrailerSet. Engine-level tests prove extraction
works for happy path and no-trailers case."
```

---

## Task 3: Elohim — `PillarTrailers`, `TrailerKey`, `ElohimProtocolSchema`

**Files:**
- Create: `brit-epr/src/elohim/pillar_trailers.rs`
- Create: `brit-epr/src/elohim/schema.rs`

- [ ] **Step 3.1: Create `pillar_trailers.rs`**

Create `brit-epr/src/elohim/pillar_trailers.rs`:

```rust
//! Pillar trailer types — the strongly-typed view the elohim app schema
//! uses to represent the three pillars plus their linked-node CID slots.

/// Which of the three pillars a trailer belongs to.
///
/// The elohim protocol pillars:
///
/// - **Lamad** (לָמַד, "to learn") — knowledge positioning.
/// - **Shefa** (שֶׁפַע, "abundance") — economic positioning.
/// - **Qahal** (קָהָל, "assembly") — governance positioning.
///
/// Each pillar has two trailer forms: a canonical summary (e.g., `Lamad:`)
/// and a linked-node CID reference (e.g., `Lamad-Node:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrailerKey {
    /// Knowledge-layer trailer.
    Lamad,
    /// Economic-layer trailer.
    Shefa,
    /// Governance-layer trailer.
    Qahal,
}

impl TrailerKey {
    /// The RFC-822 token name for the canonical-summary trailer.
    pub fn summary_token(self) -> &'static str {
        match self {
            TrailerKey::Lamad => "Lamad",
            TrailerKey::Shefa => "Shefa",
            TrailerKey::Qahal => "Qahal",
        }
    }

    /// The RFC-822 token name for the linked-node CID trailer.
    pub fn node_token(self) -> &'static str {
        match self {
            TrailerKey::Lamad => "Lamad-Node",
            TrailerKey::Shefa => "Shefa-Node",
            TrailerKey::Qahal => "Qahal-Node",
        }
    }

    /// All three pillars, in canonical order.
    pub fn all() -> [TrailerKey; 3] {
        [TrailerKey::Lamad, TrailerKey::Shefa, TrailerKey::Qahal]
    }
}

/// Pillar trailers extracted from a commit body and projected into the
/// typed view the elohim app schema uses.
///
/// Each `*_node` field holds the raw CID *string* — Phase 1 does not parse
/// the string into a typed `Cid`, does not resolve it, and does not check
/// the target's type. The parser is permissive; strict CID validation and
/// resolution arrive in Phase 2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PillarTrailers {
    /// Canonical summary value of the `Lamad:` trailer, trimmed.
    pub lamad: Option<String>,
    /// Canonical summary value of the `Shefa:` trailer, trimmed.
    pub shefa: Option<String>,
    /// Canonical summary value of the `Qahal:` trailer, trimmed.
    pub qahal: Option<String>,

    /// Raw CID string from a `Lamad-Node:` trailer, if present. Phase 1
    /// does not parse or resolve this.
    pub lamad_node: Option<String>,
    /// Raw CID string from a `Shefa-Node:` trailer, if present.
    pub shefa_node: Option<String>,
    /// Raw CID string from a `Qahal-Node:` trailer, if present.
    pub qahal_node: Option<String>,
}
```

- [ ] **Step 3.2: Create `schema.rs`**

Create `brit-epr/src/elohim/schema.rs`:

```rust
//! `ElohimProtocolSchema` — the first-party `AppSchema` implementation.

use crate::elohim::pillar_trailers::TrailerKey;
use crate::engine::{AppSchema, TrailerSet, ValidationError};

/// Zero-sized implementor of [`AppSchema`] for the Elohim Protocol.
///
/// Instances are stateless. Typically you construct one like
/// `const SCHEMA: ElohimProtocolSchema = ElohimProtocolSchema;` and pass
/// by reference.
#[derive(Debug, Clone, Copy, Default)]
pub struct ElohimProtocolSchema;

const SUMMARY_KEYS: &[&str] = &["Lamad", "Shefa", "Qahal"];
const NODE_KEYS: &[&str] = &["Lamad-Node", "Shefa-Node", "Qahal-Node"];

impl AppSchema for ElohimProtocolSchema {
    fn id(&self) -> &'static str {
        "elohim-protocol/1.0.0"
    }

    fn owns_key(&self, key: &str) -> bool {
        SUMMARY_KEYS.contains(&key) || NODE_KEYS.contains(&key)
    }

    fn required_keys(&self) -> &'static [&'static str] {
        SUMMARY_KEYS
    }

    fn cid_bearing_keys(&self) -> &'static [&'static str] {
        NODE_KEYS
    }

    fn validate_pair(&self, key: &str, value: &str) -> Result<(), ValidationError> {
        if !self.owns_key(key) {
            return Ok(()); // not our key; ignore
        }
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyValue(key.to_string()));
        }
        // Phase 1: no additional format checks. Phase 2 adds CID parsing on
        // NODE_KEYS.
        Ok(())
    }

    fn validate_set(&self, trailers: &TrailerSet) -> Result<(), ValidationError> {
        // Check required keys are present in canonical order so the error
        // always names Lamad before Shefa before Qahal.
        for key in TrailerKey::all() {
            let summary = key.summary_token();
            match trailers.get(summary) {
                None => return Err(ValidationError::MissingKey(summary.to_string())),
                Some(v) if v.trim().is_empty() => {
                    return Err(ValidationError::EmptyValue(summary.to_string()))
                }
                Some(_) => {}
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 3.3: Build-check**

Run:

```
cargo build -p brit-epr
```

Expected: compiles with default features (the elohim module is enabled). If `parse` or `validate` module files are missing, create stub files:

```rust
// brit-epr/src/elohim/parse.rs
//! Stubbed; Task 4 implements.
use super::PillarTrailers;
pub fn parse_pillar_trailers(_body: &[u8]) -> PillarTrailers {
    PillarTrailers::default()
}
```

```rust
// brit-epr/src/elohim/validate.rs
//! Stubbed; Task 5 implements.
use super::PillarTrailers;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PillarValidationError {
    #[error("stub")]
    Stub,
}

pub fn validate_pillar_trailers(_trailers: &PillarTrailers) -> Result<(), PillarValidationError> {
    Ok(())
}
```

Retry build. Pass.

- [ ] **Step 3.4: Commit**

```
git add brit-epr/src/elohim/
git commit -m "feat(brit-epr/elohim): add PillarTrailers, TrailerKey, schema impl

ElohimProtocolSchema implements AppSchema with closed vocabulary
(Lamad/Shefa/Qahal summary keys + their -Node CID counterparts).
Phase 1 stores raw CID strings without parsing — CID resolution
arrives in Phase 2."
```

---

## Task 4: Elohim — `parse_pillar_trailers` (TDD)

**Files:**
- Modify: `brit-epr/src/elohim/parse.rs` (replace stub)
- Create: `brit-epr/tests/elohim_parse.rs`
- Create: `brit-epr/tests/fixtures/happy_all_three_pillars.txt`
- Create: `brit-epr/tests/fixtures/missing_qahal.txt`
- Create: `brit-epr/tests/fixtures/malformed_shefa_node.txt`

- [ ] **Step 4.1: Write the first failing test — happy path**

Create `brit-epr/tests/fixtures/happy_all_three_pillars.txt`:

```
Add pillar trailer parser

Wires gix-object::BodyRef::trailers() into the brit-epr engine so
commit messages can carry Lamad / Shefa / Qahal values natively.

Signed-off-by: Matthew Dowell <matthew@ethosengine.com>
Lamad: introduces pillar trailer model; first testable EPR primitive
Shefa: stewardship by @matthew; contributor credit via git author
Qahal: no governance review required for scaffolding
```

Create `brit-epr/tests/elohim_parse.rs`:

```rust
//! Integration tests for elohim pillar trailer parsing.

use brit_epr::{parse_pillar_trailers, PillarTrailers};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/{}", name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"))
}

#[test]
fn happy_path_all_three_pillars_parse() {
    let body = fixture("happy_all_three_pillars.txt");
    let trailers: PillarTrailers = parse_pillar_trailers(&body);

    assert_eq!(
        trailers.lamad.as_deref(),
        Some("introduces pillar trailer model; first testable EPR primitive")
    );
    assert_eq!(
        trailers.shefa.as_deref(),
        Some("stewardship by @matthew; contributor credit via git author")
    );
    assert_eq!(
        trailers.qahal.as_deref(),
        Some("no governance review required for scaffolding")
    );
    assert_eq!(trailers.lamad_node, None);
    assert_eq!(trailers.shefa_node, None);
    assert_eq!(trailers.qahal_node, None);
}
```

- [ ] **Step 4.2: Run the test — expect failure**

Run:

```
cargo test -p brit-epr --test elohim_parse happy_path_all_three_pillars_parse
```

Expected: fails because the stub from Task 3 returns `PillarTrailers::default()`. All assertions about `lamad/shefa/qahal` having `Some(...)` values fail.

- [ ] **Step 4.3: Implement the parser**

Replace `brit-epr/src/elohim/parse.rs` with:

```rust
//! `parse_pillar_trailers` — convenience function that projects a
//! `TrailerSet` into the strongly-typed `PillarTrailers` view.

use crate::elohim::pillar_trailers::{PillarTrailers, TrailerKey};
use crate::engine::parse_trailer_block;

/// Parse pillar trailers from a commit body.
///
/// Pure function: no I/O beyond reading the body slice. Unknown trailers
/// (anything outside the six reserved pillar keys) are silently skipped —
/// a commit may carry `Signed-off-by:`, `Co-Authored-By:`, etc., alongside
/// the pillar trailers.
///
/// Permissive: malformed values in `*_Node:` trailers are accepted as raw
/// strings. Strict validation is done by `validate_pillar_trailers`.
pub fn parse_pillar_trailers(body: &[u8]) -> PillarTrailers {
    let set = parse_trailer_block(body);
    let mut out = PillarTrailers::default();

    for (key, value) in set.iter() {
        for pillar in TrailerKey::all() {
            if key == pillar.summary_token() {
                match pillar {
                    TrailerKey::Lamad => out.lamad = Some(value.to_string()),
                    TrailerKey::Shefa => out.shefa = Some(value.to_string()),
                    TrailerKey::Qahal => out.qahal = Some(value.to_string()),
                }
            } else if key == pillar.node_token() {
                match pillar {
                    TrailerKey::Lamad => out.lamad_node = Some(value.to_string()),
                    TrailerKey::Shefa => out.shefa_node = Some(value.to_string()),
                    TrailerKey::Qahal => out.qahal_node = Some(value.to_string()),
                }
            }
        }
    }

    out
}
```

- [ ] **Step 4.4: Run the test — expect pass**

Run:

```
cargo test -p brit-epr --test elohim_parse happy_path_all_three_pillars_parse
```

Expected: pass.

- [ ] **Step 4.5: Add the partial-pillars test**

Create `brit-epr/tests/fixtures/missing_qahal.txt`:

```
Routine refactor with only two pillars declared

Lamad: no knowledge change — pure refactor
Shefa: no value flow — maintenance work
```

Append to `brit-epr/tests/elohim_parse.rs`:

```rust
#[test]
fn missing_qahal_parses_partially() {
    let body = fixture("missing_qahal.txt");
    let trailers = parse_pillar_trailers(&body);

    assert_eq!(trailers.lamad.as_deref(), Some("no knowledge change — pure refactor"));
    assert_eq!(trailers.shefa.as_deref(), Some("no value flow — maintenance work"));
    assert_eq!(trailers.qahal, None);
}
```

- [ ] **Step 4.6: Add the malformed-node test**

Create `brit-epr/tests/fixtures/malformed_shefa_node.txt`:

```
Test permissive parser behavior for malformed node ref

Lamad: teaches the permissive parser behavior
Shefa: value summary is fine
Shefa-Node: not-a-valid-cid-at-all
Qahal: governance review complete
```

Append to `brit-epr/tests/elohim_parse.rs`:

```rust
#[test]
fn malformed_shefa_node_stored_as_raw_string() {
    let body = fixture("malformed_shefa_node.txt");
    let trailers = parse_pillar_trailers(&body);

    assert_eq!(trailers.lamad.as_deref(), Some("teaches the permissive parser behavior"));
    assert_eq!(trailers.shefa.as_deref(), Some("value summary is fine"));
    assert_eq!(trailers.qahal.as_deref(), Some("governance review complete"));

    // Phase 1 is permissive — stores raw string without parsing.
    // Phase 2 will add typed CID parsing and reject malformed values.
    assert_eq!(trailers.shefa_node.as_deref(), Some("not-a-valid-cid-at-all"));
}
```

- [ ] **Step 4.7: Run all elohim_parse tests**

Run:

```
cargo test -p brit-epr --test elohim_parse
```

Expected: 3 tests pass.

- [ ] **Step 4.8: Commit**

```
git add brit-epr/src/elohim/parse.rs brit-epr/tests/elohim_parse.rs brit-epr/tests/fixtures/
git commit -m "feat(brit-epr/elohim): implement parse_pillar_trailers

Projects engine's schema-agnostic TrailerSet into the typed
PillarTrailers view. Permissive: unknown trailers skipped, malformed
node refs stored as raw strings. Three fixtures cover happy path,
partial declaration, and malformed node-ref."
```

---

## Task 5: Elohim — `validate_pillar_trailers` (TDD)

**Files:**
- Modify: `brit-epr/src/elohim/validate.rs` (replace stub)
- Create: `brit-epr/tests/elohim_validate.rs`

- [ ] **Step 5.1: Write the first failing test**

Create `brit-epr/tests/elohim_validate.rs`:

```rust
//! Integration tests for elohim pillar structural validation.

use brit_epr::{validate_pillar_trailers, PillarTrailers, PillarValidationError, TrailerKey};

fn complete() -> PillarTrailers {
    PillarTrailers {
        lamad: Some("knowledge summary".into()),
        shefa: Some("economic summary".into()),
        qahal: Some("governance summary".into()),
        lamad_node: None,
        shefa_node: None,
        qahal_node: None,
    }
}

#[test]
fn all_three_present_validates_ok() {
    assert_eq!(validate_pillar_trailers(&complete()), Ok(()));
}

#[test]
fn missing_lamad_fails_with_missing_key() {
    let mut t = complete();
    t.lamad = None;
    assert_eq!(
        validate_pillar_trailers(&t),
        Err(PillarValidationError::MissingPillar(TrailerKey::Lamad))
    );
}

#[test]
fn empty_shefa_fails_with_empty_value() {
    let mut t = complete();
    t.shefa = Some("   ".into());
    assert_eq!(
        validate_pillar_trailers(&t),
        Err(PillarValidationError::EmptyPillar(TrailerKey::Shefa))
    );
}

#[test]
fn returns_first_error_in_canonical_order() {
    let t = PillarTrailers {
        lamad: None,
        shefa: Some("ok".into()),
        qahal: None,
        ..Default::default()
    };
    assert_eq!(
        validate_pillar_trailers(&t),
        Err(PillarValidationError::MissingPillar(TrailerKey::Lamad))
    );
}
```

- [ ] **Step 5.2: Run — expect failure**

Run:

```
cargo test -p brit-epr --test elohim_validate
```

Expected: compilation errors for `PillarValidationError::MissingPillar` and `::EmptyPillar` because the stub used `Stub`.

- [ ] **Step 5.3: Implement the validator**

Replace `brit-epr/src/elohim/validate.rs` with:

```rust
//! Structural validation for pillar trailers.
//!
//! Checks that each pillar has a non-empty summary value. Does NOT resolve
//! linked-node CIDs, does NOT traverse the ContentNode graph, does NOT
//! enforce domain rules — those live in higher layers (Phase 2+).

use thiserror::Error;

use crate::elohim::pillar_trailers::{PillarTrailers, TrailerKey};

/// Structural validation errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PillarValidationError {
    /// Required pillar summary trailer is missing.
    #[error("required pillar trailer missing: {0:?}")]
    MissingPillar(TrailerKey),

    /// Pillar summary trailer is present but empty after trimming.
    #[error("pillar trailer {0:?} is present but value is empty")]
    EmptyPillar(TrailerKey),
}

/// Structurally validate a `PillarTrailers` view.
///
/// Returns `Ok(())` if all three summary trailers are present and non-empty.
/// Returns the first error in canonical order (Lamad → Shefa → Qahal).
///
/// Linked-node CID strings are ignored by this validator — Phase 1 does
/// not enforce their format or resolvability.
pub fn validate_pillar_trailers(t: &PillarTrailers) -> Result<(), PillarValidationError> {
    for pillar in TrailerKey::all() {
        let summary = match pillar {
            TrailerKey::Lamad => t.lamad.as_deref(),
            TrailerKey::Shefa => t.shefa.as_deref(),
            TrailerKey::Qahal => t.qahal.as_deref(),
        };
        match summary {
            None => return Err(PillarValidationError::MissingPillar(pillar)),
            Some(v) if v.trim().is_empty() => {
                return Err(PillarValidationError::EmptyPillar(pillar))
            }
            Some(_) => {}
        }
    }
    Ok(())
}
```

- [ ] **Step 5.4: Run all tests**

Run:

```
cargo test -p brit-epr
```

Expected: all tests pass (engine_parsing: 2, elohim_parse: 3, elohim_validate: 4 → 9 total).

- [ ] **Step 5.5: Verify engine-only build still works**

Run:

```
cargo build -p brit-epr --no-default-features
```

Expected: compiles. This proves the engine/app-schema boundary still holds after all the elohim code landed.

- [ ] **Step 5.6: Commit**

```
git add brit-epr/src/elohim/validate.rs brit-epr/tests/elohim_validate.rs
git commit -m "feat(brit-epr/elohim): add structural pillar validator

validate_pillar_trailers enforces all three pillar summary trailers
are present and non-empty. Errors in canonical order Lamad → Shefa
→ Qahal. No CID resolution, no graph traversal — those are Phase 2."
```

---

## Task 6: Build the `brit-verify` CLI

**Files:**
- Create: `brit-verify/Cargo.toml`
- Create: `brit-verify/src/main.rs`
- Modify: `Cargo.toml` (root — add `"brit-verify"` to workspace members)

- [ ] **Step 6.1: Create the binary manifest**

Create `brit-verify/Cargo.toml`:

```toml
lints.workspace = true

[package]
name = "brit-verify"
version = "0.0.0"
description = "Verify pillar trailers on a git commit — the first brit binary"
repository = "https://github.com/ethosengine/brit"
authors = ["Matthew Dowell <matthew@ethosengine.com>"]
license = "MIT OR Apache-2.0"
edition = "2021"
rust-version = "1.82"

[[bin]]
name = "brit-verify"
path = "src/main.rs"

[dependencies]
brit-epr = { version = "^0.0.0", path = "../brit-epr" }
gix = { version = "^0.74.0", path = "../gix", default-features = false, features = ["revision"] }
```

> **Note:** The `gix` version and feature flags are illustrative. Read `gix/Cargo.toml` in this workspace for the actual current version. Try the smallest feature set that lets you open a repo and read a commit by rev (`revision` is probably enough). If cargo complains about a missing method in Step 6.3, enlarge the feature set.

- [ ] **Step 6.2: Add to workspace members**

Edit root `Cargo.toml`:

```toml
    "brit-epr",
    "brit-verify",
]
```

- [ ] **Step 6.3: Implement the binary**

Create `brit-verify/src/main.rs`:

```rust
//! `brit-verify` — verify pillar trailers on a git commit.
//!
//! Usage: `brit-verify <commit-rev> [--repo <path>]`
//!
//! Opens the repository at `<path>` (current directory if omitted), resolves
//! `<commit-rev>` to a commit object, extracts the commit message body,
//! parses pillar trailers with brit-epr, runs structural validation, and
//! prints the result. Exits 0 on success, 1 on validation failure, 2 on
//! usage error, 3 on repo error.
//!
//! No clap, no tracing — smallest possible end-to-end proof that parser
//! and validator work against real git objects.

use std::process::ExitCode;

use brit_epr::{parse_pillar_trailers, validate_pillar_trailers};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    let (rev, repo_path) = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            eprintln!("{msg}\n\nUsage: brit-verify <commit-rev> [--repo <path>]");
            return ExitCode::from(2);
        }
    };

    let repo = match gix::discover(&repo_path) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("failed to open repo at {repo_path}: {e}");
            return ExitCode::from(3);
        }
    };

    let commit = match repo.rev_parse_single(rev.as_str()) {
        Ok(id) => match id.object() {
            Ok(obj) => match obj.try_into_commit() {
                Ok(c) => c,
                Err(_) => {
                    eprintln!("rev {rev} does not point at a commit");
                    return ExitCode::from(3);
                }
            },
            Err(e) => {
                eprintln!("failed to load object for {rev}: {e}");
                return ExitCode::from(3);
            }
        },
        Err(e) => {
            eprintln!("failed to resolve rev {rev}: {e}");
            return ExitCode::from(3);
        }
    };

    let decoded = match commit.decode() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to decode commit {rev}: {e}");
            return ExitCode::from(3);
        }
    };

    // decoded.message is the full message including trailing trailers.
    let trailers = parse_pillar_trailers(decoded.message);

    match validate_pillar_trailers(&trailers) {
        Ok(()) => {
            println!("✓ pillar trailers valid for {rev}");
            println!("  Lamad: {}", trailers.lamad.as_deref().unwrap_or("-"));
            println!("  Shefa: {}", trailers.shefa.as_deref().unwrap_or("-"));
            println!("  Qahal: {}", trailers.qahal.as_deref().unwrap_or("-"));
            if let Some(ref c) = trailers.lamad_node {
                println!("  Lamad-Node: {c}");
            }
            if let Some(ref c) = trailers.shefa_node {
                println!("  Shefa-Node: {c}");
            }
            if let Some(ref c) = trailers.qahal_node {
                println!("  Qahal-Node: {c}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("✗ pillar validation failed for {rev}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: &[String]) -> Result<(String, String), String> {
    if args.len() < 2 {
        return Err("missing <commit-rev> argument".into());
    }
    let rev = args[1].clone();
    let mut repo_path = ".".to_string();

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => {
                i += 1;
                if i >= args.len() {
                    return Err("--repo requires a path argument".into());
                }
                repo_path = args[i].clone();
                i += 1;
            }
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }
    Ok((rev, repo_path))
}
```

> **Note:** The `gix` API surface (`discover`, `rev_parse_single`, `object`, `try_into_commit`, `decode`) is stable in recent gitoxide but names may shift. If cargo complains, `rg -n 'pub fn discover' ../gix/src/` and `rg -n 'rev_parse_single' ../gix/src/` to find the current signatures in this workspace's checkout. Swap method names as needed. The core shape (open repo → resolve rev → decode commit → get message → call parser + validator) is stable even if names drift. The `decode().message` field contains the full commit message including the trailer block — pass it directly to `parse_pillar_trailers`.

- [ ] **Step 6.4: Build the binary**

Run:

```
cargo build -p brit-verify
```

Expected: compiles. If API mismatches, follow the note above.

- [ ] **Step 6.5: End-to-end smoke test (manual)**

In the brit submodule workspace, create a scratch commit carrying pillar trailers:

```
git -c user.email=test@example.com -c user.name=test \
    commit --allow-empty -m "$(cat <<'EOF'
brit-verify smoke test

Lamad: smoke-test message for brit-verify integration
Shefa: zero-value scratch commit, no stewardship impact
Qahal: self-reviewed, scaffolding only
EOF
)"

SMOKE_SHA=$(git rev-parse HEAD)
cargo run -p brit-verify -- $SMOKE_SHA
```

Expected output (approximately):

```
✓ pillar trailers valid for <sha>
  Lamad: smoke-test message for brit-verify integration
  Shefa: zero-value scratch commit, no stewardship impact
  Qahal: self-reviewed, scaffolding only
```

Then verify negative case:

```
cargo run -p brit-verify -- HEAD~5
```

Expected: an upstream gitoxide commit fails with something like `✗ pillar validation failed for HEAD~5: required pillar trailer missing: Lamad` and exits non-zero.

- [ ] **Step 6.6: Roll back the smoke-test commit**

```
git reset --soft HEAD~1
git status --short
```

Verify only the brit-verify files are staged (`brit-verify/Cargo.toml`, `brit-verify/src/main.rs`, root `Cargo.toml`). Nothing else.

- [ ] **Step 6.7: Commit the binary**

```
git add brit-verify/Cargo.toml brit-verify/src/main.rs Cargo.toml
git commit -m "feat(brit-verify): first brit binary — pillar trailer verifier

Opens a repo, resolves a rev, parses pillar trailers via brit-epr,
runs structural validation, exits 0/1/2/3. No clap, no tracing —
smallest possible end-to-end proof that the engine + elohim schema
work against real git objects."
```

---

## Task 7: Bump the submodule pointer in the parent monorepo

**Files:**
- Modify: `/projects/elohim/` (parent monorepo — bumps the brit submodule SHA)

- [ ] **Step 7.1: Switch to parent monorepo**

```
cd /projects/elohim
```

- [ ] **Step 7.2: Verify the submodule pointer advanced**

```
git status elohim/brit
```

Expected: `modified: elohim/brit (new commits)`.

- [ ] **Step 7.3: Stage and commit the pointer bump**

```
git add elohim/brit
git commit -m "chore(brit): bump submodule to Phase 0+1 trailer foundation

Advances the brit submodule pointer to the commit range that adds
the brit-epr crate (engine + elohim feature module) and the
brit-verify binary. See elohim/brit/docs/plans/2026-04-11-phase-0-
epr-trailer-foundation.md for the implementation plan and
elohim/brit/docs/schemas/elohim-protocol-manifest.md for the schema."
```

- [ ] **Step 7.4: Push-dry-run the parent monorepo**

```
git push --dry-run
```

Expected: pre-push runs, reports what would be pushed. Do NOT actually push — leave that for the user to confirm.

- [ ] **Step 7.5: Report back**

Report to the user:

```
Phase 0+1 complete. Summary:

  - brit-epr crate scaffolded with engine + elohim feature split.
  - Engine: AppSchema trait, TrailerSet, parse_trailer_block via gix-object.
  - Elohim: PillarTrailers, ElohimProtocolSchema, parse_pillar_trailers,
    validate_pillar_trailers.
  - 9 tests passing (engine_parsing: 2, elohim_parse: 3, elohim_validate: 4).
  - --no-default-features build verified — engine compiles without elohim.
  - brit-verify binary builds, smoke-tested end-to-end against a real commit.
  - Submodule pointer bumped in parent monorepo.

Ready to push both repos. Waiting for confirmation.
```

---

## Self-Review

**Spec coverage:**
- ✅ Engine/app-schema split from schema doc §2.3 and §11.1 (Task 0, Task 1)
- ✅ `AppSchema` trait matching the pseudocode in §2.3 (Task 1)
- ✅ Engine parses trailer blocks without knowing the vocabulary (Task 2)
- ✅ Elohim feature module implements `AppSchema` with closed vocabulary (Task 3)
- ✅ `parse_pillar_trailers` + `validate_pillar_trailers` (Tasks 4, 5)
- ✅ `--no-default-features` compile check (Task 5 Step 5.5)
- ✅ End-to-end CLI binary with real git objects (Task 6)
- ✅ Submodule pointer bump (Task 7)
- ✅ Merge consent explicitly OUT of scope (header + scope section)
- ✅ Reach awareness explicitly OUT of scope (header + scope section)
- ✅ CID parsing explicitly OUT of scope — raw strings only (Task 3 Step 3.1, Task 4 Step 4.6)

**Placeholder scan:** None. Every step has the actual code or command. API drift notes are explicit about what to grep for when names shift.

**Type consistency:** `TrailerKey` is used identically across `pillar_trailers.rs`, `schema.rs`, `parse.rs`, `validate.rs`, and the tests. `PillarTrailers` fields (`lamad/shefa/qahal/*_node`) are used identically in parser and validator. `ValidationError` vs `PillarValidationError`: the engine has a broad `ValidationError`; the elohim module has a narrower `PillarValidationError` that reports errors in terms of `TrailerKey`. This is intentional — each layer speaks its own vocabulary.

**What this plan does NOT cover (deferred to later phases):**
- CID parsing / resolution / graph traversal → Phase 2
- ContentNode adapter → Phase 2
- `MergeProposalContentNode` + async merge consent → Phase 2 (co-resolves with §14.1 #12)
- Reach awareness on branches → Phase 2
- libp2p transport → Phase 3
- Full `brit-cli` with subcommands → Phase 3+
- Signal emission → Phase 2+
- JSON Schema codegen pipeline → Phase 2+
- Per-branch READMEs → Phase 4
- DHT announcement → Phase 5
- Fork-as-governance → Phase 6
