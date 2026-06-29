//! The node-root rollup: composes every EprMeta in a node into one
//! content-addressed lockfile (the import/export contract anchor).
//!
//! Source of truth: the canonical DAG-CBOR bytes (git object store);
//! identity is the CIDv1 of those bytes.

use serde::{Deserialize, Serialize};

use crate::engine::{cid::BritCid, content_node::ContentNode};

/// The node-level seed/lockfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSeed {
    /// Schema version.
    pub epr_meta_version: u32,
    /// Repository / node identifier.
    pub repo: String,
    /// CID links to every EprMeta in the node, sorted for determinism
    /// (real IPLD links, not string FKs).
    pub epr_metas: Vec<BritCid>,
}

impl ContentNode for NodeSeed {
    fn content_type(&self) -> &'static str {
        "brit.node-seed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::content_node::ContentNode;

    #[test]
    fn content_type_is_stable() {
        let s = NodeSeed {
            epr_meta_version: 1,
            repo: "brit".into(),
            epr_metas: vec![],
        };
        assert_eq!(s.content_type(), "brit.node-seed");
    }

    #[test]
    fn order_independent_via_sorted_field() {
        use crate::engine::cid::BritCid;
        // Caller sorts epr_metas; identical sets → identical CID.
        let (x, y) = (BritCid::compute_raw(b"x"), BritCid::compute_raw(b"y"));
        let a = NodeSeed {
            epr_meta_version: 1,
            repo: "brit".into(),
            epr_metas: vec![x.clone(), y.clone()],
        };
        let b = NodeSeed {
            epr_meta_version: 1,
            repo: "brit".into(),
            epr_metas: vec![x, y],
        };
        assert_eq!(a.compute_cid().unwrap(), b.compute_cid().unwrap());
    }
}
