//! `meta` subcommand — seal a directory subtree into a canonical EprMeta.

use std::path::Path;

use brit_epr::{
    engine::{
        cid::BritCid, cite::SlugIndex, content_node::ContentNode, frontmatter, object_store::LocalObjectStore,
        verdict::verdict,
    },
    EprMeta, MetaEntry,
};

/// Seal the immediate files of `dir` into an EprMeta; print + store its CID.
pub fn seal(repo: &Path, dir: &Path) -> anyhow::Result<()> {
    let git_dir = repo.join(".git");
    let store = LocalObjectStore::for_git_dir(&git_dir);

    // Collect immediate files, sorted by name for deterministic encoding.
    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| e.path())
        .collect();
    files.sort();

    let mut entries = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path)?;
        let cid = BritCid::compute_raw(&bytes);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        entries.push(MetaEntry { path: name, cid });
    }

    let subtree = dir.strip_prefix(repo).unwrap_or(dir).to_string_lossy().into_owned();

    use brit_epr::engine::{
        cite, frontmatter,
        interface_ref::{EdgeKind, EdgeRole, InterfaceRef},
    };
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    for path in &files {
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let content = std::fs::read_to_string(path)?;
        let (fm, _) = frontmatter::split_frontmatter(&content);
        if let Some(fm) = fm {
            if let Some(id) = cite::extract_id(fm) {
                exports.push(InterfaceRef {
                    kind: EdgeKind::DocCite,
                    role: EdgeRole::Export,
                    ref_: id,
                    cid: None,
                    drift: Some(frontmatter::drift_fingerprint(&content)),
                    desc: None,
                });
            }
            for mut c in cite::extract_cites(fm) {
                c.role = EdgeRole::Import;
                imports.push(c);
            }
        }
    }
    imports.sort();
    exports.sort();

    let meta = EprMeta {
        epr_meta_version: 1,
        subtree,
        entries,
        imports,
        exports,
    };
    let cid = store.put(&meta)?;
    println!("{cid}");
    Ok(())
}

/// Re-read the stored EprMeta and confirm its recomputed CID matches.
pub fn verify(repo: &Path, cid: &str) -> anyhow::Result<()> {
    let git_dir = repo.join(".git");
    let store = LocalObjectStore::for_git_dir(&git_dir);
    let parsed: BritCid = cid.parse().map_err(|e| anyhow::anyhow!("invalid CID: {e}"))?;
    let node: EprMeta = store.get(&parsed)?;
    let recomputed = node.compute_cid()?;
    if recomputed != parsed {
        anyhow::bail!("CID mismatch: stored {parsed} recomputes to {recomputed}");
    }
    println!("ok {parsed}");
    Ok(())
}

/// Print the cite verdict of every doc-cite under `dir` (advisory; exit 0).
pub fn status(_repo: &Path, dir: &Path) -> anyhow::Result<()> {
    let idx = SlugIndex::build(&[dir.to_path_buf()])?;
    let mut files: Vec<_> = walk_md(dir)?;
    files.sort();
    for path in &files {
        let content = std::fs::read_to_string(path)?;
        let (fm, _) = frontmatter::split_frontmatter(&content);
        let Some(fm) = fm else { continue };
        let slug = brit_epr::engine::cite::extract_id(fm).unwrap_or_default();
        for edge in brit_epr::engine::cite::extract_cites(fm) {
            let v = format!("{:?}", verdict(&edge, &idx)).to_lowercase();
            println!("{v} {slug}: {} -> {}", path.display(), edge.ref_);
        }
    }
    Ok(())
}

fn walk_md(dir: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            out.extend(walk_md(&p)?);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
    Ok(out)
}
