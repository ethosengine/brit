//! Safe local-branch switching and explicit private recovery.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::shared_guard;
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
