//! The public adapter's shared writer boundary, including commands with explicit destinations.
#[cfg(not(target_os = "linux"))]
type File = ();
#[cfg(target_os = "linux")]
use std::fs::File;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use gix::{Result, error::ResultExt};

/// Holds shared-repository writer guards until the invoking command finishes.
#[derive(Default)]
pub struct MutationGuards {
    repositories: BTreeSet<PathBuf>,
    guards: Vec<File>,
}

impl MutationGuards {
    pub(crate) fn repository(&mut self, repo: &gix::Repository) -> Result<()> {
        let common = repo.common_dir().canonicalize().or_error()?;
        if !self.repositories.contains(&common) {
            self.guards
                .push(gitoxide_core::repository::switch::mutation_guard(repo).map_err(super::main::anyhow_to_gix)?);
            self.repositories.insert(common);
        }
        Ok(())
    }

    /// Guard the repository containing an existing or proposed output path.
    /// An output outside a repository requires no Git writer guard.
    pub fn path(&mut self, path: &Path) -> Result<()> {
        let absolute = std::path::absolute(path).or_error()?;
        let mut parent = absolute.as_path();
        while !parent.is_dir() {
            if std::fs::symlink_metadata(parent).is_ok_and(|metadata| metadata.file_type().is_symlink())
                && !parent.exists()
            {
                return Err(gix::error::validation("cannot guard an output through a dangling symlink").into());
            }
            let Some(next) = parent.parent() else {
                return Ok(());
            };
            parent = next;
        }
        // Output APIs may follow a symlink; guard the actual destination's repository.
        let resolved = parent.canonicalize().or_error()?;
        let resolved = if absolute.exists() {
            absolute.canonicalize().or_error()?
        } else {
            resolved
        };
        let resolved = if resolved.is_dir() {
            resolved.as_path()
        } else {
            resolved.parent().unwrap_or(parent)
        };
        match gix::discover(resolved) {
            Ok(repo) => self.repository(&repo),
            Err(err)
                if matches!(
                    err.downcast_any_ref::<gix::discover::upwards::Error>(),
                    Some(
                        gix::discover::upwards::Error::NoGitRepository { .. }
                            | gix::discover::upwards::Error::NoGitRepositoryWithinCeiling { .. }
                            | gix::discover::upwards::Error::NoGitRepositoryWithinFs { .. }
                    )
                ) =>
            {
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}
