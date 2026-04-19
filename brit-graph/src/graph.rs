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
    inner: DiGraph<NodeIndex, E>,
    cid_to_index: HashMap<BritCid, NodeIndex>,
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

impl<N: ContentNode, E: Default> EprGraph<N, E> {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self {
            inner: DiGraph::new(),
            cid_to_index: HashMap::new(),
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
        let graph_idx = self.inner.add_node(NodeIndex::new(data_idx));
        self.cid_to_index.insert(cid.clone(), graph_idx);
        Ok(cid)
    }

    /// Add a directed edge: `from` depends on `to`.
    pub fn add_edge(&mut self, from: &BritCid, to: &BritCid) -> Result<(), GraphError> {
        let from_idx = self.resolve_index(from)?;
        let to_idx = self.resolve_index(to)?;
        self.inner.add_edge(from_idx, to_idx, E::default());
        Ok(())
    }

    /// Get a node by CID.
    pub fn get_node(&self, cid: &BritCid) -> Result<&N, GraphError> {
        let graph_idx = self.resolve_index(cid)?;
        let data_idx = self.inner[graph_idx].index();
        Ok(&self.node_data[data_idx])
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.cid_to_index.len()
    }

    /// Check whether the graph contains any cycles.
    pub fn has_cycle(&self) -> bool {
        is_cyclic_directed(&self.inner)
    }

    /// Get all node CIDs.
    pub fn cids(&self) -> Vec<BritCid> {
        self.cid_to_index.keys().cloned().collect()
    }

    /// Check if a CID exists in the graph.
    pub fn contains(&self, cid: &BritCid) -> bool {
        self.cid_to_index.contains_key(cid)
    }

    /// Access the inner petgraph (for traits that need direct graph access).
    pub(crate) fn inner_graph(&self) -> &DiGraph<NodeIndex, E> {
        &self.inner
    }

    /// Resolve a CID to a petgraph NodeIndex.
    pub(crate) fn resolve_index(&self, cid: &BritCid) -> Result<NodeIndex, GraphError> {
        self.cid_to_index
            .get(cid)
            .copied()
            .ok_or_else(|| GraphError::NodeNotFound(cid.clone()))
    }

    /// Resolve a petgraph NodeIndex to a CID.
    pub(crate) fn index_to_cid(&self, idx: NodeIndex) -> Option<BritCid> {
        self.cid_to_index
            .iter()
            .find(|(_, &v)| v == idx)
            .map(|(k, _)| k.clone())
    }
}

impl<N: ContentNode, E: Default> Default for EprGraph<N, E> {
    fn default() -> Self {
        Self::new()
    }
}
