use serde::{Deserialize, Serialize};
use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

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

/// Records that an agent confirms an artifact is live at an environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
