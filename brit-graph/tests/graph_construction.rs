use brit_epr::{BritCid, ContentNode};
use brit_graph::graph::EprGraph;
use brit_graph::traits::GraphConnections;
use serde::{Deserialize, Serialize};

/// A minimal ContentNode for testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestNode {
    name: String,
}

impl ContentNode for TestNode {
    fn content_type(&self) -> &'static str {
        "test.node"
    }
}

#[test]
fn add_and_retrieve_node() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let node = TestNode { name: "alpha".into() };
    let cid = node.compute_cid().unwrap();
    graph.add_node(node.clone()).unwrap();

    let retrieved = graph.get_node(&cid).unwrap();
    assert_eq!(retrieved.name, "alpha");
}

#[test]
fn add_edge_between_nodes() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "a".into() };
    let b = TestNode { name: "b".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap(); // a depends on b

    assert_eq!(graph.node_count(), 2);
}

#[test]
fn duplicate_node_is_idempotent() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let node = TestNode { name: "dup".into() };
    graph.add_node(node.clone()).unwrap();
    graph.add_node(node.clone()).unwrap();
    assert_eq!(graph.node_count(), 1);
}

#[test]
fn edge_to_missing_node_fails() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "a".into() };
    let cid_a = a.compute_cid().unwrap();
    let missing = BritCid::compute(b"does-not-exist");

    graph.add_node(a).unwrap();
    let result = graph.add_edge(&cid_a, &missing);
    assert!(result.is_err());
}

#[test]
fn has_cycle_detects_cycle() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "cycle-a".into() };
    let b = TestNode { name: "cycle-b".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap();
    graph.add_edge(&cid_b, &cid_a).unwrap();

    assert!(graph.has_cycle());
}

#[test]
fn no_cycle_in_valid_dag() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "dag-a".into() };
    let b = TestNode { name: "dag-b".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap(); // a -> b (a depends on b)

    assert!(!graph.has_cycle());
}

#[test]
fn dependencies_of_returns_direct_deps() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "tr-a".into() };
    let b = TestNode { name: "tr-b".into() };
    let c = TestNode { name: "tr-c".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();
    let cid_c = c.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_node(c).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap(); // a depends on b
    graph.add_edge(&cid_a, &cid_c).unwrap(); // a depends on c

    let deps = graph.dependencies_of(&cid_a).unwrap();
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&cid_b));
    assert!(deps.contains(&cid_c));
}

#[test]
fn dependents_of_returns_direct_dependents() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "dep-a".into() };
    let b = TestNode { name: "dep-b".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap(); // a depends on b

    let dependents = graph.dependents_of(&cid_b).unwrap();
    assert_eq!(dependents, vec![cid_a]);
}

#[test]
fn deep_dependencies_of_returns_transitive() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "deep-a".into() };
    let b = TestNode { name: "deep-b".into() };
    let c = TestNode { name: "deep-c".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();
    let cid_c = c.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_node(c).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap(); // a -> b
    graph.add_edge(&cid_b, &cid_c).unwrap(); // b -> c

    let deep = graph.deep_dependencies_of(&cid_a).unwrap();
    assert_eq!(deep.len(), 2);
    assert!(deep.contains(&cid_b));
    assert!(deep.contains(&cid_c));
}
