//! `BritCid` — content identifier: a self-describing CIDv1.
//!
//! Three profiles declared by the Rust addressing kernel (`elohim-address`).
//! Streaming readers preserve the same addresses without retaining the input:
//!
//! | Plane | Constructor | Codec | Multihash | Renders as |
//! |---|---|---|---|---|
//! | records (canonical DAG-CBOR nodes) | [`BritCid::compute`] | dag-cbor `0x71` | sha2-256 | `bafyrei…` |
//! | bytes (files, manifests, artifacts) | [`BritCid::compute_bytes`] | raw `0x55` | BLAKE3-256 | `bafkr4i…` |
//! | citations (cite fingerprints over canonical bodies) | [`BritCid::compute_raw`] | raw `0x55` | sha2-256 | `bafkrei…` |
//!
//! A CID names its own multihash, so a historical `bafkrei…` address stays verifiable under
//! its own profile; no cross-profile equality is promised.

use std::{fmt, str::FromStr};

use cid::Cid;
use multihash_codetable::{Code, MultihashDigest};
use serde::{Deserialize, Serialize};

/// A content identifier — a CIDv1 under one of the three profiles above.
///
/// Displayed and parsed as base32 (`bafyrei…` records, `bafkr4i…` bytes, `bafkrei…` citations).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BritCid(Cid);

impl BritCid {
    /// Compute a record CID (dag-cbor `0x71` + sha2-256) over already-canonical DAG-CBOR bytes.
    pub fn compute(canonical_bytes: &[u8]) -> Self {
        Self(elohim_address::compute_record_cid(canonical_bytes))
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
        Ok(Self(Cid::new_v1(0x71, mh)))
    }

    /// Address payload bytes (a file, a manifest, an artifact) under the epoch-0 byte profile:
    /// raw `0x55` + BLAKE3-256 (`bafkr4i…`).
    pub fn compute_bytes(bytes: &[u8]) -> Self {
        Self(elohim_address::compute_bytes_cid(bytes).into())
    }

    /// Address payload bytes without retaining them; I/O and cancellation errors propagate.
    pub fn compute_bytes_reader(input: impl std::io::Read) -> std::io::Result<Self> {
        let mut hash = blake3::Hasher::new();
        hash.update_reader(input)?;
        Ok(Self(
            elohim_address::ByteCid::from_digest(*hash.finalize().as_bytes()).into(),
        ))
    }

    /// Address a canonical body under the **citation** profile: raw `0x55` + sha2-256
    /// (`bafkrei…`). Citations only — the cite fingerprint is this CID's [`short_fingerprint`];
    /// payload bytes are addressed by [`BritCid::compute_bytes`].
    ///
    /// [`short_fingerprint`]: BritCid::short_fingerprint
    pub fn compute_raw(bytes: &[u8]) -> Self {
        Self(elohim_address::compute_raw_sha2_cid(bytes))
    }

    /// Admit a byte-profile address from text by the kernel's full decode (`bafkr4i…`, 59
    /// characters, header `01 55 1e 20`, canonical padding and re-encoding). Any other spelling
    /// — a `bafkrei…` citation, a record CID, a malformed header — is refused.
    pub fn parse_bytes(text: &str) -> Result<Self, CidParseError> {
        elohim_address::ByteCid::parse(text)
            .map(|cid| Self(cid.into()))
            .map_err(|e| CidParseError(e.to_string()))
    }

    /// Borrow the underlying multiformats CID.
    pub fn as_cid(&self) -> &Cid {
        &self.0
    }

    /// The cite **short-form**: `"sha256:" + hex(multihash digest)[:16]`, for sha2-256 CIDs
    /// only. For a [`compute_raw`] CID over a canonical body it equals the doc's
    /// [`drift_fingerprint`] exactly: one sha2-256 digest, two renderings (full CID `bafkrei…`
    /// ↔ `sha256:hex16`). `None` for any other multihash — a BLAKE3 byte CID has no `sha256:`
    /// short form, and labelling one would be a lie.
    ///
    /// [`drift_fingerprint`]: crate::engine::frontmatter::drift_fingerprint
    /// [`compute_raw`]: BritCid::compute_raw
    pub fn short_fingerprint(&self) -> Option<String> {
        elohim_address::short_fingerprint(&self.0).ok()
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
    fn cite_cid_is_bafkrei() {
        let cid = BritCid::compute_raw(b"hello world");
        assert!(cid.to_string().starts_with("bafkrei"), "got {cid}");
    }

    #[test]
    fn byte_cid_is_bafkr4i_and_has_no_sha256_short_form() {
        let cid = BritCid::compute_bytes(b"hello world");
        assert!(cid.to_string().starts_with("bafkr4i"), "got {cid}");
        assert_eq!(cid.to_string().len(), 59);
        assert_eq!(cid.short_fingerprint(), None);
        assert_eq!(BritCid::parse_bytes(&cid.to_string()), Ok(cid));
    }

    #[test]
    fn parse_bytes_refuses_other_profiles() {
        assert!(BritCid::parse_bytes(&BritCid::compute_raw(b"x").to_string()).is_err());
        assert!(BritCid::parse_bytes(&BritCid::compute(b"x").to_string()).is_err());
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
            assert_eq!(
                BritCid::compute_bytes_reader(bytes.as_slice())?,
                BritCid::compute_bytes(&bytes)
            );
        }
        struct Failure;
        impl Read for Failure {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(Error::other("input failed"))
            }
        }
        let input = (&b"valid prefix"[..]).chain(Failure);
        assert_eq!(BritCid::compute_reader(input).unwrap_err().to_string(), "input failed");
        let input = (&b"valid prefix"[..]).chain(Failure);
        assert_eq!(
            BritCid::compute_bytes_reader(input)
                .expect_err("reader fails")
                .to_string(),
            "input failed"
        );
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
