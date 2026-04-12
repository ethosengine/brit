//! `ElohimProtocolSchema` — the first-party `AppSchema` implementation.

use crate::elohim::pillar_trailers::TrailerKey;
use crate::engine::{AppSchema, TrailerSet, ValidationError};

/// Zero-sized implementor of [`AppSchema`] for the Elohim Protocol.
///
/// Instances are stateless. Typically you construct one like
/// `const SCHEMA: ElohimProtocolSchema = ElohimProtocolSchema;` and pass
/// by reference.
#[derive(Debug, Clone, Copy, Default)]
pub struct ElohimProtocolSchema;

const SUMMARY_KEYS: &[&str] = &["Lamad", "Shefa", "Qahal"];
const NODE_KEYS: &[&str] = &["Lamad-Node", "Shefa-Node", "Qahal-Node"];

impl AppSchema for ElohimProtocolSchema {
    fn id(&self) -> &'static str {
        "elohim-protocol/1.0.0"
    }

    fn owns_key(&self, key: &str) -> bool {
        SUMMARY_KEYS.contains(&key) || NODE_KEYS.contains(&key)
    }

    fn required_keys(&self) -> &'static [&'static str] {
        SUMMARY_KEYS
    }

    fn cid_bearing_keys(&self) -> &'static [&'static str] {
        NODE_KEYS
    }

    fn validate_pair(&self, key: &str, value: &str) -> Result<(), ValidationError> {
        if !self.owns_key(key) {
            return Ok(()); // not our key; ignore
        }
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyValue(key.to_string()));
        }
        // Phase 1: no additional format checks. Phase 2 adds CID parsing on
        // NODE_KEYS.
        Ok(())
    }

    fn validate_set(&self, trailers: &TrailerSet) -> Result<(), ValidationError> {
        // Check required keys are present in canonical order so the error
        // always names Lamad before Shefa before Qahal.
        for key in TrailerKey::all() {
            let summary = key.summary_token();
            match trailers.get(summary) {
                None => return Err(ValidationError::MissingKey(summary.to_string())),
                Some(v) if v.trim().is_empty() => {
                    return Err(ValidationError::EmptyValue(summary.to_string()))
                }
                Some(_) => {}
            }
        }
        Ok(())
    }
}
