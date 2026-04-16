//! `BritCid` — content identifier based on BLAKE3 hashing.
//!
//! Phase 2a uses a simplified CID: the BLAKE3 hash of the canonical JSON
//! serialization of a ContentNode. Full multiformats CIDv1 comes in a later
//! phase when interop with IPFS/Holochain requires it.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A content identifier — the BLAKE3 hash of a content payload.
///
/// Displayed and parsed as a 64-character lowercase hex string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BritCid(String);

impl BritCid {
    /// Compute a CID from arbitrary bytes.
    pub fn compute(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Self(hash.to_hex().to_string())
    }

    /// Return the hex string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BritCid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for BritCid {
    type Err = CidParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 {
            return Err(CidParseError::InvalidLength(s.len()));
        }
        if !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CidParseError::InvalidHex);
        }
        Ok(Self(s.to_lowercase()))
    }
}

/// Errors when parsing a CID string.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CidParseError {
    /// Expected 64 hex characters.
    #[error("expected 64 hex characters, got {0}")]
    InvalidLength(usize),
    /// Non-hex character found.
    #[error("CID contains non-hex characters")]
    InvalidHex,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_is_deterministic() {
        let a = BritCid::compute(b"hello world");
        let b = BritCid::compute(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn different_input_different_cid() {
        let a = BritCid::compute(b"hello");
        let b = BritCid::compute(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn roundtrip_display_parse() {
        let cid = BritCid::compute(b"test data");
        let parsed: BritCid = cid.to_string().parse().unwrap();
        assert_eq!(cid, parsed);
    }

    #[test]
    fn rejects_short_string() {
        let result = "abc123".parse::<BritCid>();
        assert_eq!(result, Err(CidParseError::InvalidLength(6)));
    }

    #[test]
    fn serde_roundtrip() {
        let cid = BritCid::compute(b"serde test");
        let json = serde_json::to_string(&cid).unwrap();
        let back: BritCid = serde_json::from_str(&json).unwrap();
        assert_eq!(cid, back);
    }
}
