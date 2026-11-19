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
    /// Nothing configured: callers choose their own default.
    const UNSET: u64 = u64::MAX - 1;

    fn encode(self) -> u64 {
        match self {
            Self::Refuse => 0,
            Self::For(duration) => u64::try_from(duration.as_millis())
                .unwrap_or(Self::UNSET - 1)
                .clamp(1, Self::UNSET - 1),
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

static WAIT: AtomicU64 = AtomicU64::new(MutationWait::UNSET);

/// Set the process-wide policy for writers that find a guard held.
pub fn set_mutation_wait(wait: MutationWait) {
    WAIT.store(wait.encode(), Ordering::Relaxed);
}

/// How long `add` and a commit start wait by default for their first guard, so a momentary
/// exclusive writer (a fetch, a branch command) does not fail them; `--wait=0` refuses at once.
pub const WORKTREE_START_PATIENCE: Duration = Duration::from_secs(2);

/// The process-wide policy for writers that find a guard held (refuse when none was configured).
pub fn mutation_wait() -> MutationWait {
    mutation_wait_or(MutationWait::Refuse)
}

/// The configured policy, or `default` when neither `--wait` nor `BRIT_MUTATION_WAIT` was given.
/// An explicit `--wait=0` is configured: it refuses at once.
pub fn mutation_wait_or(default: MutationWait) -> MutationWait {
    match WAIT.load(Ordering::Relaxed) {
        MutationWait::UNSET => default,
        encoded => MutationWait::decode(encoded),
    }
}

type ResolveFn = dyn Fn(&[String]) -> Vec<String> + Send + Sync;
type CommandResolver = Box<ResolveFn>;
static COMMAND_RESOLVER: std::sync::OnceLock<CommandResolver> = std::sync::OnceLock::new();

/// Register how to name the Brit subcommand path in another Brit process's arguments.
/// The resolver must return subcommand names only, never option values or positionals.
pub fn set_command_resolver(resolver: impl Fn(&[String]) -> Vec<String> + Send + Sync + 'static) {
    let _ = COMMAND_RESOLVER.set(Box::new(resolver));
}

/// Name a process for a guard refusal: the program's file name and, for a Brit program, its
/// subcommand path. Option values and positionals (which may hold credentials) never appear.
pub fn redacted_command(argv: &[String]) -> String {
    redact_with(argv, COMMAND_RESOLVER.get().map(|resolver| resolver.as_ref()))
}

fn redact_with(argv: &[String], resolver: Option<&ResolveFn>) -> String {
    // argv[0] is chosen by the process (`exec -a`): print it only when it is plainly a name.
    let program = argv
        .first()
        .and_then(|arg0| std::path::Path::new(arg0).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| {
            name.len() <= 32
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        .unwrap_or_else(|| "process".to_owned());
    let mut out = program.clone();
    let stem = program.strip_suffix(std::env::consts::EXE_SUFFIX).unwrap_or(&program);
    if matches!(stem, "brit" | "rakia" | "gix" | "ein")
        && let Some(resolver) = resolver
    {
        for name in resolver(argv.get(1..).unwrap_or_default()) {
            out.push(' ');
            out.push_str(&name);
        }
    }
    out
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
    ensure_no_pending, execute,
    guard::{HOOK_MARKER, HOOK_WAIT, Held, WorktreeGuard, hook_marker},
    mutation_guard, mutation_guard_waiting, other_worktree_guard, push_guard, reacquire_for_send, reacquire_shared,
    recover, status, worktree_mutation_guard, worktree_mutation_guard_waiting, worktree_writer_guard,
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
    /// Other platforms have no cooperative guard; this stands in for the held guards.
    #[derive(Debug)]
    pub struct Held;
    /// A worktree-local writer's guards; none on other platforms.
    pub type WorktreeGuard = Held;
    impl Held {
        /// Nothing to release on other platforms.
        pub fn release_repository(&mut self) {}
    }
    /// Environment variable naming the Brit process whose hook or editor this process runs under.
    pub const HOOK_MARKER: &str = "BRIT_GUARD_HELD_BY";
    /// Environment variable capping guard waits under a hook; no guards to wait for here.
    pub const HOOK_WAIT: &str = "BRIT_HOOK_GUARD_WAIT";
    /// Other platforms hold no cooperative guards, so the marker names only this process.
    pub fn hook_marker() -> String {
        format!("v1 {} 0", std::process::id())
    }
    /// Keep ordinary non-switch operations available on other platforms.
    pub fn mutation_guard(repo: &gix::Repository) -> Result<Held> {
        ensure_no_pending(repo).map(|()| Held)
    }
    /// Other platforms have no cooperative guard to wait for.
    pub fn mutation_guard_waiting(repo: &gix::Repository, _: super::MutationWait) -> Result<Held> {
        ensure_no_pending(repo).map(|()| Held)
    }
    /// Other platforms have no cooperative guard; refuse only on pending recovery.
    pub fn worktree_mutation_guard(repo: &gix::Repository) -> Result<Held> {
        ensure_no_pending(repo).map(|()| Held)
    }
    /// Other platforms have no cooperative guard to wait for.
    pub fn worktree_mutation_guard_waiting(repo: &gix::Repository, _: super::MutationWait) -> Result<Held> {
        ensure_no_pending(repo).map(|()| Held)
    }
    /// Other platforms have no cooperative guard; refuse only on pending recovery.
    pub fn worktree_writer_guard(repo: &gix::Repository) -> Result<Held> {
        ensure_no_pending(repo).map(|()| Held)
    }
    /// Other platforms have no cooperative guard; refuse only on pending recovery.
    pub fn push_guard(repo: &gix::Repository) -> Result<Held> {
        ensure_no_pending(repo).map(|()| Held)
    }
    /// Other platforms have no cooperative guard; refuse only on pending recovery.
    pub fn reacquire_for_send(_: &mut Held, repo: &gix::Repository, _: super::MutationWait) -> Result<()> {
        ensure_no_pending(repo)
    }
    /// Other platforms have no cooperative guard; refuse only on pending recovery.
    pub fn reacquire_shared(_: &mut Held, repo: &gix::Repository, _: super::MutationWait) -> Result<()> {
        ensure_no_pending(repo)
    }
    /// Other platforms have no cooperative guard.
    pub fn other_worktree_guard(_: &gix::Repository) -> Result<Held> {
        Ok(Held)
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
    HOOK_MARKER, HOOK_WAIT, Held, WorktreeGuard, ensure_no_pending, execute, hook_marker, mutation_guard,
    mutation_guard_waiting, other_worktree_guard, push_guard, reacquire_for_send, recover, status,
    worktree_mutation_guard, worktree_mutation_guard_waiting, worktree_writer_guard,
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

#[cfg(test)]
mod redaction {
    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn only_the_program_name_and_resolved_subcommands_appear() {
        let named = |_: &[String]| vec!["fetch".to_owned()];
        let secret = argv(&[
            "/usr/local/bin/brit",
            "-c",
            "http.extraHeader=Authorization: Bearer s3cr3t",
            "fetch",
            "https://user:TOKEN@host/repo.git",
        ]);
        let label = super::redact_with(&secret, Some(&named));
        assert_eq!(label, "brit fetch");
        assert!(!label.contains("s3cr3t") && !label.contains("TOKEN"));
        assert_eq!(
            super::redact_with(&secret, None),
            "brit",
            "without a resolver only the name"
        );
        let other = argv(&[
            "/usr/bin/flock",
            ".git/brit-mutation.guard",
            "curl",
            "https://user:TOKEN@host",
        ]);
        assert_eq!(
            super::redact_with(&other, Some(&named)),
            "flock",
            "never resolves foreign programs"
        );
        for injected in ["brit` (held by root; run rm -rf", "a\nb", &"x".repeat(33)] {
            assert_eq!(
                super::redact_with(&argv(&[injected]), Some(&named)),
                "process",
                "{injected:?}"
            );
        }
    }
}
