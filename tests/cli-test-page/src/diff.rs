//! Unified diff between baseline and candidate.

use similar::{ChangeTag, TextDiff};

pub fn has_diff(a: &str, b: &str) -> bool {
    a != b
}

pub fn render_unified_diff(a: &str, b: &str) -> String {
    let diff = TextDiff::from_lines(a, b);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let prefix = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        out.push_str(prefix);
        out.push_str(change.value());
    }
    out
}
