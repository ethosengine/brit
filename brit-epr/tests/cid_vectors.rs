//! `BritCid` against the addressing kernel's conformance vectors.
//!
//! `fixtures/elohim-address/address-vectors.json` is a vendored copy of `elohim-address` 0.1.0's
//! `vectors/address-vectors.json` (monorepo commit `ed6cedcc56d6`, identical to the file inside
//! the published crate). Its sha2-256 is pinned below: a changed copy is a review event, not a
//! silent re-vendor. The positive rows were derived outside any Rust code (coreutils sha256sum,
//! @noble/hashes blake3, a hand-written CID encoder); the negative rows are byte-surface inputs
//! the full-decode parser must refuse.

use brit_epr::BritCid;
use serde_json::Value;

const VECTORS: &str = include_str!("fixtures/elohim-address/address-vectors.json");
/// sha2-256 of the vendored vector file.
const VECTORS_SHA256: &str = "b9e740aae6320f748288741274c21558dc4b047928471eedbb628c3b3b440527";

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("vector file parses")
}

fn rows<'a>(v: &'a Value, key: &str) -> &'a Vec<Value> {
    v[key].as_array().expect("rows are an array")
}

fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row[key].as_str().expect("row field is a string")
}

#[test]
fn vendored_vector_file_is_the_pinned_copy() {
    // The citation CID's digest is the file's sha2-256: one digest, two renderings.
    let cid = BritCid::compute_raw(VECTORS.as_bytes());
    assert_eq!(hex::encode(cid.as_cid().hash().digest()), VECTORS_SHA256);
}

#[test]
fn every_profile_matches_the_independent_oracle() {
    let v = vectors();
    let positive = rows(&v, "positive");
    assert_eq!(positive.len(), 3, "hello world, abc and the empty input");
    for row in positive {
        let input = text(row, "input").as_bytes();

        let bytes = BritCid::compute_bytes(input);
        assert_eq!(bytes.to_string(), text(row, "byteCid"));
        assert_eq!(hex::encode(bytes.as_cid().hash().digest()), text(row, "blake3"));
        assert_eq!(BritCid::parse_bytes(text(row, "byteCid")), Ok(bytes.clone()));
        assert_eq!(
            bytes.short_fingerprint(),
            None,
            "a BLAKE3 CID has no sha256: short form"
        );

        let cite = BritCid::compute_raw(input);
        assert_eq!(cite.to_string(), text(row, "citeCid"));
        assert_eq!(cite.short_fingerprint().as_deref(), Some(text(row, "shortFingerprint")));

        let record = BritCid::compute(input);
        assert_eq!(record.to_string(), text(row, "recordCid"));
        assert_eq!(
            record.short_fingerprint().as_deref(),
            Some(text(row, "shortFingerprint"))
        );

        // Each text form still parses as a CID, and the citation and record spellings are
        // refused on the byte surface.
        for (field, cid) in [("byteCid", &bytes), ("citeCid", &cite), ("recordCid", &record)] {
            assert_eq!(&text(row, field).parse::<BritCid>().unwrap(), cid, "{field}");
        }
        assert!(BritCid::parse_bytes(text(row, "citeCid")).is_err());
        assert!(BritCid::parse_bytes(text(row, "recordCid")).is_err());
    }
}

#[test]
fn negative_rows_are_refused_on_the_byte_surface() {
    let v = vectors();
    let negative = rows(&v, "negative");
    assert!(negative.len() >= 10);
    for row in negative {
        let (name, input) = (text(row, "name"), text(row, "input"));
        assert!(
            BritCid::parse_bytes(input).is_err(),
            "{name}: {input:?} was admitted as a byte address"
        );
    }
}
