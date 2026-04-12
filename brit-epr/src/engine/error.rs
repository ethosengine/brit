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
