//! Canonical, content-addressed governance+seed manifest for a directory subtree.
//! The next-generation successor to a directory-local `.epr-meta`.
//!
//! Source of truth: the canonical DAG-CBOR bytes (stored in the git object
//! store); identity is the CIDv1 of those bytes. Any index is a projection.

use serde::{Deserialize, Serialize};

use crate::engine::{cid::BritCid, content_node::ContentNode};

/// One sealed filesystem entry: a path and the content address of its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaEntry {
    /// Path relative to the sealed subtree root.
    pub path: String,
    /// Content address of the entry's bytes — a real IPLD CID link (tag-42 in
    /// dag-cbor), never a string FK, so it cannot dangle across versions.
    pub cid: BritCid,
}

/// The canonical seed manifest for a subtree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EprMeta {
    /// Schema version of the EprMeta format.
    pub epr_meta_version: u32,
    /// Path of the governed subtree, relative to the repo root.
    pub subtree: String,
    /// Sealed entries, sorted by `path` for deterministic encoding.
    pub entries: Vec<MetaEntry>,
    /// Typed import edges (cites). Stable empty encoding (Decision A).
    #[serde(default)]
    pub imports: Vec<crate::engine::interface_ref::InterfaceRef>,
    /// Typed export edges (provided ids). Stable empty encoding.
    #[serde(default)]
    pub exports: Vec<crate::engine::interface_ref::InterfaceRef>,
}

impl ContentNode for EprMeta {
    fn content_type(&self) -> &'static str {
        "brit.epr-meta"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::content_node::ContentNode;

    fn sample() -> EprMeta {
        use crate::engine::cid::BritCid;
        EprMeta {
            epr_meta_version: 1,
            subtree: "docs".into(),
            entries: vec![
                MetaEntry {
                    path: "a.md".into(),
                    cid: BritCid::compute_raw(b"a"),
                },
                MetaEntry {
                    path: "b.md".into(),
                    cid: BritCid::compute_raw(b"b"),
                },
            ],
            imports: vec![],
            exports: vec![],
        }
    }

    #[test]
    fn content_type_is_stable() {
        assert_eq!(sample().content_type(), "brit.epr-meta");
    }

    #[test]
    fn cid_is_deterministic() {
        assert_eq!(sample().compute_cid().unwrap(), sample().compute_cid().unwrap());
    }

    #[test]
    fn cid_changes_with_content() {
        let mut other = sample();
        other.subtree = "src".into();
        assert_ne!(sample().compute_cid().unwrap(), other.compute_cid().unwrap());
    }

    #[test]
    fn epr_meta_carries_imports_exports_with_stable_default() {
        use crate::engine::interface_ref::{EdgeKind, EdgeRole, InterfaceRef};
        let m = EprMeta {
            epr_meta_version: 1,
            subtree: "docs".into(),
            entries: vec![],
            imports: vec![InterfaceRef {
                kind: EdgeKind::DocCite,
                role: EdgeRole::Import,
                ref_: "x".into(),
                cid: None,
                drift: Some("sha256:0000000000000000".into()),
                desc: None,
            }],
            exports: vec![],
        };
        assert_eq!(m.content_type(), "brit.epr-meta");
        let bytes = m.canonical_bytes().unwrap();
        let back: EprMeta = serde_ipld_dagcbor::from_slice(&bytes).unwrap();
        assert_eq!(m, back);
    }
}
