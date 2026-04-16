//! Reach computation — derives a reach level from existing attestations.

use serde::{Deserialize, Serialize};

/// Reach level derived from attestations. Unknown < Built < Deployed < Verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReachLevel {
    /// No attestations exist for this artifact.
    Unknown,
    /// Build attestation exists; artifact was produced successfully.
    Built,
    /// Build and deploy attestations exist; artifact is live.
    Deployed,
    /// Build, deploy, and validation attestations exist; artifact is verified.
    Verified,
}

/// Input for reach computation.
#[derive(Debug, Clone)]
pub struct ReachInput {
    /// Agent IDs or step names of agents that produced build attestations.
    pub build_attestations: Vec<String>,
    /// Environment labels or step names of deploy attestations.
    pub deploy_attestations: Vec<String>,
    /// Check names of validation attestations.
    pub validation_attestations: Vec<String>,
}

/// Compute reach level. Deterministic: same inputs = same output.
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
