//! Topological planning — sort affected nodes into parallelizable levels.
//!
//! Level 0: nodes with no unmet dependencies (leaves).
//! Level 1: nodes whose dependencies are all in level 0.
//! And so on. Nodes within a level can execute in parallel.

use brit_epr::{BritCid, ContentNode};
use petgraph::Direction;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::{EprGraph, GraphError};

/// A topological execution plan grouped by dependency level.
#[derive(Debug, Clone)]
pub struct TopoPlan {
    /// Each inner vec is a set of nodes that can execute in parallel.
    /// levels[0] has no dependencies, levels[1] depends only on levels[0], etc.
    pub levels: Vec<Vec<BritCid>>,
}

impl TopoPlan {
    /// Build a topological plan from a set of affected CIDs within a graph.
    ///
    /// Only includes nodes that appear in `affected_cids`. Dependencies between
    /// affected nodes determine the level grouping. Dependencies on non-affected
    /// nodes are treated as already satisfied.
    pub fn from_affected<N: ContentNode, E>(
        graph: &EprGraph<N, E>,
        affected_cids: &[BritCid],
    ) -> Result<Self, GraphError> {
        if affected_cids.is_empty() {
            return Ok(TopoPlan { levels: vec![] });
        }

        let affected_set: FxHashSet<BritCid> = affected_cids.iter().cloned().collect();

        // Compute in-degree for each affected node (only counting edges to other affected nodes)
        // Outgoing edges = dependencies. In-degree here counts how many affected deps this node has.
        let mut in_degree: FxHashMap<BritCid, usize> = FxHashMap::default();
        for cid in &affected_set {
            let idx = graph.resolve_index(cid)?;
            let count = graph
                .inner_graph()
                .neighbors_directed(idx, Direction::Outgoing)
                .filter(|&neighbor| {
                    graph
                        .index_to_cid(neighbor)
                        .is_some_and(|c| affected_set.contains(&c))
                })
                .count();
            in_degree.insert(cid.clone(), count);
        }

        // Kahn's algorithm with level tracking
        let mut levels = Vec::new();
        let mut current_level: Vec<BritCid> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(cid, _)| cid.clone())
            .collect();

        while !current_level.is_empty() {
            let mut next_level: Vec<BritCid> = Vec::new();

            for cid in &current_level {
                let idx = graph.resolve_index(cid)?;
                // Find affected nodes that depend on this node (incoming edges = dependents)
                for neighbor in graph.inner_graph().neighbors_directed(idx, Direction::Incoming) {
                    if let Some(neighbor_cid) = graph.index_to_cid(neighbor) {
                        if let Some(deg) = in_degree.get_mut(&neighbor_cid) {
                            *deg = deg.saturating_sub(1);
                            if *deg == 0 {
                                next_level.push(neighbor_cid);
                            }
                        }
                    }
                }
            }

            levels.push(current_level);
            current_level = next_level;
        }

        Ok(TopoPlan { levels })
    }

    /// Total number of nodes across all levels.
    #[must_use]
    pub fn total_nodes(&self) -> usize {
        self.levels.iter().map(Vec::len).sum()
    }

    /// Flatten into a single ordered vec (level 0 first, then level 1, etc).
    #[must_use]
    pub fn flatten(&self) -> Vec<BritCid> {
        self.levels.iter().flat_map(|l| l.iter().cloned()).collect()
    }

    /// Whether the plan is empty (no affected nodes).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}
