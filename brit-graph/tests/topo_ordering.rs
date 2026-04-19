use brit_epr::{BritCid, ContentNode};
use brit_graph::graph::EprGraph;
use brit_graph::topo::TopoPlan;
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

#[test]
fn topo_plan_groups_by_level() {
    // c has no deps (level 0)
    // b depends on c (level 1)
    // a depends on b (level 2)
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "topo-a".into() };
    let b = TestNode { name: "topo-b".into() };
    let c = TestNode { name: "topo-c".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();
    let cid_c = c.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_node(c).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap();
    graph.add_edge(&cid_b, &cid_c).unwrap();

    let affected = vec![cid_a.clone(), cid_b.clone(), cid_c.clone()];
    let plan = TopoPlan::from_affected(&graph, &affected).unwrap();

    assert_eq!(plan.levels.len(), 3);
    assert!(plan.levels[0].contains(&cid_c)); // leaf first
    assert!(plan.levels[1].contains(&cid_b));
    assert!(plan.levels[2].contains(&cid_a));
}

#[test]
fn topo_plan_parallel_at_same_level() {
    // b and c have no deps (level 0, parallelizable)
    // a depends on both b and c (level 1)
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "par-a".into() };
    let b = TestNode { name: "par-b".into() };
    let c = TestNode { name: "par-c".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();
    let cid_c = c.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_node(c).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap();
    graph.add_edge(&cid_a, &cid_c).unwrap();

    let affected = vec![cid_a.clone(), cid_b.clone(), cid_c.clone()];
    let plan = TopoPlan::from_affected(&graph, &affected).unwrap();

    assert_eq!(plan.levels.len(), 2);
    assert_eq!(plan.levels[0].len(), 2); // b and c at level 0
    assert!(plan.levels[0].contains(&cid_b));
    assert!(plan.levels[0].contains(&cid_c));
    assert_eq!(plan.levels[1], vec![cid_a]); // a at level 1
}

#[test]
fn topo_plan_skips_unaffected() {
    let mut graph: EprGraph<TestNode> = EprGraph::new();
    let a = TestNode { name: "skip-a".into() };
    let b = TestNode { name: "skip-b".into() };
    let c = TestNode { name: "skip-c".into() };
    let cid_a = a.compute_cid().unwrap();
    let cid_b = b.compute_cid().unwrap();
    let cid_c = c.compute_cid().unwrap();

    graph.add_node(a).unwrap();
    graph.add_node(b).unwrap();
    graph.add_node(c).unwrap();
    graph.add_edge(&cid_a, &cid_b).unwrap();

    // Only b is affected, not c (c is independent)
    let affected = vec![cid_b.clone()];
    let plan = TopoPlan::from_affected(&graph, &affected).unwrap();

    let all_cids: Vec<&BritCid> = plan.levels.iter().flat_map(|l: &Vec<BritCid>| l.iter()).collect();
    assert!(all_cids.contains(&&cid_b));
    assert!(!all_cids.contains(&&cid_c));
}

#[test]
fn topo_plan_empty_affected_produces_empty_plan() {
    let graph: EprGraph<TestNode> = EprGraph::new();
    let affected: Vec<BritCid> = vec![];
    let plan = TopoPlan::from_affected(&graph, &affected).unwrap();
    assert!(plan.levels.is_empty());
}
