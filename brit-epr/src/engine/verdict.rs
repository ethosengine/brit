//! The cite verdict engine — `envelope_verdict`-equivalent, parity with the
//! parent oracle. "Current" is the live filesystem (no head pointer; that is
//! Layer-2). Precedence: dead > held > stale > ok.

use serde::{Deserialize, Serialize};

use crate::engine::{
    cite::SlugIndex,
    frontmatter::drift_fingerprint,
    interface_ref::{EdgeKind, InterfaceRef},
};

/// The health of one cite edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Resolves, in the live tree, body unchanged.
    Ok,
    /// Resolves but the target is sequestered under `held/`.
    Held,
    /// Resolves but the target body drifted since the pin.
    Stale,
    /// Absent from the live tree, but the edge carries positive remote evidence
    /// — a full-CID token (`baf…`) in the fingerprint slot, resolvable on the
    /// substrate. Conservative: only a full CID promotes an absent target off
    /// `Dead`; a short-form/absent fingerprint stays `Dead`.
    Remote,
    /// The slug resolves nowhere and there is no full-CID evidence.
    Dead,
}

/// Compute the verdict for one edge against the slug index + filesystem.
/// Mirrors the parent oracle's `envelope_verdict`: a legacy edge or a
/// fingerprint-less cite is `Ok`; an unreadable in-index target is `Ok`.
pub fn verdict(edge: &InterfaceRef, idx: &SlugIndex) -> Verdict {
    if edge.kind == EdgeKind::Legacy {
        return Verdict::Ok;
    }
    let Some(path) = idx.resolve(&edge.ref_) else {
        // Absent locally. Conservative mirror of the oracle: stays `Dead` UNLESS the fingerprint
        // slot (`edge.drift`) carries a full-CID token (`baf…`) — positive, substrate-resolvable
        // remote evidence. A short-form `sha256:hex16` (or absent) fingerprint is NOT full-CID
        // evidence → `Dead`. Existing dead cites are never reclassified.
        return if edge.drift.as_deref().is_some_and(|d| d.starts_with("baf")) {
            Verdict::Remote
        } else {
            Verdict::Dead
        };
    };
    if path.components().any(|c| c.as_os_str() == "held") {
        return Verdict::Held;
    }
    // No fingerprint → no drift claim → ok (parent parity).
    let Some(drift) = edge.drift.as_deref() else {
        return Verdict::Ok;
    };
    // In-index but unreadable → ok, matching the oracle's OSError→pass.
    let Ok(current) = std::fs::read_to_string(path) else {
        return Verdict::Ok;
    };
    if drift != drift_fingerprint(&current).as_str() {
        return Verdict::Stale;
    }
    Verdict::Ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        cite::SlugIndex,
        frontmatter::drift_fingerprint,
        interface_ref::{EdgeKind, EdgeRole, InterfaceRef},
    };

    fn cite(reff: &str, drift: Option<&str>) -> InterfaceRef {
        InterfaceRef {
            kind: EdgeKind::DocCite,
            role: EdgeRole::Import,
            ref_: reff.into(),
            cid: None,
            drift: drift.map(String::from),
            desc: None,
        }
    }

    #[test]
    fn ok_held_stale_dead() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("t.md"), "---\nid: target\n---\nbody\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("held")).unwrap();
        std::fs::write(tmp.path().join("held/h.md"), "---\nid: held-doc\n---\nb\n").unwrap();
        let idx = SlugIndex::build(&[tmp.path().to_path_buf()]).unwrap();
        let target_fp = drift_fingerprint("---\nid: target\n---\nbody\n");

        assert_eq!(verdict(&cite("target", Some(&target_fp)), &idx), Verdict::Ok);
        assert_eq!(
            verdict(&cite("target", Some("sha256:0000000000000000")), &idx),
            Verdict::Stale
        );
        assert_eq!(
            verdict(&cite("held-doc", Some("sha256:0000000000000000")), &idx),
            Verdict::Held
        );
        assert_eq!(
            verdict(&cite("nope", Some("sha256:0000000000000000")), &idx),
            Verdict::Dead
        );
        // parity with the oracle: a cite carrying no fingerprint is Ok, never Stale.
        assert_eq!(verdict(&cite("target", None), &idx), Verdict::Ok);
    }

    #[test]
    fn remote_only_on_absent_slug_with_full_cid() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("t.md"), "---\nid: target\n---\nbody\n").unwrap();
        let idx = SlugIndex::build(&[tmp.path().to_path_buf()]).unwrap();
        // Absent slug + full-CID (`baf…`) drift token → positive remote evidence → Remote.
        let full_cid = "bafkreidmi27tswvwvwm45sbvlytbbe4yo5acu3jpif42eynnk3h4sntyty";
        assert_eq!(verdict(&cite("no-such-slug", Some(full_cid)), &idx), Verdict::Remote);
        // Absent slug + short-form fingerprint → conservative → still Dead.
        assert_eq!(
            verdict(&cite("no-such-slug", Some("sha256:0000000000000000")), &idx),
            Verdict::Dead
        );
        // Absent slug + no fingerprint → still Dead.
        assert_eq!(verdict(&cite("no-such-slug", None), &idx), Verdict::Dead);
        // Precedence: a resolvable target with a full-CID token is never Remote.
        assert_eq!(verdict(&cite("target", Some(full_cid)), &idx), Verdict::Stale);
    }
}
