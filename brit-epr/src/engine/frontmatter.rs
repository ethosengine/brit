//! Frontmatter splitting + the canonical-body drift fingerprint.
//!
//! Generic engine surface: a doc's drift identity is a `sha256:hex16`
//! fingerprint (NOT a content address) over its frontmatter-excluded,
//! trimmed body — byte-matching the parent cite oracle's recipe so a
//! metadata/cites edit never trips staleness.

use multihash_codetable::{Code, MultihashDigest};

/// Split a document into `(frontmatter_yaml, body)`. Frontmatter is the block
/// between a leading `---` line and the next `---` line. Returns `None`
/// frontmatter when the content does not open with `---`.
pub fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let rest = match content.strip_prefix("---\n") {
        Some(r) => r,
        None => return (None, content),
    };
    if let Some(end) = rest.find("\n---\n") {
        let fm = &rest[..=end]; // include the trailing newline of the fm block
        let body = &rest[end + "\n---\n".len()..];
        (Some(fm), body)
    } else if let Some(stripped) = rest.strip_suffix("\n---") {
        (Some(stripped), "")
    } else {
        (None, content) // unterminated frontmatter: treat all as body
    }
}

/// The frontmatter-excluded, trimmed body — the bytes the drift fingerprint hashes.
pub fn canonical_body(content: &str) -> String {
    let (_, body) = split_frontmatter(content);
    body.trim().to_string()
}

/// `"sha256:" + first-16-hex of sha256(canonical_body)` — a non-address
/// fingerprint that byte-matches the parent oracle.
pub fn drift_fingerprint(content: &str) -> String {
    let body = canonical_body(content);
    let mh = Code::Sha2_256.digest(body.as_bytes());
    let hex = hex::encode(mh.digest());
    format!("sha256:{}", &hex[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_yaml_frontmatter() {
        let c = "---\nid: foo\n---\nbody text\n";
        let (fm, body) = split_frontmatter(c);
        assert_eq!(fm, Some("id: foo\n"));
        assert_eq!(body, "body text\n");
    }

    #[test]
    fn no_frontmatter_is_whole_body() {
        let c = "# Title\nno frontmatter here\n";
        let (fm, body) = split_frontmatter(c);
        assert_eq!(fm, None);
        assert_eq!(body, c);
    }

    #[test]
    fn canonical_body_excludes_frontmatter_and_trims() {
        let c = "---\nid: foo\n---\n\n  body  \n\n";
        assert_eq!(canonical_body(c), "body");
    }

    #[test]
    fn drift_is_sha256_prefixed_16_hex() {
        let f = drift_fingerprint("---\nid: x\n---\nhello\n");
        assert!(f.starts_with("sha256:"));
        assert_eq!(f.len(), "sha256:".len() + 16);
    }

    #[test]
    fn frontmatter_edit_does_not_change_drift() {
        let a = "---\nid: foo\n---\nstable body\n";
        let b = "---\nid: foo\ncites:\n  - x | d | sha256:0000000000000000\n---\nstable body\n";
        assert_eq!(drift_fingerprint(a), drift_fingerprint(b));
    }
}
