//! `meta` subcommand — seal a directory subtree into a canonical EprMeta.

use std::path::Path;

use brit_epr::{
    engine::{cid::BritCid, content_node::ContentNode, object_store::LocalObjectStore},
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

    let meta = EprMeta {
        epr_meta_version: 1,
        subtree,
        entries,
        imports: vec![],
        exports: vec![],
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
