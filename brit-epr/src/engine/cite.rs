//! `id`/`cites` frontmatter extraction + the move-survivable slug index.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::engine::frontmatter::split_frontmatter;
use crate::engine::interface_ref::{parse_cite_line, InterfaceRef};

/// Read the `id:` scalar from a frontmatter block.
pub fn extract_id(frontmatter: &str) -> Option<String> {
    frontmatter
        .lines()
        .find_map(|l| l.strip_prefix("id:").map(|v| v.trim().to_string()))
        .filter(|s| !s.is_empty())
}

/// Read the `cites:` list (lines `  - …`) as import edges.
pub fn extract_cites(frontmatter: &str) -> Vec<InterfaceRef> {
    let mut out = Vec::new();
    let mut in_cites = false;
    for line in frontmatter.lines() {
        if line.trim_end() == "cites:" {
            in_cites = true;
            continue;
        }
        if in_cites {
            let t = line.trim_start();
            if let Some(item) = t.strip_prefix("- ") {
                out.push(parse_cite_line(item));
            } else if !line.starts_with(char::is_whitespace) && !line.trim().is_empty() {
                break; // next top-level key ends the block
            }
        }
    }
    out
}

/// Slug → path map over a doc corpus. The slug (a doc's `id:`) is identity;
/// the path is a disposable cache, so a move self-heals on rebuild.
pub struct SlugIndex(BTreeMap<String, PathBuf>);

impl SlugIndex {
    /// Walk every `*.md` under `roots`, mapping `id:` → path.
    pub fn build(roots: &[PathBuf]) -> std::io::Result<Self> {
        let mut map = BTreeMap::new();
        for root in roots {
            walk(root, &mut |path, content| {
                if let (Some(fm), _) = split_frontmatter(content) {
                    if let Some(id) = extract_id(fm) {
                        map.entry(id).or_insert_with(|| path.to_path_buf());
                    }
                }
            })?;
        }
        Ok(Self(map))
    }

    /// Resolve a slug to its current path, if any.
    pub fn resolve(&self, slug: &str) -> Option<&Path> {
        self.0.get(slug).map(PathBuf::as_path)
    }
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path, &str)) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, f)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                f(&path, &content);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FM: &str =
        "id: my-doc\ncites:\n  - alpha | a desc | sha256:00aa11bb22cc33dd\n  - beta | b | sha256:44ee55ff66007788\n";

    #[test]
    fn extracts_id() {
        assert_eq!(extract_id(FM).as_deref(), Some("my-doc"));
    }

    #[test]
    fn extracts_two_cites() {
        let cites = extract_cites(FM);
        assert_eq!(cites.len(), 2);
        assert_eq!(cites[0].ref_, "alpha");
        assert_eq!(cites[1].drift.as_deref(), Some("sha256:44ee55ff66007788"));
    }

    #[test]
    fn slug_index_resolves_ids() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.md"), "---\nid: doc-a\n---\nbody\n").unwrap();
        let idx = SlugIndex::build(&[tmp.path().to_path_buf()]).unwrap();
        assert!(idx.resolve("doc-a").is_some());
        assert!(idx.resolve("missing").is_none());
    }
}
