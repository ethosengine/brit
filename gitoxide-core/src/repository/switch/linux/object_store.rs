//! Shared local-history boundaries for durable Git operations. These checks perform no writes.
use super::*;

/// Local retention refs cannot protect alternate or virtualized object histories.
pub(crate) fn validate_local_history(repo: &gix::Repository) -> Result<()> {
    ensure!(repo.namespace().is_none(), "reference namespaces require Git");
    for variable in [
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_REPLACE_REF_BASE",
    ] {
        ensure!(std::env::var_os(variable).is_none(), "{variable} requires Git");
    }
    ensure!(!repo.is_shallow()?, "shallow repositories require Git");
    ensure!(
        repo.config_snapshot().string("extensions.partialClone").is_none(),
        "partial repositories require Git"
    );
    for name in ["objects/info/alternates", "info/grafts"] {
        ensure!(
            !present(&repo.common_dir().join(name))?,
            "alternate objects or grafts require Git"
        );
    }
    ensure!(
        repo.references()?.prefixed("refs/replace/")?.next().is_none(),
        "replacement objects require Git"
    );
    let pack_dir = repo.common_dir().join("objects/pack");
    if pack_dir.is_dir() {
        for entry in fs::read_dir(pack_dir)? {
            ensure!(
                entry?
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "promisor"),
                "partial repositories require Git"
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_object_boundaries(repo: &gix::Repository) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let root = repo.common_dir().join("objects");
    ensure!(
        fs::canonicalize(absolute(repo, repo.objects.store_ref().path()))? == fs::canonicalize(&root)?,
        "effective object store differs from the repository object directory"
    );
    let device = fs::metadata(repo.common_dir())?.dev();
    let mut pending = vec![root];
    while let Some(directory) = pending.pop() {
        let metadata = fs::symlink_metadata(&directory)?;
        ensure!(
            metadata.is_dir() && metadata.dev() == device,
            "unsafe object directory {}",
            directory.display()
        );
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            ensure!(
                !metadata.file_type().is_symlink() && metadata.dev() == device,
                "unsafe object storage boundary"
            );
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                ensure!(metadata.is_file(), "unsupported object storage member");
            }
        }
    }
    Ok(())
}
