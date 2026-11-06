use serde::{Deserialize, Serialize};

use super::note::{NoteBody, VersionedNote};
use crate::engine::{cid::BritCid, content_node::ContentNode, signing::Signed};

/// Health status of a deployed service at attestation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Service is reachable and responding correctly.
    Healthy,
    /// Service is reachable but returning degraded responses.
    Degraded,
    /// Service did not respond within the health-check timeout.
    Unreachable,
}

/// Records that an agent confirms an artifact is live at an environment — the frozen legacy
/// (epoch 1) note; its serializer is the signature preimage of every historical deploy note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeployAttestationContentNode {
    /// CID of the artifact that was deployed.
    pub artifact_cid: BritCid,
    /// Pipeline step name (e.g. `elohim-edge:cargo-build-storage`).
    pub step_name: String,
    /// Human-readable label for the deployment environment (e.g. `staging`).
    pub environment_label: String,
    /// Base URL of the deployed service.
    pub endpoint: String,
    /// EPR reference for the liveness health check (e.g. `epr:{service-cid}/health`).
    /// Resolves through doorway when protocol-aware; degrades to no-op on stock git forges.
    pub health_check_epr: String,
    /// Health status observed at attestation time.
    pub health_status: HealthStatus,
    /// ISO-8601 timestamp when the artifact was deployed.
    pub deployed_at: String,
    /// ISO-8601 timestamp when the health check was performed.
    pub attested_at: String,
    /// Seconds before this attestation expires and a re-check is required.
    pub liveness_ttl_sec: u64,
    /// Agent identifier (public key or agent DID).
    pub agent_id: String,
    /// Agent signature over the attestation payload.
    pub signature: String,
}

impl ContentNode for DeployAttestationContentNode {
    fn content_type(&self) -> &'static str {
        "brit.deploy-attestation"
    }
}

impl Signed for DeployAttestationContentNode {
    fn signature(&self) -> &str {
        &self.signature
    }
    fn agent_id(&self) -> &str {
        &self.agent_id
    }
    fn without_signature(&self) -> Self {
        let mut c = self.clone();
        c.signature = String::new();
        c
    }
}

/// What a deploy note attests, signed in the envelope format (`brit.deploy@2`): the legacy
/// node's fields without `signature`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeployBody {
    /// CID of the artifact that was deployed.
    pub artifact_cid: BritCid,
    /// Pipeline step name (e.g. `elohim-edge:cargo-build-storage`).
    pub step_name: String,
    /// Human-readable label for the deployment environment (e.g. `staging`).
    pub environment_label: String,
    /// Base URL of the deployed service.
    pub endpoint: String,
    /// EPR reference for the liveness health check (e.g. `epr:{service-cid}/health`).
    pub health_check_epr: String,
    /// Health status observed at attestation time.
    pub health_status: HealthStatus,
    /// ISO-8601 timestamp when the artifact was deployed.
    pub deployed_at: String,
    /// ISO-8601 timestamp when the health check was performed.
    pub attested_at: String,
    /// Seconds before this attestation expires and a re-check is required.
    pub liveness_ttl_sec: u64,
    /// The signer's raw ed25519 public key (hex); the envelope key must be `ed 01 ‖` it.
    pub agent_id: String,
}

impl NoteBody for DeployBody {
    type Legacy = DeployAttestationContentNode;
    const FORMAT: &'static str = "brit.deploy@2";
    const DOMAIN: &'static [u8] = b"brit:deploy@2:";

    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn from_legacy(legacy: &DeployAttestationContentNode) -> Self {
        Self {
            artifact_cid: legacy.artifact_cid.clone(),
            step_name: legacy.step_name.clone(),
            environment_label: legacy.environment_label.clone(),
            endpoint: legacy.endpoint.clone(),
            health_check_epr: legacy.health_check_epr.clone(),
            health_status: legacy.health_status,
            deployed_at: legacy.deployed_at.clone(),
            attested_at: legacy.attested_at.clone(),
            liveness_ttl_sec: legacy.liveness_ttl_sec,
            agent_id: legacy.agent_id.clone(),
        }
    }
}

/// A deploy note in either format.
pub type DeployNote = VersionedNote<DeployBody>;

impl From<&DeployAttestationContentNode> for DeployNote {
    fn from(legacy: &DeployAttestationContentNode) -> Self {
        Self::Legacy(legacy.clone())
    }
}

impl From<&DeployNote> for DeployNote {
    fn from(note: &DeployNote) -> Self {
        note.clone()
    }
}
