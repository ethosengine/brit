//! `BritCid` — content identifier: CIDv1 over canonical bytes.
//!
//! Nodes use multicodec 0x71 (dag-cbor); raw blobs use 0x55 (raw).
//! Multihash is 0x12 (sha2-256). Byte-identical to the protocol's
//! `elohim-epr` codec. BLAKE3 is for non-address fingerprints only.

use std::{fmt, str::FromStr};

use cid::Cid;
use multihash_codetable::{Code, MultihashDigest};
use serde::{Deserialize, Serialize};

/// Multicodec for dag-cbor content (the IPLD multicodec table).
const DAG_CBOR_CODEC: u64 = 0x71;
/// Multicodec for raw bytes.
const RAW_CODEC: u64 = 0x55;

/// A content identifier — a CIDv1 wrapping the sha2-256 of canonical bytes.
///
/// Displayed and parsed as base32 (`bafyrei…` for dag-cbor, `bafkrei…` for raw).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BritCid(Cid);

impl BritCid {
    /// Compute a dag-cbor CID (codec 0x71) over already-canonical DAG-CBOR bytes.
    pub fn compute(canonical_bytes: &[u8]) -> Self {
        let mh = Code::Sha2_256.digest(canonical_bytes);
        Self(Cid::new_v1(DAG_CBOR_CODEC, mh))
    }

    /// Compute the same dag-cbor CID as [`Self::compute`] without retaining the input.
    /// Readers can provide cancellation and propagate I/O failures between bounded reads.
    pub fn compute_reader(mut input: impl std::io::Read) -> std::io::Result<Self> {
        use sha2::Digest;

        let mut hash = sha2::Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let amount = match input.read(&mut buffer) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => result?,
            };
            if amount == 0 {
                break;
            }
            hash.update(&buffer[..amount]);
        }
        let mh = Code::Sha2_256
            .wrap(&hash.finalize())
            .expect("SHA-256 produces a supported 32-byte multihash");
        Ok(Self(Cid::new_v1(DAG_CBOR_CODEC, mh)))
    }

    /// Compute a raw-blob CID (codec 0x55) over arbitrary file bytes.
    pub fn compute_raw(bytes: &[u8]) -> Self {
        let mh = Code::Sha2_256.digest(bytes);
        Self(Cid::new_v1(RAW_CODEC, mh))
    }

    /// Borrow the underlying multiformats CID.
    pub fn as_cid(&self) -> &Cid {
        &self.0
    }

    /// The cite **short-form**: `"sha256:" + hex(multihash digest)[:16]`. The same first-16-hex of
    /// sha2-256 that [`drift_fingerprint`] emits — a short projection of THIS CID's digest. For a
    /// [`compute_raw`] CID over a canonical body, it equals the doc's `drift_fingerprint` exactly:
    /// one sha2-256 digest, two renderings (full CID `bafkrei…` ↔ `sha256:hex16`). Mirrors
    /// `eprfs_core::BlobCid::short_fingerprint`.
    ///
    /// [`drift_fingerprint`]: crate::engine::frontmatter::drift_fingerprint
    /// [`compute_raw`]: BritCid::compute_raw
    pub fn short_fingerprint(&self) -> String {
        let hex = hex::encode(self.0.hash().digest());
        format!("sha256:{}", &hex[..16])
    }
}

impl fmt::Display for BritCid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // cid's Display is base32 lowercase by default.
        write!(f, "{}", self.0)
    }
}

impl FromStr for BritCid {
    type Err = CidParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Cid::from_str(s).map(Self).map_err(|e| CidParseError(e.to_string()))
    }
}

/// Error parsing a CID string.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid CID: {0}")]
pub struct CidParseError(String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cbor_map_is_bafyrei() {
        // The elohim-epr golden vector: 0xa0 (empty map) → dag-cbor CIDv1.
        let cid = BritCid::compute(&[0xa0]);
        assert!(cid.to_string().starts_with("bafyrei"), "got {cid}");
    }

    #[test]
    fn raw_blob_is_bafkrei() {
        let cid = BritCid::compute_raw(b"hello world");
        assert!(cid.to_string().starts_with("bafkrei"), "got {cid}");
    }

    #[test]
    fn compute_is_deterministic() {
        assert_eq!(BritCid::compute(&[1, 2, 3]), BritCid::compute(&[1, 2, 3]));
    }

    #[test]
    fn reader_matches_buffered_cids_and_propagates_failure() -> std::io::Result<()> {
        use std::io::{Error, Read};

        for size in [0, 1, 65535, 65536, 65537, 200000] {
            let bytes: Vec<_> = (0..size).map(|n| (n % 251) as u8).collect();
            assert_eq!(BritCid::compute_reader(bytes.as_slice())?, BritCid::compute(&bytes));
        }
        struct Failure;
        impl Read for Failure {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(Error::other("input failed"))
            }
        }
        let input = (&b"valid prefix"[..]).chain(Failure);
        assert_eq!(BritCid::compute_reader(input).unwrap_err().to_string(), "input failed");
        Ok(())
    }

    #[test]
    fn different_input_different_cid() {
        assert_ne!(BritCid::compute(&[1]), BritCid::compute(&[2]));
    }

    #[test]
    fn roundtrip_display_parse() {
        let cid = BritCid::compute(&[0xa0]);
        let parsed: BritCid = cid.to_string().parse().unwrap();
        assert_eq!(cid, parsed);
    }

    #[test]
    fn rejects_non_cid_string() {
        assert!("not-a-cid".parse::<BritCid>().is_err());
    }

    #[test]
    fn serde_roundtrip_json() {
        let cid = BritCid::compute(&[0xa0]);
        let json = serde_json::to_string(&cid).unwrap();
        let back: BritCid = serde_json::from_str(&json).unwrap();
        assert_eq!(cid, back);
    }

    #[test]
    fn brit_cids_sort_deterministically() {
        let mut v = [BritCid::compute(&[2]), BritCid::compute(&[1])];
        v.sort();
        assert!(v[0] <= v[1]);
    }
}
