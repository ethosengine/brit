use serde::{Deserialize, Serialize};

use super::note::{NoteBody, VersionedNote};
use crate::engine::{cid::BritCid, content_node::ContentNode, signing::Signed};

/// Records that an agent produced an output artifact from a manifest's inputs — the frozen
/// legacy (epoch 1) note. Its serializer is the signature preimage of every historical build
/// note: never change a field, its order of declaration, its name or its serde attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

impl Signed for BuildAttestationContentNode {
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

/// What a build note attests, signed in the envelope format (`brit.build@2`): the legacy node's
/// fields without `signature`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildBody {
    /// CID of the build manifest that was executed.
    pub manifest_cid: BritCid,
    /// Pipeline step name (e.g. `elohim-edge:cargo-build-storage`).
    pub step_name: String,
    /// Hash of all input files consumed by this build step.
    pub inputs_hash: String,
    /// CID of the artifact produced by this step.
    pub output_cid: BritCid,
    /// The signer's raw ed25519 public key (hex); the envelope key must be `ed 01 ‖` it.
    pub agent_id: String,
    /// Hardware profile of the build machine (arch, OS, memory, etc.).
    pub hardware_profile: serde_json::Value,
    /// Wall-clock duration of the build in milliseconds.
    pub build_duration_ms: u64,
    /// ISO-8601 timestamp when the build completed.
    pub built_at: String,
    /// Whether the build step succeeded.
    pub success: bool,
}

impl NoteBody for BuildBody {
    type Legacy = BuildAttestationContentNode;
    const FORMAT: &'static str = "brit.build@2";
    const DOMAIN: &'static [u8] = b"brit:build@2:";

    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn from_legacy(legacy: &BuildAttestationContentNode) -> Self {
        Self {
            manifest_cid: legacy.manifest_cid.clone(),
            step_name: legacy.step_name.clone(),
            inputs_hash: legacy.inputs_hash.clone(),
            output_cid: legacy.output_cid.clone(),
            agent_id: legacy.agent_id.clone(),
            hardware_profile: legacy.hardware_profile.clone(),
            build_duration_ms: legacy.build_duration_ms,
            built_at: legacy.built_at.clone(),
            success: legacy.success,
        }
    }
}

/// A build note in either format.
pub type BuildNote = VersionedNote<BuildBody>;

impl From<&BuildAttestationContentNode> for BuildNote {
    fn from(legacy: &BuildAttestationContentNode) -> Self {
        Self::Legacy(legacy.clone())
    }
}

impl From<&BuildNote> for BuildNote {
    fn from(note: &BuildNote) -> Self {
        note.clone()
    }
}
