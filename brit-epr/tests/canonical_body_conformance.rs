//! brit's canonical-body fingerprint MUST reproduce the parent oracle's
//! `cite_graph.fingerprint` byte-for-byte. The committed vectors are the
//! portable spec (regenerate with tests/fixtures/conformance/gen.py).
use std::{collections::BTreeMap, fs, path::Path};

#[test]
fn drift_fingerprint_matches_oracle_vectors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance");
    let expected: BTreeMap<String, String> =
        serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    assert!(!expected.is_empty(), "no conformance vectors found");
    for (name, want) in &expected {
        let content = fs::read_to_string(dir.join(name)).unwrap();
        let got = brit_epr::engine::frontmatter::drift_fingerprint(&content);
        assert_eq!(&got, want, "drift mismatch for {name}");
    }
}
