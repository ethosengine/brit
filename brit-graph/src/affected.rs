//! Affected tracking — which nodes are affected and why.
//!
//! Given a set of initially-affected nodes (e.g., "this step's source files changed"),
//! propagate through the graph to find all transitively affected nodes.
//! Each affected node carries `Vec<AffectedBy>` explaining why it was affected.

use std::collections::VecDeque;

use brit_epr::{BritCid, ContentNode};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::{EprGraph, GraphError};
use crate::traits::GraphConnections;

/// Why a node was marked as affected.
#[derive(Debug, Clone)]
pub enum AffectedBy {
    /// A source file matched an input pattern.
    ChangedFile(String),
    /// A dependency (upstream in the DAG) was affected.
    UpstreamNode(BritCid),
    /// A dependent (downstream in the DAG) was affected.
    DownstreamNode(BritCid),
    /// The content fingerprint of inputs changed.
    InputFingerprint,
    /// Explicitly marked as always-affected.
    AlwaysAffected,
}

/// How far to propagate through the graph.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PropagationScope {
    /// Don't propagate beyond the initial set.
    None,
    /// Propagate to immediate neighbors only.
    Direct,
    /// Propagate through the full transitive closure.
    #[default]
    Deep,
}

/// Tracks which nodes are affected during graph analysis.
pub struct AffectedTracker<'g, N: ContentNode, E> {
    graph: &'g EprGraph<N, E>,
    affected: FxHashMap<BritCid, Vec<AffectedBy>>,
    upstream_scope: PropagationScope,
}

impl<'g, N: ContentNode, E> AffectedTracker<'g, N, E> {
    /// Create a new tracker for the given graph.
    pub fn new(graph: &'g EprGraph<N, E>) -> Self {
        Self {
            graph,
            affected: FxHashMap::default(),
            upstream_scope: PropagationScope::Deep,
        }
    }

    /// Set the upstream propagation scope (dependents of affected nodes).
    pub fn set_upstream_scope(&mut self, scope: PropagationScope) {
        self.upstream_scope = scope;
    }

    /// Mark a node as affected with a given reason.
    pub fn mark_affected(&mut self, cid: BritCid, reason: AffectedBy) {
        self.affected.entry(cid).or_default().push(reason);
    }

    /// Propagate affected state through the graph based on scope settings.
    ///
    /// "Upstream propagation" means: if node C is affected, and B depends on C,
    /// then B is affected too (B is upstream — it's a dependent of C).
    /// This matches build semantics: if a leaf changes, everything that
    /// depends on it needs rebuilding.
    pub fn propagate(&mut self) -> Result<(), GraphError> {
        match self.upstream_scope {
            PropagationScope::None => Ok(()),
            PropagationScope::Direct => self.propagate_direct(),
            PropagationScope::Deep => self.propagate_deep(),
        }
    }

    /// Consume the tracker and produce the final affected set.
    pub fn build(self) -> AffectedSet {
        AffectedSet {
            affected: self.affected,
        }
    }

    fn propagate_direct(&mut self) -> Result<(), GraphError> {
        let initial: Vec<BritCid> = self.affected.keys().cloned().collect();
        for cid in initial {
            let dependents = self.graph.dependents_of(&cid)?;
            for dep_cid in dependents {
                self.affected
                    .entry(dep_cid)
                    .or_default()
                    .push(AffectedBy::UpstreamNode(cid.clone()));
            }
        }
        Ok(())
    }

    fn propagate_deep(&mut self) -> Result<(), GraphError> {
        let mut queue: VecDeque<BritCid> = self.affected.keys().cloned().collect();
        let mut visited: FxHashSet<BritCid> = queue.iter().cloned().collect();

        while let Some(cid) = queue.pop_front() {
            let dependents = self.graph.dependents_of(&cid)?;
            for dep_cid in dependents {
                self.affected
                    .entry(dep_cid.clone())
                    .or_default()
                    .push(AffectedBy::UpstreamNode(cid.clone()));
                if visited.insert(dep_cid.clone()) {
                    queue.push_back(dep_cid);
                }
            }
        }
        Ok(())
    }
}

/// The result of affected tracking — an immutable set of affected nodes with reasons.
pub struct AffectedSet {
    affected: FxHashMap<BritCid, Vec<AffectedBy>>,
}

impl AffectedSet {
    /// Check if a node is affected.
    #[must_use]
    pub fn is_affected(&self, cid: &BritCid) -> bool {
        self.affected.contains_key(cid)
    }

    /// Get the reasons a node was affected. Returns None if not affected.
    #[must_use]
    pub fn reasons(&self, cid: &BritCid) -> Option<&[AffectedBy]> {
        self.affected.get(cid).map(Vec::as_slice)
    }

    /// Get all affected CIDs.
    #[must_use]
    pub fn affected_cids(&self) -> Vec<BritCid> {
        self.affected.keys().cloned().collect()
    }

    /// Number of affected nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.affected.len()
    }

    /// Whether the affected set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.affected.is_empty()
    }
}
