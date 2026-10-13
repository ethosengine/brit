//! Safe local-branch switching and explicit private recovery.
fn ensure_no_replay_cleanup(git_dir: &std::path::Path) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        super::replay::ensure_cleanup_retired(git_dir)
    }
    #[cfg(not(target_os = "linux"))]
    {
        for entry in std::fs::read_dir(git_dir)? {
            let entry = entry?;
            anyhow::ensure!(
                !entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"brit-rebase-cleanup-"),
                "replay cleanup pending at {}; explicitly recover that receipt before another mutation",
                entry.path().display()
            );
        }
        Ok(())
    }
}
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::{
    Change, Lock, State, acquire_recorded_locks, checkpoint, complete_directory_ledger, creation_mode, digest,
    prepare_directories, private_write, publish_file, publish_metadata_at, shared_guard, snapshot, sync_dir, sync_file,
    sync_path, validate_local_history, validate_object_boundaries, verify_files, verify_locks,
};
#[cfg(target_os = "linux")]
pub use linux::{ensure_no_pending, execute, mutation_guard, recover, status};
#[cfg(all(target_os = "linux", feature = "blocking-client"))]
pub use linux::{pull_execute, pull_recover};

#[cfg(not(target_os = "linux"))]
mod unavailable {
    use anyhow::{Result, bail, ensure};
    fn journal_present(path: &std::path::Path) -> Result<bool> {
        match std::fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err.into()),
        }
    }
    /// Refuse mutation while a Linux-created switch journal is pending.
    pub fn ensure_no_pending(repo: &gix::Repository) -> Result<()> {
        for worktree in repo.worktrees_including_main()? {
            let worktree = worktree?;
            super::ensure_no_replay_cleanup(worktree.git_dir())?;
            ensure!(
                !journal_present(&worktree.git_dir().join("brit-rebase"))?,
                "replay recovery requires Linux"
            );
            ensure!(
                !journal_present(&worktree.git_dir().join("brit-authoring"))?,
                "conflict authoring recovery requires Linux"
            );
            ensure!(
                !journal_present(&worktree.git_dir().join("brit-switch"))?,
                "switch recovery pending; recover on a supported Linux filesystem"
            );
        }
        Ok(())
    }
    /// Keep ordinary non-switch operations available on other platforms.
    pub fn mutation_guard(repo: &gix::Repository) -> Result<()> {
        ensure_no_pending(repo)
    }
    /// Refuse before mutation where the durability contract cannot be fulfilled.
    pub fn execute(_: gix::Repository, _: &gix::bstr::BStr) -> Result<()> {
        bail!("durable switch currently requires Linux file and directory synchronization")
    }
    /// Recovery uses the same durability contract as initial publication.
    pub fn recover(_: gix::Repository, _: bool) -> Result<()> {
        bail!("durable switch recovery currently requires Linux")
    }
    /// Report availability without modifying a journal.
    pub fn status(repo: &gix::Repository, out: &mut dyn std::io::Write) -> Result<()> {
        if journal_present(&repo.git_dir().join("brit-switch"))? {
            writeln!(out, "Switch recovery pending; recover on Linux")?;
        } else {
            writeln!(out, "No switch or pull recovery pending")?;
        }
        Ok(())
    }
}
#[cfg(not(target_os = "linux"))]
pub use unavailable::{ensure_no_pending, execute, mutation_guard, recover, status};
