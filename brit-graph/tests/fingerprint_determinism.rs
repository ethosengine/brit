use brit_epr::BritCid;
use brit_graph::fingerprint::ContentFingerprint;
use std::collections::BTreeMap;

#[test]
fn same_inputs_same_fingerprint() {
    let mut inputs = BTreeMap::new();
    inputs.insert("file_a".to_string(), b"content_a".to_vec());
    inputs.insert("file_b".to_string(), b"content_b".to_vec());

    let fp1 = ContentFingerprint::compute(&inputs);
    let fp2 = ContentFingerprint::compute(&inputs);
    assert_eq!(fp1.cid, fp2.cid);
}

#[test]
fn different_inputs_different_fingerprint() {
    let mut inputs1 = BTreeMap::new();
    inputs1.insert("file".to_string(), b"v1".to_vec());

    let mut inputs2 = BTreeMap::new();
    inputs2.insert("file".to_string(), b"v2".to_vec());

    let fp1 = ContentFingerprint::compute(&inputs1);
    let fp2 = ContentFingerprint::compute(&inputs2);
    assert_ne!(fp1.cid, fp2.cid);
}

#[test]
fn insertion_order_does_not_matter() {
    let mut inputs1 = BTreeMap::new();
    inputs1.insert("z_file".to_string(), b"z_content".to_vec());
    inputs1.insert("a_file".to_string(), b"a_content".to_vec());

    let mut inputs2 = BTreeMap::new();
    inputs2.insert("a_file".to_string(), b"a_content".to_vec());
    inputs2.insert("z_file".to_string(), b"z_content".to_vec());

    let fp1 = ContentFingerprint::compute(&inputs1);
    let fp2 = ContentFingerprint::compute(&inputs2);
    assert_eq!(fp1.cid, fp2.cid);
}

#[test]
fn empty_inputs_produce_valid_fingerprint() {
    let inputs = BTreeMap::new();
    let fp = ContentFingerprint::compute(&inputs);
    assert_eq!(fp.cid.as_str().len(), 64);
}

#[test]
fn individual_input_cids_are_populated() {
    let mut inputs = BTreeMap::new();
    inputs.insert("alpha".to_string(), b"a".to_vec());
    inputs.insert("beta".to_string(), b"b".to_vec());

    let fp = ContentFingerprint::compute(&inputs);
    assert_eq!(fp.inputs.len(), 2);
    assert!(fp.inputs.contains_key("alpha"));
    assert!(fp.inputs.contains_key("beta"));
    // Each individual CID should be the BritCid::compute of the bytes
    assert_eq!(fp.inputs["alpha"], BritCid::compute(b"a"));
    assert_eq!(fp.inputs["beta"], BritCid::compute(b"b"));
}
