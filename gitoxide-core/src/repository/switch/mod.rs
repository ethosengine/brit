//! Safe local-branch switching and explicit private recovery.
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

/// How long a writer waits for a held guard before refusing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MutationWait {
    /// Refuse at once, naming the holder.
    #[default]
    Refuse,
    /// Wait up to this long, then refuse naming the holder.
    For(Duration),
    /// Wait until the guard is free or the process is interrupted.
    Forever,
}

impl MutationWait {
    const FOREVER: u64 = u64::MAX;

    fn encode(self) -> u64 {
        match self {
            Self::Refuse => 0,
            Self::For(duration) => u64::try_from(duration.as_millis())
                .unwrap_or(Self::FOREVER - 1)
                .clamp(1, Self::FOREVER - 1),
            Self::Forever => Self::FOREVER,
        }
    }

    fn decode(millis: u64) -> Self {
        match millis {
            0 => Self::Refuse,
            Self::FOREVER => Self::Forever,
            millis => Self::For(Duration::from_millis(millis)),
        }
    }

    /// The longer of two policies.
    pub fn at_least(self, other: Self) -> Self {
        match (self, other) {
            (Self::Forever, _) | (_, Self::Forever) => Self::Forever,
            (Self::For(a), Self::For(b)) => Self::For(a.max(b)),
            (Self::For(a), Self::Refuse) | (Self::Refuse, Self::For(a)) => Self::For(a),
            (Self::Refuse, Self::Refuse) => Self::Refuse,
        }
    }

    /// Parse `SECONDS` (`0` refuses at once) or `forever`.
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("forever") {
            return Ok(Self::Forever);
        }
        match value.parse::<u64>() {
            Ok(0) => Ok(Self::Refuse),
            Ok(seconds) => Ok(Self::For(Duration::from_secs(seconds))),
            Err(_) => Err(format!(
                "expected a whole number of seconds or `forever`, got {value:?}"
            )),
        }
    }
}

static WAIT: AtomicU64 = AtomicU64::new(0);

/// Set the process-wide policy for writers that find a guard held.
pub fn set_mutation_wait(wait: MutationWait) {
    WAIT.store(wait.encode(), Ordering::Relaxed);
}

/// The process-wide policy for writers that find a guard held.
pub fn mutation_wait() -> MutationWait {
    MutationWait::decode(WAIT.load(Ordering::Relaxed))
}

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
    Change, Lock, State, acquire_recorded_locks, checkpoint, complete_directory_ledger, creation_mode, digest, payload,
    prepare_directories, private_copy, private_publish, private_write, publish_file_from, publish_metadata_at,
    shared_guard, snapshot, sync_dir, sync_file, sync_path, validate_local_history, validate_object_boundaries,
    verify_files, verify_locks,
};
#[cfg(target_os = "linux")]
pub use linux::{
    ensure_no_pending, execute, guard::WorktreeGuard, mutation_guard, mutation_guard_waiting, recover, status,
    worktree_mutation_guard, worktree_mutation_guard_waiting,
};
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
    /// Other platforms have no cooperative guard to wait for.
    pub fn mutation_guard_waiting(repo: &gix::Repository, _: super::MutationWait) -> Result<()> {
        ensure_no_pending(repo)
    }
    /// Other platforms have no cooperative guard; refuse only on pending recovery.
    pub fn worktree_mutation_guard(repo: &gix::Repository) -> Result<()> {
        ensure_no_pending(repo)
    }
    /// Other platforms have no cooperative guard to wait for.
    pub fn worktree_mutation_guard_waiting(repo: &gix::Repository, _: super::MutationWait) -> Result<()> {
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
pub use unavailable::{
    ensure_no_pending, execute, mutation_guard, mutation_guard_waiting, recover, status, worktree_mutation_guard,
    worktree_mutation_guard_waiting,
};

#[cfg(test)]
mod tests {
    use super::MutationWait;
    use std::time::Duration;

    #[test]
    fn wait_policy_round_trips_and_parses() {
        for wait in [
            MutationWait::Refuse,
            MutationWait::For(Duration::from_secs(30)),
            MutationWait::Forever,
        ] {
            assert_eq!(MutationWait::decode(wait.encode()), wait);
        }
        assert_eq!(MutationWait::parse("0"), Ok(MutationWait::Refuse));
        assert_eq!(MutationWait::parse("5"), Ok(MutationWait::For(Duration::from_secs(5))));
        assert_eq!(MutationWait::parse("forever"), Ok(MutationWait::Forever));
        assert!(MutationWait::parse("soon").is_err());
        assert_eq!(
            MutationWait::Refuse.at_least(MutationWait::For(Duration::from_secs(2))),
            MutationWait::For(Duration::from_secs(2))
        );
        assert_eq!(
            MutationWait::For(Duration::from_secs(9)).at_least(MutationWait::Forever),
            MutationWait::Forever
        );
    }
}
