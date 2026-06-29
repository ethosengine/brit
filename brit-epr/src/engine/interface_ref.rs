//! `InterfaceRef` — one typed import/export edge of the composition envelope.
//! Generic: `kind: doc-cite` is the only populated kind in this slice.

use crate::engine::cid::BritCid;
use serde::{Deserialize, Serialize};

/// The typed interface kind an edge carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    /// A citation between docs (this slice).
    DocCite,
    /// (deferred) addressable content.
    Content,
    /// (deferred) a schema version.
    SchemaVersion,
    /// (deferred) a capability.
    Capability,
    /// (deferred) a constitutional governance contract.
    Contract,
    /// A legacy path-string cite to an id-bearing target.
    Legacy,
    /// A cross-repo target outside this snapshot.
    External,
}

/// Whether an edge is an import (a need) or an export (a provision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeRole {
    /// This node needs the target.
    Import,
    /// This node provides the target.
    Export,
}

/// One typed import/export edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceRef {
    /// The typed interface kind.
    pub kind: EdgeKind,
    /// Import or export.
    pub role: EdgeRole,
    /// Stable identity — the target's `id`/slug (move-survivable).
    #[serde(rename = "ref")]
    pub ref_: String,
    /// The addressable content version, when one exists.
    pub cid: Option<BritCid>,
    /// `sha256:hex16` non-address drift fingerprint (doc-cite).
    pub drift: Option<String>,
    /// Directional relationship hint (imports only).
    pub desc: Option<String>,
}

fn is_fingerprint(s: &str) -> bool {
    s.starts_with("sha256:") || s.starts_with("bafy")
}

/// Parse one frontmatter `cites:` line into a `DocCite` import (or `Legacy`).
pub fn parse_cite_line(line: &str) -> InterfaceRef {
    let line = line.split(" # ").next().unwrap_or(line).trim();
    let segments: Vec<&str> = line.split(" | ").map(str::trim).collect();
    if segments.len() == 1 {
        return InterfaceRef {
            kind: EdgeKind::Legacy,
            role: EdgeRole::Import,
            ref_: segments[0].to_string(),
            cid: None,
            drift: None,
            desc: None,
        };
    }
    let ref_ = segments[0].to_string();
    let (mut drift, mut desc) = (None, None);
    for seg in &segments[1..] {
        if is_fingerprint(seg) {
            drift = Some((*seg).to_string());
        } else if seg.starts_with("status:") || seg.starts_with("path:") {
            // health/locator hints — not load-bearing for identity.
        } else if desc.is_none() {
            desc = Some((*seg).to_string());
        }
    }
    InterfaceRef {
        kind: EdgeKind::DocCite,
        role: EdgeRole::Import,
        ref_,
        cid: None,
        drift,
        desc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_envelope_cite() {
        let r = parse_cite_line("constitution | the law | sha256:1eb96af782012fc6 | path: a/b.md");
        assert_eq!(r.kind, EdgeKind::DocCite);
        assert_eq!(r.role, EdgeRole::Import);
        assert_eq!(r.ref_, "constitution");
        assert_eq!(r.desc.as_deref(), Some("the law"));
        assert_eq!(r.drift.as_deref(), Some("sha256:1eb96af782012fc6"));
    }

    #[test]
    fn no_pipe_segment_is_legacy() {
        let r = parse_cite_line("genesis/docs/foo.md");
        assert_eq!(r.kind, EdgeKind::Legacy);
        assert_eq!(r.ref_, "genesis/docs/foo.md");
        assert!(r.drift.is_none());
    }
}
