//! Mutating accessors for `branch.<name>.{remote,merge,description}` config keys.
//! Read counterparts live in `branch.rs`.

use std::io::Write as _;

use crate::{
    bstr::{BStr, BString, ByteSlice},
    config::tree::Branch,
};

///
pub mod error {
    use crate::bstr::BString;

    /// The error returned by [`Repository::set_branch_upstream()`](crate::Repository::set_branch_upstream()).
    #[derive(Debug, thiserror::Error)]
    #[allow(missing_docs)]
    pub enum SetUpstream {
        #[error("failed to write the config value")]
        Set(#[from] crate::config::set_value::Error),
        #[error("failed to commit the config snapshot")]
        Commit(#[from] crate::config::Error),
        #[error("failed to persist config changes to {path}")]
        Persist {
            path: std::path::PathBuf,
            #[source]
            source: std::io::Error,
        },
        #[error("the upstream ref name '{name}' is not a valid full ref name under refs/remotes/ or refs/heads/")]
        InvalidUpstream { name: BString },
    }

    /// The error returned by [`Repository::unset_branch_upstream()`](crate::Repository::unset_branch_upstream()).
    #[derive(Debug, thiserror::Error)]
    #[allow(missing_docs)]
    pub enum UnsetUpstream {
        #[error("failed to commit the config snapshot")]
        Commit(#[from] crate::config::Error),
        #[error("failed to persist config changes to {path}")]
        Persist {
            path: std::path::PathBuf,
            #[source]
            source: std::io::Error,
        },
        #[error("the branch '{0}' has no upstream tracking information (branch.<name>.remote / branch.<name>.merge)")]
        NoUpstream(BString),
    }

    /// The error returned by [`Repository::set_branch_description()`](crate::Repository::set_branch_description()).
    #[derive(Debug, thiserror::Error)]
    #[allow(missing_docs)]
    pub enum SetDescription {
        #[error("failed to write the config value")]
        Set(#[from] crate::config::set_value::Error),
        #[error("failed to commit the config snapshot")]
        Commit(#[from] crate::config::Error),
        #[error("failed to persist config changes to {path}")]
        Persist {
            path: std::path::PathBuf,
            #[source]
            source: std::io::Error,
        },
    }
}

/// Overwrite the on-disk `$GIT_DIR/config` (the local source layer) with
/// all Local-source sections from `config`.
///
/// This must be called BEFORE `SnapshotMut::commit()` because `commit()`
/// consumes the snapshot.  The in-memory state after `commit()` will then
/// be consistent with what was just written to disk.
fn flush_local_config(config: &gix_config::File<'static>, git_dir: &std::path::Path) -> std::io::Result<()> {
    // Prefer the path embedded in an existing Local section's metadata
    // (that is exactly the path git loaded the local config from).
    // Fall back to `$GIT_DIR/config` — this is always correct for a
    // standard (non-worktree, non-commondir) repository, and is what git
    // itself uses.
    let local_path: std::path::PathBuf = config
        .sections()
        .find(|s| s.meta().source == gix_config::Source::Local)
        .and_then(|s| s.meta().path.clone())
        .unwrap_or_else(|| git_dir.join("config"));

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&local_path)?;
    config.write_to_filter(&mut file, |s| s.meta().source == gix_config::Source::Local)?;
    file.flush()
}

impl crate::Repository {
    /// Write `branch.<short_name>.remote` and `branch.<short_name>.merge` so that
    /// `<short_name>` tracks `<upstream_full_ref>`.
    ///
    /// The translation from upstream ref to (remote, merge-ref) pair follows
    /// `vendor/git/branch.c:install_branch_config_multiple_remotes`:
    ///
    /// - If `upstream_full_ref` is `refs/remotes/<remote>/<branch>`, the remote name is
    ///   `<remote>` and `merge` is `refs/heads/<branch>`.
    /// - Otherwise the remote name is `.` (local tracking) and `merge` is the full ref
    ///   name as-is.
    pub fn set_branch_upstream(
        &mut self,
        short_name: &BStr,
        upstream_full_ref: &gix_ref::FullNameRef,
    ) -> Result<(), error::SetUpstream> {
        let upstream_bytes = upstream_full_ref.as_bstr();

        let (remote_name, merge_ref): (BString, BString) =
            if let Some(rest) = upstream_bytes.strip_prefix(b"refs/remotes/") {
                // refs/remotes/<remote>/<branch>
                let slash =
                    rest.iter()
                        .position(|&b| b == b'/')
                        .ok_or_else(|| error::SetUpstream::InvalidUpstream {
                            name: upstream_bytes.to_owned(),
                        })?;
                let remote = &rest[..slash];
                let local_branch_part = &rest[slash + 1..];
                if local_branch_part.is_empty() {
                    return Err(error::SetUpstream::InvalidUpstream {
                        name: upstream_bytes.to_owned(),
                    });
                }
                let mut merge: BString = BString::from(b"refs/heads/".as_ref());
                merge.extend_from_slice(local_branch_part);
                (remote.into(), merge)
            } else {
                // Local branch or any other full ref — remote is `.` (sentinel for local)
                (BString::from(b".".as_ref()), upstream_bytes.to_owned())
            };

        let git_dir = self.git_dir().to_owned();
        let mut snap = self.config_snapshot_mut();
        snap.set_subsection_value(&Branch::REMOTE, short_name, remote_name.as_bstr())?;
        snap.set_subsection_value(&Branch::MERGE, short_name, merge_ref.as_bstr())?;
        flush_local_config(&snap, &git_dir).map_err(|source| error::SetUpstream::Persist {
            path: git_dir.join("config"),
            source,
        })?;
        snap.commit()?;
        Ok(())
    }

    /// Clear `branch.<short_name>.remote` and `branch.<short_name>.merge`.
    ///
    /// Returns [`error::UnsetUpstream::NoUpstream`] if neither key was present, matching
    /// the behaviour of `git branch --unset-upstream` when there is no upstream configured.
    ///
    /// Mirrors `vendor/git/builtin/branch.c` lines 985-994.
    pub fn unset_branch_upstream(&mut self, short_name: &BStr) -> Result<(), error::UnsetUpstream> {
        // Probe directly via the resolved config before taking the mutable snapshot.
        let had_remote = self
            .config
            .resolved
            .string_by("branch", Some(short_name), Branch::REMOTE.name)
            .is_some();
        let had_merge = self
            .config
            .resolved
            .string_by("branch", Some(short_name), Branch::MERGE.name)
            .is_some();

        if !had_remote && !had_merge {
            return Err(error::UnsetUpstream::NoUpstream(short_name.to_owned()));
        }

        let git_dir = self.git_dir().to_owned();
        let mut snap = self.config_snapshot_mut();
        if had_remote {
            snap.clear_subsection_value(&Branch::REMOTE, short_name);
        }
        if had_merge {
            snap.clear_subsection_value(&Branch::MERGE, short_name);
        }
        flush_local_config(&snap, &git_dir).map_err(|source| error::UnsetUpstream::Persist {
            path: git_dir.join("config"),
            source,
        })?;
        snap.commit()?;
        Ok(())
    }

    /// Set or clear `branch.<short_name>.description`.
    ///
    /// An empty `value` clears the key (best-effort: no error if the key was already absent).
    /// A non-empty `value` sets or overwrites the key.
    pub fn set_branch_description(&mut self, short_name: &BStr, value: &BStr) -> Result<(), error::SetDescription> {
        let git_dir = self.git_dir().to_owned();
        let mut snap = self.config_snapshot_mut();
        if value.is_empty() {
            // Best-effort clear: ignore the result — absent key is fine.
            snap.clear_subsection_value(&Branch::DESCRIPTION, short_name);
        } else {
            snap.set_subsection_value(&Branch::DESCRIPTION, short_name, value)?;
        }
        flush_local_config(&snap, &git_dir).map_err(|source| error::SetDescription::Persist {
            path: git_dir.join("config"),
            source,
        })?;
        snap.commit()?;
        Ok(())
    }
}
