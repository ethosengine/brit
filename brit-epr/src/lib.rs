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
pub use engine::{AppSchema, BritCid, CidParseError, ContentNode, LocalObjectStore, ObjectStoreError, TrailerSet, ValidationError};

// Feature-gated re-exports
#[cfg(feature = "elohim-protocol")]
pub use elohim::{
    parse_pillar_trailers, validate_pillar_trailers, ElohimProtocolSchema, PillarTrailers,
    PillarValidationError, TrailerKey,
};

/// Convenience re-exports for attestation types.
#[cfg(feature = "elohim-protocol")]
pub mod attestation {
    pub use crate::elohim::attestation::build::BuildAttestationContentNode;
    pub use crate::elohim::attestation::deploy::{DeployAttestationContentNode, HealthStatus};
    pub use crate::elohim::attestation::reach::{compute_reach, ReachInput, ReachLevel};
    pub use crate::elohim::attestation::validation::{
        ValidationAttestationContentNode, ValidationResult,
    };
    pub use crate::elohim::refs::BritRefManager;
}
