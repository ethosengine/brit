//! Admission and completion of an already resolved, ordinary Git merge.
use std::{fs, path::Path};

use anyhow::{Context, Result, bail, ensure};

// MERGE_HEAD is the recovery marker: remove it only after every other captured file.
const FILES: [&str; 4] = ["MERGE_MSG", "MERGE_MODE", "AUTO_MERGE", "MERGE_HEAD"];
const UNSUPPORTED: [&str; 9] = [
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "rebase-apply",
    "rebase-merge",
    "sequencer",
    "BISECT_LOG",
    "MERGE_AUTOSTASH",
    "SQUASH_MSG",
    "REBASE_HEAD",
];

pub(in crate::repository) struct ResolvedMerge {
    pub commit_id: gix::ObjectId,
    files: Vec<Option<Vec<u8>>>,
}

fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file(),
                "merge metadata is not a regular file: {}",
                path.display()
            );
            Ok(Some(
                fs::read(path).with_context(|| format!("read {}", path.display()))?,
            ))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("inspect {}", path.display())),
    }
}

fn present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("inspect {}", path.display())),
    }
}

fn no_pending_work(repo: &gix::Repository) -> Result<()> {
    for name in UNSUPPORTED {
        ensure!(
            !present(&repo.git_dir().join(name))?,
            "merge commit cannot complete pending {name}"
        );
    }
    ensure!(
        read(&repo.git_dir().join("MERGE_RR"))?.is_none_or(|bytes| bytes.is_empty()),
        "merge commit cannot complete pending rerere work"
    );
    Ok(())
}

impl ResolvedMerge {
    pub fn capture(repo: &gix::Repository) -> Result<Option<Self>> {
        let files: Vec<_> = FILES
            .iter()
            .map(|name| read(&repo.git_dir().join(name)))
            .collect::<Result<_>>()?;
        if files.iter().all(Option::is_none) {
            return Ok(None);
        }
        no_pending_work(repo)?;
        let bytes = files[3]
            .as_deref()
            .context("merge metadata exists without MERGE_HEAD; inspect interrupted state")?;
        let hex = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        ensure!(
            hex.len() == repo.object_hash().len_in_hex(),
            "MERGE_HEAD must contain exactly one full commit ID; octopus merges are unsupported"
        );
        let commit_id = gix::ObjectId::from_hex(hex).context("invalid MERGE_HEAD commit ID")?;
        {
            let source = gix::object::bounded::Source::deferred(repo);
            let mut buffer = gix::objs::find::Buffer::default();
            let data = source.load(&commit_id, &mut buffer)?;
            ensure!(data.kind == gix::objs::Kind::Commit, "MERGE_HEAD must name a commit");
        }
        let original_commit_id = repo.head_id().context("merge commit requires initialized HEAD")?;
        ensure!(
            repo.merge_base(original_commit_id, commit_id)?
                .is_none_or(|base| base != commit_id),
            "MERGE_HEAD is already incorporated into HEAD; inspect interrupted merge cleanup, do not commit again"
        );
        let index = repo.index()?;
        ensure!(
            index
                .entries()
                .iter()
                .all(|entry| entry.stage() == gix::index::entry::Stage::Unconflicted),
            "cannot commit an unmerged index"
        );
        let merge = Self { commit_id, files };
        merge.validate(repo)?;
        Ok(Some(merge))
    }

    pub fn validate(&self, repo: &gix::Repository) -> Result<()> {
        no_pending_work(repo)?;
        for (name, expected) in FILES.iter().zip(&self.files) {
            ensure!(
                read(&repo.git_dir().join(name))? == *expected,
                "merge metadata {name} changed; inspect before retrying"
            );
        }
        Ok(())
    }

    /// Hold ordinary metadata locks through publication and cleanup. They are
    /// disposable guards, not a recovery journal; foreign locks are never removed.
    pub fn lock(&self, repo: &gix::Repository) -> Result<Vec<gix::lock::Marker>> {
        let locks = FILES
            .iter()
            .map(|name| repo.git_dir().join(name))
            .chain(std::iter::once(repo.index_path()))
            .map(|path| {
                gix::lock::Marker::acquire_to_hold_resource(&path, gix::lock::acquire::Fail::Immediately, None, 0)
                    .with_context(|| format!("lock merge input {}", path.display()))
            })
            .collect::<Result<_>>()?;
        self.validate(repo)?;
        Ok(locks)
    }

    pub fn remaining(&self, repo: &gix::Repository) -> String {
        FILES
            .iter()
            .filter(|name| present(&repo.git_dir().join(name)).unwrap_or(true))
            .copied()
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn cleanup(&self, repo: &gix::Repository) -> Result<()> {
        self.validate(repo)?;
        for (name, expected) in FILES.iter().zip(&self.files) {
            if gix::interrupt::is_triggered() {
                bail!("merge cleanup interrupted before removing {name}");
            }
            // Recheck each file just before removing it, including absence.
            let path = repo.git_dir().join(name);
            ensure!(
                read(&path)? == *expected,
                "merge metadata {name} changed during cleanup"
            );
            if expected.is_some() {
                fs::remove_file(&path).with_context(|| format!("remove merge metadata {name}"))?;
            }
            #[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
            if *name == "MERGE_MSG" {
                super::super::switch::checkpoint("merge-cleanup")?;
            }
        }
        Ok(())
    }
}
