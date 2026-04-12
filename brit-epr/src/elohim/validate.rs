//! Structural validation for pillar trailers.
//!
//! Checks that each pillar has a non-empty summary value. Does NOT resolve
//! linked-node CIDs, does NOT traverse the ContentNode graph, does NOT
//! enforce domain rules — those live in higher layers (Phase 2+).

use thiserror::Error;

use super::pillar_trailers::{PillarTrailers, TrailerKey};

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
