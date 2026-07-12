//! The cite↔CID convergence pin: the drift fingerprint IS the short-form (first 16 hex of the
//! sha2-256 digest) of the canonical body's raw-codec CID. One digest, two renderings.
//! See `genesis/docs/superpowers/specs/2026-07-12-cite-fingerprint-cid-convergence-design.md`.

use brit_epr::engine::{canonical_body, drift_fingerprint, BritCid};

#[test]
fn drift_fingerprint_is_body_cid_short_form() {
    for content in [
        "---\nid: x\n---\nhello body\n",
        "no frontmatter, whole thing is body",
        "---\ntitle: T\nid: y\ncites:\n  - a | d | sha256:0000000000000000\n---\n\n  trimmed  \n\n",
        "---\nid: empty\n---",
    ] {
        let body = canonical_body(content);
        let cid = BritCid::compute_raw(body.as_bytes());
        assert_eq!(
            drift_fingerprint(content),
            cid.short_fingerprint(),
            "drift fingerprint must equal the canonical body CID short-form for {content:?}",
        );
    }
}

#[test]
fn body_cid_is_raw_codec_bafkrei() {
    // The body address is a RAW (0x55) CID — arbitrary body bytes, not a dag-cbor atom.
    let cid = BritCid::compute_raw(canonical_body("---\nid: x\n---\nbody").as_bytes());
    assert!(cid.to_string().starts_with("bafkrei"), "got {cid}");
}
