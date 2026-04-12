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
