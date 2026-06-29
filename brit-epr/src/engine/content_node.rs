//! `ContentNode` — trait for CID-addressed content objects stored locally.

use serde::{de::DeserializeOwned, Serialize};

use crate::engine::cid::BritCid;

/// A content-addressed node: serialized to canonical DAG-CBOR and stored in
/// the local object store, identified by the CIDv1 of those bytes.
pub trait ContentNode: Serialize + DeserializeOwned {
    /// The content type discriminator, e.g. `"brit.epr-meta"`.
    fn content_type(&self) -> &'static str;

    /// Serialize to canonical DAG-CBOR bytes (RFC 8949 §4.2.1 deterministic:
    /// sorted keys, shortest-form ints, no indefinite-length items).
    fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        serde_ipld_dagcbor::to_vec(self).map_err(|e| CborError(e.to_string()))
    }

    /// Compute the CIDv1 (dag-cbor) over the canonical bytes.
    fn compute_cid(&self) -> Result<BritCid, CborError> {
        Ok(BritCid::compute(&self.canonical_bytes()?))
    }
}

/// Error encoding a node to canonical DAG-CBOR.
#[derive(Debug, thiserror::Error)]
#[error("dag-cbor encode error: {0}")]
pub struct CborError(String);
