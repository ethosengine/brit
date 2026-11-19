//! The public adapter's shared writer boundary, including commands with explicit destinations.
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use gitoxide_core::repository::switch::Held;
use gix::{Result, error::ResultExt};

/// Holds writer guards until the invoking command finishes.
///
/// A repository guard excludes every other Brit writer of the shared repository.
/// A worktree guard excludes shared writers and other writers of the same worktree only.
#[derive(Default)]
pub struct MutationGuards {
    repositories: BTreeSet<PathBuf>,
    worktrees: BTreeSet<PathBuf>,
    guards: Vec<Held>,
}

impl MutationGuards {
    /// Guard a write confined to `repo`'s worktree (index, HEAD, checkout, new objects).
    /// A repository already guarded exclusively by this process is covered by that guard.
    pub(crate) fn worktree(&mut self, repo: &gix::Repository) -> Result<()> {
        let common = repo.common_dir().canonicalize().or_error()?;
        let git_dir = repo.git_dir().canonicalize().or_error()?;
        if self.repositories.contains(&common) || self.worktrees.contains(&git_dir) {
            return Ok(());
        }
        if self
            .worktrees
            .iter()
            .any(|held| held.starts_with(&common) || common.starts_with(held))
        {
            return Err(gix::error::validation("one command cannot guard two worktrees of a repository").into());
        }
        self.guards.push(
            gitoxide_core::repository::switch::worktree_mutation_guard(repo).map_err(super::main::anyhow_to_gix)?,
        );
        self.worktrees.insert(git_dir);
        Ok(())
    }

    pub(crate) fn repository(&mut self, repo: &gix::Repository) -> Result<()> {
        self.exclusive(repo, false)
    }

    /// Guard a shared write that also rewrites `repo`'s own worktree.
    pub(crate) fn repository_and_worktree(&mut self, repo: &gix::Repository) -> Result<()> {
        self.exclusive(repo, true)
    }

    fn exclusive(&mut self, repo: &gix::Repository, rewrites_worktree: bool) -> Result<()> {
        let common = repo.common_dir().canonicalize().or_error()?;
        if !self.worktrees.is_empty() && !self.repositories.contains(&common) {
            // A held shared repository guard cannot be upgraded without a deadlock-prone window.
            return Err(
                gix::error::validation("a worktree-local command cannot also take the repository guard").into(),
            );
        }
        let git_dir = repo.git_dir().canonicalize().or_error()?;
        if self.repositories.contains(&common) {
            if rewrites_worktree && repo.workdir().is_some() && self.worktrees.insert(git_dir) {
                // Never wait for a worktree while holding the repository guard.
                self.guards.push(
                    gitoxide_core::repository::switch::other_worktree_guard(repo)
                        .map_err(super::main::anyhow_to_gix)?,
                );
            }
            return Ok(());
        }
        self.guards.push(
            if rewrites_worktree {
                gitoxide_core::repository::switch::worktree_writer_guard(repo)
            } else {
                gitoxide_core::repository::switch::mutation_guard(repo)
            }
            .map_err(super::main::anyhow_to_gix)?,
        );
        self.repositories.insert(common);
        if rewrites_worktree {
            self.worktrees.insert(git_dir);
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::MutationGuards;

    #[test]
    fn a_worktree_local_command_cannot_upgrade_to_the_repository_guard() {
        let dir = std::env::temp_dir().join(format!("brit-guard-upgrade-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary directory");
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&dir)
            .status()
            .expect("git runs");
        assert!(status.success());
        let repo = gix::open(&dir).expect("fresh repository opens");
        let mut guards = MutationGuards::default();
        guards.worktree(&repo).expect("worktree guard");
        guards.worktree(&repo).expect("the same worktree is already covered");
        let refused = guards.repository(&repo).expect_err("no upgrade to exclusive");
        assert!(
            refused.to_string().contains("cannot also take the repository guard"),
            "{refused}"
        );
        let refused = guards
            .repository_and_worktree(&repo)
            .expect_err("no upgrade to exclusive with the worktree");
        assert!(
            refused.to_string().contains("cannot also take the repository guard"),
            "{refused}"
        );
        drop(guards);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
