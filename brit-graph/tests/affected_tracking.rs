use brit_epr::{BritCid, ContentNode};
use brit_graph::affected::{AffectedBy, AffectedTracker, PropagationScope};
use brit_graph::graph::EprGraph;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestNode {
    name: String,
}

impl ContentNode for TestNode {
    fn content_type(&self) -> &'static str {
        "test.node"
    }
}

fn three_node_chain() -> (EprGraph<TestNode>, BritCid, BritCid, BritCid) {
    let mut graph = EprGraph::new();
    let a = TestNode { name: "aff-a".into() }; // depends on b
    let b = TestNode { name: "aff-b".into() }; // depends on c
    let c = TestNode { name: "aff-c".into() }; // leaf
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();
    let cid_c = c.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_node(c).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap();
    graph.add_edge(&cid_b, &cid_c).unwrap();

    (graph, cid_a, cid_b, cid_c)
}

#[test]
fn directly_affected_node_is_tracked() {
    let (graph, _, _, cid_c) = three_node_chain();
    let mut tracker = AffectedTracker::new(&graph);
    tracker.mark_affected(cid_c.clone(), AffectedBy::ChangedFile("src/lib.rs".into()));

    let affected = tracker.build();
    assert!(affected.is_affected(&cid_c));
    assert!(!affected.is_affected(&BritCid::compute(b"nonexistent")));
}

#[test]
fn upstream_propagation_deep() {
    let (graph, cid_a, cid_b, cid_c) = three_node_chain();
    // c changed -> b is affected (depends on c) -> a is affected (depends on b)
    let mut tracker = AffectedTracker::new(&graph);
    tracker.set_upstream_scope(PropagationScope::Deep);
    tracker.mark_affected(cid_c.clone(), AffectedBy::ChangedFile("leaf.rs".into()));
    tracker.propagate().unwrap();

    let affected = tracker.build();
    assert!(affected.is_affected(&cid_c));
    assert!(affected.is_affected(&cid_b));
    assert!(affected.is_affected(&cid_a));
}

#[test]
fn upstream_propagation_direct() {
    let (graph, cid_a, cid_b, cid_c) = three_node_chain();
    let mut tracker = AffectedTracker::new(&graph);
    tracker.set_upstream_scope(PropagationScope::Direct);
    tracker.mark_affected(cid_c.clone(), AffectedBy::ChangedFile("leaf.rs".into()));
    tracker.propagate().unwrap();

    let affected = tracker.build();
    assert!(affected.is_affected(&cid_c));
    assert!(affected.is_affected(&cid_b)); // direct dependent of c
    assert!(!affected.is_affected(&cid_a)); // NOT affected — only direct
}

#[test]
fn upstream_propagation_none() {
    let (graph, cid_a, cid_b, cid_c) = three_node_chain();
    let mut tracker = AffectedTracker::new(&graph);
    tracker.set_upstream_scope(PropagationScope::None);
    tracker.mark_affected(cid_c.clone(), AffectedBy::ChangedFile("leaf.rs".into()));
    tracker.propagate().unwrap();

    let affected = tracker.build();
    assert!(affected.is_affected(&cid_c));
    assert!(!affected.is_affected(&cid_b));
    assert!(!affected.is_affected(&cid_a));
}

#[test]
fn provenance_tracks_why() {
    let (graph, _, cid_b, cid_c) = three_node_chain();
    let mut tracker = AffectedTracker::new(&graph);
    tracker.set_upstream_scope(PropagationScope::Deep);
    tracker.mark_affected(cid_c.clone(), AffectedBy::ChangedFile("leaf.rs".into()));
    tracker.propagate().unwrap();

    let affected = tracker.build();
    let reasons = affected.reasons(&cid_b).unwrap();
    assert!(reasons.iter().any(|r| matches!(r, AffectedBy::UpstreamNode(_))));
}
