//! `ContentNode` — trait for CID-addressed content objects stored locally.

use serde::{de::DeserializeOwned, Serialize};
use crate::engine::cid::BritCid;

/// Recursively sort all object keys in a JSON value for canonical representation.
fn sort_json_keys(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: serde_json::Map<String, serde_json::Value> = map
                .into_iter()
                .map(|(k, v)| (k, sort_json_keys(v)))
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .collect();
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(sort_json_keys).collect())
        }
        other => other,
    }
}

/// A content-addressed node that can be serialized to canonical JSON and
/// stored in the local object store.
pub trait ContentNode: Serialize + DeserializeOwned {
    /// The content type discriminator, e.g. `"brit.build-attestation"`.
    fn content_type(&self) -> &'static str;

    /// Serialize to canonical JSON bytes with lexicographically sorted keys.
    ///
    /// Keys are sorted recursively at all nesting levels. This guarantees
    /// that the same logical content always produces the same byte sequence,
    /// regardless of struct field declaration order or serialization library
    /// internals.
    fn canonical_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        let value = serde_json::to_value(self)?;
        let sorted = sort_json_keys(value);
        serde_json::to_vec(&sorted)
    }

    /// Compute the content identifier from the canonical JSON.
    fn compute_cid(&self) -> Result<BritCid, serde_json::Error> {
        let bytes = self.canonical_json()?;
        Ok(BritCid::compute(&bytes))
    }
}
