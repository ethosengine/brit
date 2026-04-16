//! `ContentNode` — trait for CID-addressed content objects stored locally.

use serde::{de::DeserializeOwned, Serialize};
use crate::engine::cid::BritCid;

/// A content-addressed node that can be serialized to canonical JSON and
/// stored in the local object store.
pub trait ContentNode: Serialize + DeserializeOwned {
    /// The content type discriminator, e.g. `"brit.build-attestation"`.
    fn content_type(&self) -> &'static str;

    /// Serialize to canonical JSON bytes (sorted keys for determinism).
    fn canonical_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        let value = serde_json::to_value(self)?;
        serde_json::to_vec(&value)
    }

    /// Compute the content identifier from the canonical JSON.
    fn compute_cid(&self) -> Result<BritCid, serde_json::Error> {
        let bytes = self.canonical_json()?;
        Ok(BritCid::compute(&bytes))
    }
}
