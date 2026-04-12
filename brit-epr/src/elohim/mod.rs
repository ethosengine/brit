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
