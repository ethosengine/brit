//! `InterfaceRef` — one typed import/export edge of the composition envelope.
//! Generic: `kind: doc-cite` is the only populated kind in this slice.

use serde::{Deserialize, Serialize};

use crate::engine::cid::BritCid;

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
    // `sha256:hex16` short-form, OR a full CIDv1 base32 token in the fingerprint slot: both
    // `bafy…` (dag-cbor 0x71) and `bafk…` (raw 0x55). A raw-codec *body* CID starts `bafk`, so a
    // `bafy`-only guard silently dropped it into `desc` — leaving `drift: None` and defeating the
    // `remote` verdict for raw-codec pins. `baf` covers the multibase-b CID family and matches both
    // the parent oracle (`cite_graph._is_fingerprint`) and this crate's own `verdict.rs`.
    s.starts_with("sha256:") || s.starts_with("baf")
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
    fn parses_raw_codec_full_cid_into_drift() {
        // A raw-codec body CID (`bafk…`, codec 0x55) in the fingerprint slot must be captured as
        // `drift`, not misread as `desc`. Regression pin for the `bafy`→`baf` parity fix: with the
        // old guard this CID fell through to `desc` and `drift` stayed `None`, so an absent target
        // could never reach `Verdict::Remote`.
        let full_cid = "bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e";
        let r = parse_cite_line(&format!("some-slug | a description | {full_cid}"));
        assert_eq!(r.kind, EdgeKind::DocCite);
        assert_eq!(r.drift.as_deref(), Some(full_cid));
        assert_eq!(r.desc.as_deref(), Some("a description"));
    }

    #[test]
    fn parses_dag_cbor_full_cid_into_drift() {
        // The `bafy…` (dag-cbor) rendering keeps working — the broadening is additive.
        let r = parse_cite_line("slug | desc | bafyreic36hmigi34p4nf6s2l3sfpnlcuop7hlc6zzd7uee2q6ar2ekzioy");
        assert_eq!(
            r.drift.as_deref(),
            Some("bafyreic36hmigi34p4nf6s2l3sfpnlcuop7hlc6zzd7uee2q6ar2ekzioy")
        );
    }

    #[test]
    fn no_pipe_segment_is_legacy() {
        let r = parse_cite_line("genesis/docs/foo.md");
        assert_eq!(r.kind, EdgeKind::Legacy);
        assert_eq!(r.ref_, "genesis/docs/foo.md");
        assert!(r.drift.is_none());
    }
}
