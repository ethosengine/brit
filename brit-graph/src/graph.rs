//! `EprGraph` — a content-addressed directed graph.
//!
//! Nodes implement `ContentNode` from brit-epr. Each node is indexed by its
//! `BritCid`. Edges represent dependencies: an edge from A to B means
//! "A depends on B" (B must complete before A).

use std::collections::HashMap;

use brit_epr::{BritCid, ContentNode};
use petgraph::algo::is_cyclic_directed;
use petgraph::graph::{DiGraph, NodeIndex};

/// A content-addressed directed graph where nodes implement `ContentNode`.
pub struct EprGraph<N: ContentNode, E = ()> {
    /// Petgraph directed graph. Node weight is the offset index into `node_data`.
    inner: DiGraph<usize, E>,
    /// Forward map: CID → petgraph NodeIndex.
    cid_to_index: HashMap<BritCid, NodeIndex>,
    /// Reverse map: petgraph NodeIndex → CID (O(1) lookup for traversal).
    index_to_cid_map: HashMap<NodeIndex, BritCid>,
    /// Parallel Vec of node payloads; addressed by the usize stored in `inner`.
    node_data: Vec<N>,
}

/// Errors from graph operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// A referenced node CID was not found in the graph.
    #[error("node not found: {0}")]
    NodeNotFound(BritCid),
    /// Failed to compute CID for a node.
    #[error("CID computation failed: {0}")]
    CidError(#[from] serde_json::Error),
}

impl<N: ContentNode, E> EprGraph<N, E> {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self {
            inner: DiGraph::new(),
            cid_to_index: HashMap::new(),
            index_to_cid_map: HashMap::new(),
            node_data: Vec::new(),
        }
    }

    /// Add a node. If a node with the same CID already exists, this is a no-op.
    /// Returns the CID of the node.
    pub fn add_node(&mut self, node: N) -> Result<BritCid, GraphError> {
        let cid = node.compute_cid()?;
        if self.cid_to_index.contains_key(&cid) {
            return Ok(cid);
        }
        let data_idx = self.node_data.len();
        self.node_data.push(node);
        let graph_idx = self.inner.add_node(data_idx);
        self.cid_to_index.insert(cid.clone(), graph_idx);
        self.index_to_cid_map.insert(graph_idx, cid.clone());
        Ok(cid)
    }

    /// Add a directed edge: `from` depends on `to`.
    /// No-op if the edge already exists (prevents parallel edges).
    pub fn add_edge(&mut self, from: &BritCid, to: &BritCid) -> Result<(), GraphError>
    where
        E: Default,
    {
        let from_idx = self.resolve_index(from)?;
        let to_idx = self.resolve_index(to)?;
        if self.inner.find_edge(from_idx, to_idx).is_none() {
            self.inner.add_edge(from_idx, to_idx, E::default());
        }
        Ok(())
    }

    /// Get a node by CID.
    pub fn get_node(&self, cid: &BritCid) -> Result<&N, GraphError> {
        let graph_idx = self.resolve_index(cid)?;
        let data_idx = self.inner[graph_idx];
        Ok(&self.node_data[data_idx])
    }

    /// Number of nodes in the graph.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.cid_to_index.len()
    }

    /// Check whether the graph contains any cycles.
    #[must_use]
    pub fn has_cycle(&self) -> bool {
        is_cyclic_directed(&self.inner)
    }

    /// Get all node CIDs.
    #[must_use]
    pub fn cids(&self) -> Vec<BritCid> {
        self.cid_to_index.keys().cloned().collect()
    }

    /// Check if a CID exists in the graph.
    #[must_use]
    pub fn contains(&self, cid: &BritCid) -> bool {
        self.cid_to_index.contains_key(cid)
    }

    /// Access the inner petgraph (for traits that need direct graph access).
    // Used by GraphConnections trait in Task 3.
    #[allow(dead_code)]
    pub(crate) fn inner_graph(&self) -> &DiGraph<usize, E> {
        &self.inner
    }

    /// Resolve a CID to a petgraph NodeIndex.
    pub(crate) fn resolve_index(&self, cid: &BritCid) -> Result<NodeIndex, GraphError> {
        self.cid_to_index
            .get(cid)
            .copied()
            .ok_or_else(|| GraphError::NodeNotFound(cid.clone()))
    }

    /// Resolve a petgraph NodeIndex to a CID — O(1) via reverse map.
    // Used by GraphConnections trait in Task 3.
    #[allow(dead_code)]
    pub(crate) fn index_to_cid(&self, idx: NodeIndex) -> Option<BritCid> {
        self.index_to_cid_map.get(&idx).cloned()
    }
}

impl<N: ContentNode, E: Default> Default for EprGraph<N, E> {
    fn default() -> Self {
        Self::new()
    }
}
