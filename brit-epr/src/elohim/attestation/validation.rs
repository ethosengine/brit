use serde::{Deserialize, Serialize};

use super::note::{NoteBody, VersionedNote};
use crate::engine::{cid::BritCid, content_node::ContentNode, signing::Signed};

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

/// Records that a validator applied a named check to an artifact — the frozen legacy (epoch 1)
/// note; its serializer is the signature preimage of every historical validation note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

impl Signed for ValidationAttestationContentNode {
    fn signature(&self) -> &str {
        &self.signature
    }
    fn agent_id(&self) -> &str {
        &self.validator_id
    }
    fn without_signature(&self) -> Self {
        let mut c = self.clone();
        c.signature = String::new();
        c
    }
}

/// What a validation note attests, signed in the envelope format (`brit.validate@2`): the
/// legacy node's fields without `signature`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationBody {
    /// CID of the artifact under validation.
    pub artifact_cid: BritCid,
    /// Name and version of the check (e.g. `sonarqube-scan@v10`).
    pub check_name: String,
    /// The validator's raw ed25519 public key (hex); the envelope key must be `ed 01 ‖` it.
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
}

impl NoteBody for ValidationBody {
    type Legacy = ValidationAttestationContentNode;
    const FORMAT: &'static str = "brit.validate@2";
    const DOMAIN: &'static [u8] = b"brit:validate@2:";

    fn agent_id(&self) -> &str {
        &self.validator_id
    }

    fn from_legacy(legacy: &ValidationAttestationContentNode) -> Self {
        Self {
            artifact_cid: legacy.artifact_cid.clone(),
            check_name: legacy.check_name.clone(),
            validator_id: legacy.validator_id.clone(),
            validator_version: legacy.validator_version.clone(),
            result: legacy.result,
            result_summary: legacy.result_summary.clone(),
            findings_cid: legacy.findings_cid.clone(),
            validated_at: legacy.validated_at.clone(),
            ttl_sec: legacy.ttl_sec,
        }
    }
}

/// A validation note in either format.
pub type ValidationNote = VersionedNote<ValidationBody>;

impl From<&ValidationAttestationContentNode> for ValidationNote {
    fn from(legacy: &ValidationAttestationContentNode) -> Self {
        Self::Legacy(legacy.clone())
    }
}

impl From<&ValidationNote> for ValidationNote {
    fn from(note: &ValidationNote) -> Self {
        note.clone()
    }
}
