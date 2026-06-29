//! Proves the `elohim` registry resolves and the published codec is callable.
#![cfg(feature = "elohim-protocol")]

#[test]
fn elohim_epr_compute_cid_is_dag_cbor() {
    // Empty CBOR map (0xa0) → CIDv1 dag-cbor sha2-256 → base32 "bafyrei…".
    let cid = elohim_epr::cid::compute_cid(&[0xa0]);
    assert!(cid.to_string().starts_with("bafyrei"), "got {cid}");
}
