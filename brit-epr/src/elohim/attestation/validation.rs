use serde::{Deserialize, Serialize};
use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

/// Outcome of a named validation check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationResult {
    /// All criteria satisfied — no issues found.
    Pass,
    /// One or more blocking issues found.
    Fail,
    /// Non-blocking issues found; manual review recommended.
    Warn,
    /// Check was skipped (e.g. not applicable to this artifact).
    Skip,
}

/// Records that a validator applied a named check to an artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationAttestationContentNode {
    /// CID of the artifact under validation.
    pub artifact_cid: BritCid,
    /// Name and version of the check (e.g. `sonarqube-scan@v10`).
    pub check_name: String,
    /// Identifier of the agent that ran the check.
    pub validator_id: String,
    /// Version of the validator tool.
    pub validator_version: String,
    /// Outcome of the check.
    pub result: ValidationResult,
    /// Human-readable summary of the findings.
    pub result_summary: String,
    /// Optional CID pointing to detailed findings output.
    pub findings_cid: Option<BritCid>,
    /// ISO-8601 timestamp when the check was performed.
    pub validated_at: String,
    /// Seconds before this attestation expires; `None` means no expiry.
    pub ttl_sec: Option<u64>,
    /// Agent signature over the attestation payload.
    pub signature: String,
}

impl ContentNode for ValidationAttestationContentNode {
    fn content_type(&self) -> &'static str {
        "brit.validation-attestation"
    }
}
