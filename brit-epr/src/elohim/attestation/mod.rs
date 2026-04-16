//! Attestation ContentNode types for the Elohim Protocol.

/// Build attestation — records an agent producing an output artifact.
pub mod build;
/// Deploy attestation — records an agent confirming an artifact is live.
pub mod deploy;
/// Reach computation — derives a reach level from existing attestations.
pub mod reach;
/// Validation attestation — records a named check applied to an artifact.
pub mod validation;
