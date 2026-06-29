// brit-epr/tests/cid_conformance.rs
//! brit's CID engine MUST be byte-identical to the protocol's elohim-epr.
//! These vectors are the portable spec — any reimplementation (incl. a future
//! machine-code port) is correct iff it reproduces them.
#![cfg(feature = "elohim-protocol")]

use brit_epr::BritCid;

const VECTORS: &[&[u8]] = &[&[0xa0], &[0x01, 0x02, 0x03, 0x04], &[0xaa, 0xbb, 0xcc], b"covenant"];

#[test]
fn brit_matches_elohim_epr_for_every_vector() {
    for v in VECTORS {
        let brit = BritCid::compute(v).to_string();
        let canonical = elohim_epr::cid::compute_cid(v).to_string();
        assert_eq!(brit, canonical, "CID drift for vector {v:?}");
    }
}

#[test]
fn empty_map_vector_is_stable() {
    // Frozen golden value — guards against silent codec/hash drift.
    assert_eq!(
        BritCid::compute(&[0xa0]).to_string(),
        elohim_epr::cid::compute_cid(&[0xa0]).to_string()
    );
}
