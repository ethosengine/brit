use serde::{Deserialize, Serialize};
use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

/// Records that an agent produced an output artifact from a manifest's inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildAttestationContentNode {
    /// CID of the build manifest that was executed.
    pub manifest_cid: BritCid,
    /// Pipeline step name (e.g. `elohim-edge:cargo-build-storage`).
    pub step_name: String,
    /// Hash of all input files consumed by this build step.
    pub inputs_hash: String,
    /// CID of the artifact produced by this step.
    pub output_cid: BritCid,
    /// Agent identifier (public key or agent DID).
    pub agent_id: String,
    /// Hardware profile of the build machine (arch, OS, memory, etc.).
    pub hardware_profile: serde_json::Value,
    /// Wall-clock duration of the build in milliseconds.
    pub build_duration_ms: u64,
    /// ISO-8601 timestamp when the build completed.
    pub built_at: String,
    /// Whether the build step succeeded.
    pub success: bool,
    /// Agent signature over the attestation payload.
    pub signature: String,
}

impl ContentNode for BuildAttestationContentNode {
    fn content_type(&self) -> &'static str {
        "brit.build-attestation"
    }
}
