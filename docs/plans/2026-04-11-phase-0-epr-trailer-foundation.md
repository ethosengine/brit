# Phase 0+1: EPR Trailer Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the `brit-epr` crate scaffolding and ship the first working EPR primitive — a pillar-trailer parser/validator — such that any git commit carrying `Lamad:`, `Shefa:`, `Qahal:` trailers can be verified by a `brit-verify` binary.

**Architecture:** New crate `brit-epr` lives in the gitoxide workspace alongside `gix-*` crates, depends only on `gix-object` (for trailer parsing) and `gix-hash` (for OID types). A thin example binary `brit-verify` demonstrates end-to-end use: given a commit SHA in a local repo, parse its trailers, extract pillar fields, and exit 0 if all three pillars are present and well-formed. Zero modifications to existing `gix-*` crates — pure additive scaffolding.

**Tech Stack:** Rust 2021, gitoxide's existing `gix-object` + `gix-hash` crates, `thiserror` for error types, `winnow` (already in gitoxide) is **not** needed because we reuse `BodyRef::trailers()`. Stock Rust + cargo only.

**Upstream compatibility:** Every commit a brit user creates round-trips through stock git. Pillar trailers follow RFC-822 "Key: value" syntax, indistinguishable from `Signed-off-by:` to any reader that doesn't know about them.

**Scope notes:**

- Phases 0 and 1 are bundled because each is too small to justify its own plan file. Phase 0 is the crate + workspace plumbing; Phase 1 is the parser, validator, and binary. Splitting would mean two commits with one adding an empty crate.
- No changes to any existing `gix-*` crate in this plan. If a bug is discovered in `gix-object::commit::message::body` while working on this plan, file it as a separate issue — do not fold it in.
- Linked-ContentNode CIDs are **allowed but not required** at this phase. The trailer format reserves keys `Lamad-Node`, `Shefa-Node`, `Qahal-Node` for CID references, and the parser accepts them, but validation does NOT require them to exist or resolve. That's Phase 2.

---

## File Structure

```
brit/
├── brit-epr/
│   ├── Cargo.toml           # new crate, members of workspace
│   ├── src/
│   │   ├── lib.rs           # crate root, re-exports
│   │   ├── trailer.rs       # PillarTrailers struct, TrailerKey enum
│   │   ├── parse.rs         # parse_pillar_trailers() using gix-object
│   │   ├── validate.rs      # PillarValidator, PillarValidationError
│   │   └── error.rs         # crate error type
│   └── tests/
│       ├── parse.rs         # unit tests for parser
│       ├── validate.rs      # unit tests for validator
│       └── fixtures/        # raw commit-object bytes for happy/sad paths
│           ├── happy_all_three_pillars.txt
│           ├── missing_qahal.txt
│           └── malformed_shefa.txt
├── brit-verify/
│   ├── Cargo.toml           # new binary crate
│   └── src/
│       └── main.rs          # CLI: brit-verify <commit-sha> [--repo <path>]
└── Cargo.toml               # modified: add "brit-epr", "brit-verify" to workspace members
```

**Responsibilities per file:**

- `brit-epr/src/trailer.rs` — data types only. `PillarTrailers` is a plain struct with three `Option<String>` fields (one per pillar) and three `Option<gix_hash::ObjectId>` fields for linked-node CIDs (reserved for Phase 2; parsed but unused in validation).
- `brit-epr/src/parse.rs` — one public function `parse_pillar_trailers(body: &BodyRef<'_>) -> PillarTrailers`. Pure function, no I/O, no allocation beyond the returned struct.
- `brit-epr/src/validate.rs` — `PillarValidator::validate(&PillarTrailers) -> Result<(), PillarValidationError>`. Structural validation only (all three present + non-empty). No semantic validation (no CID resolution, no graph checks).
- `brit-epr/src/error.rs` — `PillarError` enum via `thiserror`. Three variants for now: `MissingTrailer(TrailerKey)`, `EmptyValue(TrailerKey)`, `MalformedNodeRef(TrailerKey, String)`.
- `brit-epr/src/lib.rs` — re-exports everything under the crate root, adds crate-level docs.
- `brit-verify/src/main.rs` — minimal CLI parsing (`std::env::args`, no clap), opens repo, reads commit object, parses body, runs validator, prints result, exits 0/1.

---

## Task 0: Scaffolding — add `brit-epr` crate

**Files:**
- Create: `brit-epr/Cargo.toml`
- Create: `brit-epr/src/lib.rs`
- Modify: `Cargo.toml` (root — add to workspace members)

- [ ] **Step 0.1: Create the empty crate manifest**

Create `brit-epr/Cargo.toml`:

```toml
lints.workspace = true

[package]
name = "brit-epr"
version = "0.0.0"
description = "Elohim Protocol primitives (pillar trailers, ContentNode types, validation) for brit — an expansion of gitoxide with EPR semantics"
repository = "https://github.com/ethosengine/brit"
authors = ["Matthew Dowell <matthew@ethosengine.com>"]
license = "MIT OR Apache-2.0"
edition = "2021"
rust-version = "1.82"

[lib]
doctest = false

[dependencies]
gix-object = { version = "^0.52.0", path = "../gix-object" }
gix-hash = { version = "^0.19.0", path = "../gix-hash" }
thiserror = "2.0"

[dev-dependencies]
# (unit tests don't need external crates yet)
```

> **Note:** Version numbers for `gix-object` / `gix-hash` must match the values currently in `gix-object/Cargo.toml` and `gix-hash/Cargo.toml`. If you see a version mismatch when cargo builds, read the `[package] version` line in each crate's `Cargo.toml` and update accordingly. This plan uses illustrative versions.

- [ ] **Step 0.2: Create the empty lib.rs**

Create `brit-epr/src/lib.rs`:

```rust
//! Elohim Protocol primitives for brit.
//!
//! This crate is additive scaffolding — it imports types from `gix-object` and
//! `gix-hash` but never modifies them. The goal is to layer EPR semantics onto
//! stock git without forking the object model.
//!
//! # Modules
//!
//! - [`trailer`] — [`PillarTrailers`] data type and [`TrailerKey`] enum.
//! - [`parse`]   — [`parse::parse_pillar_trailers`], a pure function that extracts
//!                 pillar trailers from a parsed [`gix_object::commit::message::BodyRef`].
//! - [`validate`] — [`validate::PillarValidator`], structural validation only
//!                  (no CID resolution, no graph traversal).
//! - [`error`]   — [`error::PillarError`] via `thiserror`.

#![deny(missing_docs, rust_2018_idioms)]
#![forbid(unsafe_code)]

pub mod error;
pub mod parse;
pub mod trailer;
pub mod validate;

pub use error::PillarError;
pub use parse::parse_pillar_trailers;
pub use trailer::{PillarTrailers, TrailerKey};
pub use validate::PillarValidator;
```

- [ ] **Step 0.3: Add to workspace members**

Edit root `Cargo.toml`. Find the `members = [` list and add `"brit-epr"` as the last entry before the closing `]`. Place it after `"gix-shallow"`:

```toml
# ... existing members ...
    "gix-shallow",
    "brit-epr",
]
```

- [ ] **Step 0.4: Verify the workspace builds with the empty crate**

Run:

```
cargo build -p brit-epr
```

Expected: compiles with at most warnings about unused `thiserror` import. The `deny(missing_docs)` lint requires every module to exist, so this will fail until Task 1 creates the module files. Expected failure message includes `file not found for module`. This is OK — move to Task 1.

*(If the build passes with no errors despite missing modules, something's wrong — re-read the lib.rs to confirm the `pub mod` lines are present.)*

- [ ] **Step 0.5: Commit**

```
git add brit-epr/Cargo.toml brit-epr/src/lib.rs Cargo.toml
git commit -m "feat(brit-epr): scaffold crate for EPR primitives

Adds an empty brit-epr crate to the workspace. No functionality
yet — subsequent tasks land the trailer data types, parser, and
validator."
```

---

## Task 1: Define `PillarTrailers` data types

**Files:**
- Create: `brit-epr/src/trailer.rs`
- Create: `brit-epr/src/error.rs`

- [ ] **Step 1.1: Create `error.rs` first (trailer module depends on it)**

Create `brit-epr/src/error.rs`:

```rust
//! Errors emitted by pillar trailer parsing and validation.

use crate::trailer::TrailerKey;

/// Errors raised by [`crate::PillarValidator`] and [`crate::parse_pillar_trailers`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PillarError {
    /// A required pillar trailer is missing from the commit message body.
    #[error("required pillar trailer is missing: {0:?}")]
    MissingTrailer(TrailerKey),

    /// A pillar trailer is present but has an empty value after trimming.
    #[error("pillar trailer {0:?} is present but value is empty")]
    EmptyValue(TrailerKey),

    /// A linked-node trailer (e.g. `Lamad-Node:`) is present but the value
    /// cannot be parsed as a git `ObjectId` / CID.
    #[error("pillar trailer {0:?} has malformed node reference: {1}")]
    MalformedNodeRef(TrailerKey, String),
}
```

- [ ] **Step 1.2: Create `trailer.rs`**

Create `brit-epr/src/trailer.rs`:

```rust
//! Pillar trailer data types.
//!
//! The Elohim Protocol couples every notarized artifact to three pillars:
//!
//! - **Lamad** (לָמַד, "to learn") — knowledge positioning: what this change teaches,
//!   what path it advances, what mastery it unlocks.
//! - **Shefa** (שֶׁפַע, "abundance") — economic positioning: who contributed, what
//!   value flowed, what stewardship changed.
//! - **Qahal** (קָהָל, "assembly") — governance positioning: who consented, who
//!   reviewed, what collective authorized the change.
//!
//! Each pillar gets a trailer with a canonical summary (`Lamad:` line) AND an
//! optional CID-addressed linked ContentNode (`Lamad-Node:` line) carrying the
//! rich graph data. This plan implements only the canonical-summary trailers;
//! linked-node refs are parsed but otherwise unused until Phase 2.

use gix_hash::ObjectId;

/// Which of the three pillars a trailer belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrailerKey {
    /// Knowledge-layer trailer (`Lamad:` or `Lamad-Node:`).
    Lamad,
    /// Economic-layer trailer (`Shefa:` or `Shefa-Node:`).
    Shefa,
    /// Governance-layer trailer (`Qahal:` or `Qahal-Node:`).
    Qahal,
}

impl TrailerKey {
    /// The RFC-822-style token name for the canonical-summary trailer.
    pub fn summary_token(self) -> &'static str {
        match self {
            TrailerKey::Lamad => "Lamad",
            TrailerKey::Shefa => "Shefa",
            TrailerKey::Qahal => "Qahal",
        }
    }

    /// The RFC-822-style token name for the linked-node trailer.
    pub fn node_token(self) -> &'static str {
        match self {
            TrailerKey::Lamad => "Lamad-Node",
            TrailerKey::Shefa => "Shefa-Node",
            TrailerKey::Qahal => "Qahal-Node",
        }
    }
}

/// Pillar trailers as extracted from a commit message body.
///
/// Each field is `Option` because parsing is permissive — if a commit is missing
/// a pillar trailer, the parser reports `None` and the validator decides whether
/// that's an error. This lets tools that don't enforce EPR (e.g. mirrors, legacy
/// importers) still read what they can.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PillarTrailers {
    /// Canonical summary value of the `Lamad:` trailer, trimmed.
    pub lamad: Option<String>,
    /// Canonical summary value of the `Shefa:` trailer, trimmed.
    pub shefa: Option<String>,
    /// Canonical summary value of the `Qahal:` trailer, trimmed.
    pub qahal: Option<String>,

    /// CID of the linked-node ContentNode for the Lamad pillar. `None` means
    /// either the trailer was absent OR the value failed CID parsing — check
    /// the parser error log if strict mode is needed.
    pub lamad_node: Option<ObjectId>,
    /// CID of the linked-node ContentNode for the Shefa pillar.
    pub shefa_node: Option<ObjectId>,
    /// CID of the linked-node ContentNode for the Qahal pillar.
    pub qahal_node: Option<ObjectId>,
}
```

- [ ] **Step 1.3: Write a compilation-only test**

Create `brit-epr/tests/parse.rs` with a single placeholder test so `cargo test -p brit-epr` has something to run:

```rust
//! Integration tests for pillar trailer parsing.
//!
//! Each fixture is a raw commit-object body (post-header) checked into
//! `tests/fixtures/`. The parser is called on the body and the resulting
//! `PillarTrailers` compared against a hand-written expectation.

use brit_epr::PillarTrailers;

#[test]
fn data_types_compile() {
    let _ = PillarTrailers::default();
}
```

- [ ] **Step 1.4: Build and run the placeholder test**

Run:

```
cargo test -p brit-epr
```

Expected: 1 test passes. `data_types_compile ... ok`. If you see `unresolved import` or `file not found for module parse`, create empty stub files at `brit-epr/src/parse.rs` and `brit-epr/src/validate.rs` containing only:

```rust
//! (stub — implementation in the next task)
```

and retry.

- [ ] **Step 1.5: Commit**

```
git add brit-epr/src/error.rs brit-epr/src/trailer.rs brit-epr/tests/parse.rs brit-epr/src/parse.rs brit-epr/src/validate.rs
git commit -m "feat(brit-epr): add PillarTrailers and TrailerKey data types

Defines the data model for the three pillar trailers (Lamad, Shefa,
Qahal) and their linked-node CID references. No parsing or validation
yet — those land in the next two tasks."
```

---

## Task 2: Implement the parser (TDD)

**Files:**
- Modify: `brit-epr/src/parse.rs`
- Modify: `brit-epr/tests/parse.rs`
- Create: `brit-epr/tests/fixtures/happy_all_three_pillars.txt`
- Create: `brit-epr/tests/fixtures/missing_qahal.txt`
- Create: `brit-epr/tests/fixtures/malformed_shefa.txt`

- [ ] **Step 2.1: Write the first failing test — happy path**

Create `brit-epr/tests/fixtures/happy_all_three_pillars.txt`:

```
Add pillar trailer parser

Wires gix-object::BodyRef::trailers() into the brit-epr parser so
commit messages can carry Lamad / Shefa / Qahal values natively.

Signed-off-by: Matthew Dowell <matthew@ethosengine.com>
Lamad: introduces pillar trailer model; first testable EPR primitive
Shefa: stewardship by @matthew; contributor credit via git author
Qahal: no governance review required for scaffolding
```

Replace the entire contents of `brit-epr/tests/parse.rs` with:

```rust
//! Integration tests for pillar trailer parsing.

use gix_object::commit::message::BodyRef;
use brit_epr::{parse_pillar_trailers, PillarTrailers};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/{}", name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"))
}

#[test]
fn happy_path_all_three_pillars_parse() {
    let body_bytes = fixture("happy_all_three_pillars.txt");
    let body = BodyRef::from_bytes(&body_bytes);

    let trailers = parse_pillar_trailers(&body);

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

- [ ] **Step 2.2: Run the test — expect compilation failure**

Run:

```
cargo test -p brit-epr happy_path_all_three_pillars_parse
```

Expected: compilation error, `cannot find function parse_pillar_trailers in crate brit_epr`. This is the RED step of TDD — do not implement yet.

- [ ] **Step 2.3: Implement the parser**

Replace `brit-epr/src/parse.rs` with:

```rust
//! Pillar trailer parser.
//!
//! Reuses gitoxide's existing RFC-822 trailer parser
//! ([`gix_object::commit::message::body::Trailers`]) and projects the
//! key/value pairs into the typed [`PillarTrailers`] struct. Unknown
//! trailers are ignored.

use gix_hash::ObjectId;
use gix_object::commit::message::BodyRef;

use crate::trailer::{PillarTrailers, TrailerKey};

/// Parse pillar trailers from a commit message body.
///
/// This is a pure function. It does not allocate beyond the returned struct
/// (three `String`s for summary values, up to three `ObjectId`s for linked
/// nodes) and does no I/O.
///
/// Unknown trailers (anything whose token isn't one of the six reserved pillar
/// keys) are silently skipped — commits may carry `Signed-off-by:`, `Co-Authored-By:`,
/// etc. alongside pillar trailers.
///
/// Malformed linked-node values (invalid CID / OID) are *silently dropped*:
/// the corresponding `*_node` field stays `None`. This is intentional — the
/// parser is permissive; strict validation lives in [`crate::PillarValidator`].
pub fn parse_pillar_trailers(body: &BodyRef<'_>) -> PillarTrailers {
    let mut out = PillarTrailers::default();

    for trailer in body.trailers() {
        let token = trailer.token.to_string();
        let value = trailer.value.to_string();

        match token.as_str() {
            t if t == TrailerKey::Lamad.summary_token() => {
                out.lamad = Some(value);
            }
            t if t == TrailerKey::Shefa.summary_token() => {
                out.shefa = Some(value);
            }
            t if t == TrailerKey::Qahal.summary_token() => {
                out.qahal = Some(value);
            }
            t if t == TrailerKey::Lamad.node_token() => {
                out.lamad_node = ObjectId::from_hex(value.as_bytes()).ok();
            }
            t if t == TrailerKey::Shefa.node_token() => {
                out.shefa_node = ObjectId::from_hex(value.as_bytes()).ok();
            }
            t if t == TrailerKey::Qahal.node_token() => {
                out.qahal_node = ObjectId::from_hex(value.as_bytes()).ok();
            }
            _ => {} // unknown trailer — ignore
        }
    }

    out
}
```

- [ ] **Step 2.4: Run the test again — expect pass**

Run:

```
cargo test -p brit-epr happy_path_all_three_pillars_parse
```

Expected: 1 test passes. If the `.to_string()` call fails to compile because `BStr` doesn't have `to_string`, swap it for `.to_str_lossy().into_owned()`. (The `BStr` type in gitoxide is from `bstr`, which implements `Display` and therefore `ToString`, so this should just work.)

- [ ] **Step 2.5: Add a second failing test — missing qahal**

Create `brit-epr/tests/fixtures/missing_qahal.txt`:

```
Routine refactor with only two pillars declared

Lamad: no knowledge change — pure refactor
Shefa: no value flow — maintenance work
```

Append to `brit-epr/tests/parse.rs`:

```rust
#[test]
fn missing_qahal_parses_partially() {
    let body_bytes = fixture("missing_qahal.txt");
    let body = BodyRef::from_bytes(&body_bytes);

    let trailers = parse_pillar_trailers(&body);

    assert_eq!(trailers.lamad.as_deref(), Some("no knowledge change — pure refactor"));
    assert_eq!(trailers.shefa.as_deref(), Some("no value flow — maintenance work"));
    assert_eq!(trailers.qahal, None);
}
```

- [ ] **Step 2.6: Run both tests**

Run:

```
cargo test -p brit-epr
```

Expected: 2 tests pass (`data_types_compile`, `happy_path_all_three_pillars_parse`, `missing_qahal_parses_partially`). If `missing_qahal_parses_partially` fails, `BodyRef::from_bytes` may have trimmed the two-trailer block because there's no separator empty line; inspect the fixture and confirm the file has a blank line between the body and the trailers.

- [ ] **Step 2.7: Add a third failing test — malformed node ref**

Create `brit-epr/tests/fixtures/malformed_shefa.txt`:

```
Test malformed shefa node reference

Lamad: teaches the permissive parser behavior
Shefa: value summary is fine
Shefa-Node: not-a-valid-object-id-at-all
Qahal: governance review complete
```

Append to `brit-epr/tests/parse.rs`:

```rust
#[test]
fn malformed_shefa_node_drops_silently() {
    let body_bytes = fixture("malformed_shefa.txt");
    let body = BodyRef::from_bytes(&body_bytes);

    let trailers = parse_pillar_trailers(&body);

    // Summary values all parse…
    assert_eq!(trailers.lamad.as_deref(), Some("teaches the permissive parser behavior"));
    assert_eq!(trailers.shefa.as_deref(), Some("value summary is fine"));
    assert_eq!(trailers.qahal.as_deref(), Some("governance review complete"));

    // …but the malformed Shefa-Node is silently dropped.
    assert_eq!(trailers.shefa_node, None);
}
```

- [ ] **Step 2.8: Run all three tests**

Run:

```
cargo test -p brit-epr
```

Expected: 4 tests pass (`data_types_compile` + three parse tests).

- [ ] **Step 2.9: Commit**

```
git add brit-epr/src/parse.rs brit-epr/tests/parse.rs brit-epr/tests/fixtures/
git commit -m "feat(brit-epr): implement pillar trailer parser

parse_pillar_trailers() projects gix-object trailer iterator output into
a typed PillarTrailers struct. Permissive: unknown trailers skipped,
malformed linked-node refs silently dropped. Three fixtures cover the
happy path, partial declaration, and malformed node-ref."
```

---

## Task 3: Implement the validator (TDD)

**Files:**
- Modify: `brit-epr/src/validate.rs`
- Create: `brit-epr/tests/validate.rs`

- [ ] **Step 3.1: Write the first failing test — happy path validation**

Create `brit-epr/tests/validate.rs`:

```rust
//! Integration tests for pillar trailer structural validation.

use brit_epr::{PillarError, PillarTrailers, PillarValidator, TrailerKey};

fn complete_trailers() -> PillarTrailers {
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
    let trailers = complete_trailers();
    assert_eq!(PillarValidator::validate(&trailers), Ok(()));
}
```

- [ ] **Step 3.2: Run it — expect failure**

Run:

```
cargo test -p brit-epr all_three_present_validates_ok
```

Expected: compile error, `cannot find PillarValidator in crate brit_epr`.

- [ ] **Step 3.3: Implement the validator**

Replace `brit-epr/src/validate.rs` with:

```rust
//! Structural validation for pillar trailers.
//!
//! This layer only checks that each pillar has a non-empty summary value.
//! It does NOT resolve linked-node CIDs, does NOT traverse the ContentNode
//! graph, and does NOT enforce domain-specific rules (those live in higher
//! layers built in Phase 2+).

use crate::error::PillarError;
use crate::trailer::{PillarTrailers, TrailerKey};

/// Structural validator for [`PillarTrailers`].
///
/// Usage:
///
/// ```ignore
/// use brit_epr::{PillarValidator, PillarTrailers};
/// let trailers = PillarTrailers::default();
/// let result = PillarValidator::validate(&trailers);
/// assert!(result.is_err());
/// ```
pub struct PillarValidator;

impl PillarValidator {
    /// Validate structural completeness: all three pillar summary trailers
    /// must be present and must not be empty after trimming whitespace.
    ///
    /// Returns `Ok(())` on success, or the first [`PillarError`] encountered
    /// in order (Lamad, Shefa, Qahal).
    pub fn validate(trailers: &PillarTrailers) -> Result<(), PillarError> {
        Self::check_pillar(TrailerKey::Lamad, trailers.lamad.as_deref())?;
        Self::check_pillar(TrailerKey::Shefa, trailers.shefa.as_deref())?;
        Self::check_pillar(TrailerKey::Qahal, trailers.qahal.as_deref())?;
        Ok(())
    }

    fn check_pillar(key: TrailerKey, value: Option<&str>) -> Result<(), PillarError> {
        match value {
            None => Err(PillarError::MissingTrailer(key)),
            Some(v) if v.trim().is_empty() => Err(PillarError::EmptyValue(key)),
            Some(_) => Ok(()),
        }
    }
}
```

- [ ] **Step 3.4: Run the test — expect pass**

Run:

```
cargo test -p brit-epr all_three_present_validates_ok
```

Expected: pass.

- [ ] **Step 3.5: Write failure-path tests**

Append to `brit-epr/tests/validate.rs`:

```rust
#[test]
fn missing_lamad_fails_with_missing_trailer() {
    let mut trailers = complete_trailers();
    trailers.lamad = None;

    assert_eq!(
        PillarValidator::validate(&trailers),
        Err(PillarError::MissingTrailer(TrailerKey::Lamad))
    );
}

#[test]
fn empty_shefa_fails_with_empty_value() {
    let mut trailers = complete_trailers();
    trailers.shefa = Some("   ".into());

    assert_eq!(
        PillarValidator::validate(&trailers),
        Err(PillarError::EmptyValue(TrailerKey::Shefa))
    );
}

#[test]
fn validation_returns_first_error_in_order() {
    // Both lamad and qahal are missing — we expect Lamad (first in order).
    let trailers = PillarTrailers {
        lamad: None,
        shefa: Some("ok".into()),
        qahal: None,
        ..Default::default()
    };

    assert_eq!(
        PillarValidator::validate(&trailers),
        Err(PillarError::MissingTrailer(TrailerKey::Lamad))
    );
}
```

- [ ] **Step 3.6: Run all validation tests**

Run:

```
cargo test -p brit-epr
```

Expected: 7 tests total pass (1 compile + 3 parse + 1 happy validator + 3 failure validators).

- [ ] **Step 3.7: Commit**

```
git add brit-epr/src/validate.rs brit-epr/tests/validate.rs
git commit -m "feat(brit-epr): add structural pillar validator

PillarValidator::validate() enforces all three pillar summary
trailers are present and non-empty. Errors in order Lamad→Shefa→Qahal.
No semantic validation (CID resolution, graph traversal) — that's
Phase 2."
```

---

## Task 4: Build the `brit-verify` CLI

**Files:**
- Create: `brit-verify/Cargo.toml`
- Create: `brit-verify/src/main.rs`
- Modify: `Cargo.toml` (root — add `"brit-verify"` to workspace members)

- [ ] **Step 4.1: Create the binary crate manifest**

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

> **Note:** The `gix` version and feature flags are illustrative. Read `gix/Cargo.toml` in this workspace for the actual current version and pick the smallest feature set that lets you open a repo and read a commit by rev. `revision` is likely enough; if `cargo build` complains about missing features, try adding `"blocking-network-client"` (probably unnecessary for local reads) or `"max-performance"`.

- [ ] **Step 4.2: Add to workspace members**

Edit root `Cargo.toml`, add `"brit-verify"` after `"brit-epr"`:

```toml
    "brit-epr",
    "brit-verify",
]
```

- [ ] **Step 4.3: Implement the binary**

Create `brit-verify/src/main.rs`:

```rust
//! `brit-verify` — verify pillar trailers on a git commit.
//!
//! Usage: `brit-verify <commit-rev> [--repo <path>]`
//!
//! Opens the repository at `<path>` (or the current directory if `--repo` is
//! omitted), resolves `<commit-rev>` to a commit object, extracts the commit
//! message body, parses pillar trailers, runs structural validation, and
//! prints the result. Exits 0 on success, 1 on any error.
//!
//! No clap, no tracing — this is the smallest possible end-to-end proof that
//! the parser + validator work against real git objects.

use std::process::ExitCode;

use brit_epr::{parse_pillar_trailers, PillarValidator};

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
            return ExitCode::FAILURE;
        }
    };

    let object = match repo.rev_parse_single(rev.as_str()).and_then(|id| id.object().map_err(Into::into)) {
        Ok(obj) => obj,
        Err(e) => {
            eprintln!("failed to resolve rev {rev}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let commit = match object.try_into_commit() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("rev {rev} does not point to a commit");
            return ExitCode::FAILURE;
        }
    };

    let decoded = match commit.decode() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to decode commit {rev}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let body = gix::objs::commit::message::BodyRef::from_bytes(decoded.message);
    let trailers = parse_pillar_trailers(&body);

    match PillarValidator::validate(&trailers) {
        Ok(()) => {
            println!("✓ pillar trailers valid for {rev}");
            println!("  Lamad: {}", trailers.lamad.as_deref().unwrap_or("-"));
            println!("  Shefa: {}", trailers.shefa.as_deref().unwrap_or("-"));
            println!("  Qahal: {}", trailers.qahal.as_deref().unwrap_or("-"));
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

> **Note:** The `gix` API surface (`discover`, `rev_parse_single`, `object`, `try_into_commit`, `decode`) is stable as of gitoxide 0.52 but names may shift in this workspace's checkout. If cargo complains about any of these methods, `rg -n 'pub fn discover' ../gix/src/` and `rg -n 'rev_parse_single' ../gix/src/` to find the current signatures. Swap method names as needed. The test in Step 4.5 will catch regressions.

- [ ] **Step 4.4: Build the binary**

Run:

```
cargo build -p brit-verify
```

Expected: compiles. If the `gix` API differs from the plan, read the relevant `gix/src/*.rs` files and adjust the binary — the core shape (open repo → resolve rev → decode commit → get message → call parser + validator) is stable even if the method names shift.

- [ ] **Step 4.5: End-to-end smoke test against a real commit**

This is a manual verification step. In the brit submodule workspace, create a scratch commit carrying pillar trailers:

```
git -c user.email=test@example.com -c user.name=test \
    commit --allow-empty -m "$(cat <<'EOF'
brit-verify smoke test

Lamad: smoke-test message for brit-verify integration
Shefa: zero-value scratch commit, no stewardship impact
Qahal: self-reviewed, scaffolding only
EOF
)"
```

Grab the SHA of the new commit:

```
SMOKE_SHA=$(git rev-parse HEAD)
```

Run the binary:

```
cargo run -p brit-verify -- $SMOKE_SHA
```

Expected output (approximately):

```
✓ pillar trailers valid for <sha>
  Lamad: smoke-test message for brit-verify integration
  Shefa: zero-value scratch commit, no stewardship impact
  Qahal: self-reviewed, scaffolding only
```

Then run a negative case against a real upstream gitoxide commit that has no pillar trailers:

```
cargo run -p brit-verify -- HEAD~5
```

Expected output (approximately):

```
✗ pillar validation failed for HEAD~5: required pillar trailer is missing: Lamad
```

and the process should exit with a non-zero code.

- [ ] **Step 4.6: Roll back the smoke-test commit before committing**

The scratch commit from 4.5 was only to test the binary. Reset it:

```
git reset --soft HEAD~1
```

> **Note:** `--soft` keeps the working tree and staged files intact so the binary sources from Steps 4.1-4.3 are preserved. Verify with `git status` that only the brit-verify files are staged, nothing else.

- [ ] **Step 4.7: Commit the binary**

```
git add brit-verify/Cargo.toml brit-verify/src/main.rs Cargo.toml
git commit -m "feat(brit-verify): add pillar trailer verification binary

First brit binary. Opens a repo, resolves a commit rev, parses pillar
trailers, runs structural validation, exits 0/1. No clap, no tracing —
smallest possible end-to-end proof that parser + validator work against
real git objects."
```

---

## Task 5: Update the submodule pointer in the parent monorepo

**Files:**
- Modify: `/projects/elohim/` (parent monorepo — bumps the brit submodule SHA)

- [ ] **Step 5.1: Change directory to parent monorepo**

Run:

```
cd /projects/elohim
```

- [ ] **Step 5.2: Confirm the submodule pointer advanced**

Run:

```
git status elohim/brit
```

Expected: `modified: elohim/brit (new commits)`.

- [ ] **Step 5.3: Commit the submodule pointer bump**

Run:

```
git add elohim/brit .gitmodules
git commit -m "chore(brit): bump submodule to Phase 0+1 trailer foundation

Advances the brit submodule pointer to the commit that adds the
brit-epr crate and brit-verify binary. See brit/docs/plans/
README.md for the EPR-git roadmap and Phase 0+1 implementation plan."
```

- [ ] **Step 5.4: Run the monorepo's pre-push gate locally**

The `.husky/pre-push` hook will run the orchestrator gate for Jenkinsfile changes, and will skip the brit change because there's no manifest entry yet for it. That's expected. Run:

```
HUSKY=1 git push --dry-run
```

Expected: pre-push runs, no errors, "Everything up-to-date" or a list of what would be pushed. Do **not** actually push yet — that's deferred to the user's call.

- [ ] **Step 5.5: Final task — back to user**

Stop and report to the user:

```
Phase 0+1 complete. Summary:

  - brit-epr crate scaffolded with PillarTrailers, parse_pillar_trailers,
    and PillarValidator. 7 tests passing.
  - brit-verify binary builds and runs end-to-end against a real commit
    (smoke-tested locally, commit rolled back before committing).
  - submodule pointer bumped in parent monorepo.

Ready to push both repos. Brit commits are local in the submodule;
parent monorepo has one commit bumping the pointer. Want me to push
both, or hold for review?
```

---

## Self-Review (done before handoff)

**Spec coverage:**
- ✅ `PillarTrailers` data model (Task 1)
- ✅ Parser using gix-object BodyRef::trailers() (Task 2)
- ✅ Structural validator (Task 3)
- ✅ End-to-end CLI binary (Task 4)
- ✅ Upstream-rebaseable — zero modifications to `gix-*` crates
- ✅ Trailer format is RFC-822 compatible — round-trips through stock git
- ✅ Linked-node CID slots reserved but unused (Phase 2 placeholder)

**Placeholder scan:** None. Every step has the actual code or the actual command. Where the `gix` API surface might shift, the plan says *exactly* what to grep for.

**Type consistency:** `TrailerKey` is used identically across `trailer.rs`, `error.rs`, `parse.rs`, `validate.rs`. `PillarTrailers` fields (`lamad`, `shefa`, `qahal`, `lamad_node`, `shefa_node`, `qahal_node`) are used identically across parser and validator.

**What this plan does NOT cover (deferred to later phases):**
- CID resolution / ContentNode graph traversal → Phase 2
- libp2p transport → Phase 3
- Per-branch READMEs → Phase 4
- DHT announcement → Phase 5
- Fork-as-governance → Phase 6
