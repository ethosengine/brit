use brit_epr::engine::cid::BritCid;
use brit_epr::engine::content_node::ContentNode;
use brit_epr::engine::object_store::LocalObjectStore;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestNode {
    name: String,
    value: u32,
}

impl ContentNode for TestNode {
    fn content_type(&self) -> &'static str {
        "test.node"
    }
}

#[test]
fn put_then_get_roundtrips() {
    let tmp = TempDir::new().unwrap();
    let store = LocalObjectStore::new(tmp.path().join("objects"));
    let node = TestNode { name: "hello".into(), value: 42 };
    let cid = store.put(&node).unwrap();
    let back: TestNode = store.get(&cid).unwrap();
    assert_eq!(node, back);
}

#[test]
fn same_content_same_cid() {
    let tmp = TempDir::new().unwrap();
    let store = LocalObjectStore::new(tmp.path().join("objects"));
    let node = TestNode { name: "deterministic".into(), value: 7 };
    let cid1 = store.put(&node).unwrap();
    let cid2 = store.put(&node).unwrap();
    assert_eq!(cid1, cid2);
}

#[test]
fn get_missing_cid_returns_error() {
    let tmp = TempDir::new().unwrap();
    let store = LocalObjectStore::new(tmp.path().join("objects"));
    let fake_cid = BritCid::compute(b"does not exist");
    let result = store.get::<TestNode>(&fake_cid);
    assert!(result.is_err());
}

#[test]
fn list_returns_all_stored_cids() {
    let tmp = TempDir::new().unwrap();
    let store = LocalObjectStore::new(tmp.path().join("objects"));
    let a = store.put(&TestNode { name: "a".into(), value: 1 }).unwrap();
    let b = store.put(&TestNode { name: "b".into(), value: 2 }).unwrap();
    let mut cids = store.list().unwrap();
    cids.sort_by(|x, y| x.as_str().cmp(y.as_str()));
    let mut expected = vec![a, b];
    expected.sort_by(|x, y| x.as_str().cmp(y.as_str()));
    assert_eq!(cids, expected);
}
