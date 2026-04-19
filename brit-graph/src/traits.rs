//! Graph traversal traits — dependencies_of, dependents_of, and deep variants.

use std::collections::VecDeque;

use brit_epr::{BritCid, ContentNode};
use petgraph::Direction;
use rustc_hash::FxHashSet;

use crate::graph::{EprGraph, GraphError};

/// Trait for querying graph relationships.
pub trait GraphConnections<N: ContentNode> {
    /// Direct dependencies of a node (outgoing edges).
    fn dependencies_of(&self, cid: &BritCid) -> Result<Vec<BritCid>, GraphError>;

    /// Direct dependents of a node (incoming edges).
    fn dependents_of(&self, cid: &BritCid) -> Result<Vec<BritCid>, GraphError>;

    /// All transitive dependencies (deep).
    fn deep_dependencies_of(&self, cid: &BritCid) -> Result<Vec<BritCid>, GraphError>;

    /// All transitive dependents (deep).
    fn deep_dependents_of(&self, cid: &BritCid) -> Result<Vec<BritCid>, GraphError>;
}

impl<N: ContentNode, E> GraphConnections<N> for EprGraph<N, E> {
    fn dependencies_of(&self, cid: &BritCid) -> Result<Vec<BritCid>, GraphError> {
        let idx = self.resolve_index(cid)?;
        let graph = self.inner_graph();
        Ok(graph
            .neighbors_directed(idx, Direction::Outgoing)
            .filter_map(|neighbor| self.index_to_cid(neighbor))
            .collect())
    }

    fn dependents_of(&self, cid: &BritCid) -> Result<Vec<BritCid>, GraphError> {
        let idx = self.resolve_index(cid)?;
        let graph = self.inner_graph();
        Ok(graph
            .neighbors_directed(idx, Direction::Incoming)
            .filter_map(|neighbor| self.index_to_cid(neighbor))
            .collect())
    }

    fn deep_dependencies_of(&self, cid: &BritCid) -> Result<Vec<BritCid>, GraphError> {
        self.traverse_deep(cid, Direction::Outgoing)
    }

    fn deep_dependents_of(&self, cid: &BritCid) -> Result<Vec<BritCid>, GraphError> {
        self.traverse_deep(cid, Direction::Incoming)
    }
}

impl<N: ContentNode, E> EprGraph<N, E> {
    fn traverse_deep(
        &self,
        start: &BritCid,
        direction: Direction,
    ) -> Result<Vec<BritCid>, GraphError> {
        let start_idx = self.resolve_index(start)?;
        let graph = self.inner_graph();
        let mut visited = FxHashSet::default();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        for neighbor in graph.neighbors_directed(start_idx, direction) {
            queue.push_back(neighbor);
        }

        while let Some(idx) = queue.pop_front() {
            if !visited.insert(idx) {
                continue;
            }
            if let Some(cid) = self.index_to_cid(idx) {
                result.push(cid);
            }
            for neighbor in graph.neighbors_directed(idx, direction) {
                if !visited.contains(&neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }

        Ok(result)
    }
}
