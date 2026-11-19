//! Cooperative writer guards for one repository and its linked worktrees.
//!
//! Advisory `flock` locks order every Brit writer:
//!
//! - `<common-dir>/brit-mutation.guard` is the repository guard. A writer of shared state
//!   (references, packed-refs, configuration, worktree registrations, switch/pull/replay
//!   journals, object-store maintenance) holds it exclusively. A worktree-local writer holds
//!   it shared, so it excludes every shared writer but not other worktree-local writers.
//! - The worktree's private Git directory (`<git-dir>` itself, locked as a directory) is one
//!   worktree's guard. Any writer of that worktree's index, HEAD or files holds it
//!   exclusively, whether its repository-guard hold is shared or exclusive. Locking the
//!   existing directory adds no file.
//! - `<common-dir>/brit-mutation.turnstile`, created only once an exclusive writer has had to
//!   wait, keeps waiting exclusive writers from starving: an exclusive waiter holds it until it
//!   has the repository guard, and a new shared acquirer passes through it first.
//!
//! Order: a worktree guard is always taken before the repository guard, and nobody waits for
//! a worktree guard while holding the repository guard. Waiting for the repository guard while
//! holding a worktree guard is allowed (push re-acquires the repository guard after its
//! pre-push hook while still holding its worktree), so no cycle can form.
//!
//! A process never waits for a guard held by itself or by one of its ancestors: a hook that
//! runs a Brit writer would otherwise wait forever on the command it runs under. Every hook
//! and editor Brit spawns carries [`HOOK_MARKER`], which stands in when `/proc` cannot name a
//! holder, and which lets hooks bypass the turnstile their parent's waiters may hold.
//!
//! A refusal names the holders the kernel records in `/proc/locks`, which never goes stale and
//! needs no extra files in the repository. Holder command lines are reduced to the program
//! name and Brit subcommand path, so option values and positionals (credentials) never leak.
use super::super::MutationWait;
use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

/// Environment variable naming the Brit process whose hook or editor this process runs under.
pub const HOOK_MARKER: &str = "BRIT_GUARD_HELD_BY";

const REPOSITORY_GUARD: &str = "brit-mutation.guard";
const TURNSTILE: &str = "brit-mutation.turnstile";

/// Guards released together when dropped.
#[derive(Debug)]
pub struct Held {
    // Dropped in order: repository guard first, then the worktree guard.
    repository: Option<fs::File>,
    _worktree: Option<fs::File>,
}

/// A worktree-local writer's guards: the repository guard shared, its worktree's exclusive.
pub type WorktreeGuard = Held;

impl From<fs::File> for Held {
    /// Hold a worktree guard taken with [`other_worktree`].
    fn from(worktree: fs::File) -> Self {
        Self {
            repository: None,
            _worktree: Some(worktree),
        }
    }
}

impl Held {
    /// Release the repository guard, keeping the worktree guard.
    pub fn release_repository(&mut self) {
        self.repository = None;
    }

    /// Re-take the repository guard exclusively while the worktree guard stays held.
    pub(crate) fn reacquire_repository(&mut self, repo: &gix::Repository, wait: MutationWait) -> Result<()> {
        let deadline = Deadline::new(wait);
        let file = open(&repo.common_dir().join(REPOSITORY_GUARD))?;
        lock_repository_exclusive(repo, &file, &deadline)?;
        self.repository = Some(file);
        Ok(())
    }
}

fn open(path: &Path) -> Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .with_context(|| format!("could not open Brit mutation guard at {}", path.display()))
}

fn open_directory(path: &Path) -> Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::DIRECTORY.bits() as i32)
        .open(path)
        .with_context(|| format!("could not open the worktree guard at {}", path.display()))
}

/// The repository guard mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Exclusive,
    Shared,
}

/// Which lock a file is, so a holder's lock reads as the write it performs.
#[derive(Clone, Copy)]
enum Level {
    Repository,
    Worktree,
    Turnstile,
}

/// One deadline across every lock of an acquisition.
enum Deadline {
    Refuse,
    At(Instant, Instant),
    Forever(Instant),
}

impl Deadline {
    fn new(wait: MutationWait) -> Self {
        let now = Instant::now();
        match wait {
            MutationWait::Refuse => Self::Refuse,
            MutationWait::For(budget) => Self::At(now, now + budget),
            MutationWait::Forever => Self::Forever(now),
        }
    }

    fn expired(&self) -> bool {
        match self {
            Self::Refuse => true,
            Self::At(_, end) => Instant::now() >= *end,
            Self::Forever(_) => false,
        }
    }

    fn waits(&self) -> bool {
        !matches!(self, Self::Refuse)
    }

    fn waited(&self) -> String {
        match self {
            Self::Refuse => String::new(),
            Self::At(start, _) | Self::Forever(start) => format!(" after waiting {}s", start.elapsed().as_secs()),
        }
    }

    fn pause(&self, pause: Duration) -> Duration {
        match self {
            Self::At(_, end) => pause.min(end.saturating_duration_since(Instant::now())),
            Self::Refuse | Self::Forever(_) => pause,
        }
    }
}

fn try_lock(file: &fs::File, mode: Mode) -> Result<bool> {
    use rustix::fs::FlockOperation;
    let operation = match mode {
        Mode::Exclusive => FlockOperation::NonBlockingLockExclusive,
        Mode::Shared => FlockOperation::NonBlockingLockShared,
    };
    match rustix::fs::flock(file, operation) {
        Ok(()) => Ok(true),
        Err(errno) if errno == rustix::io::Errno::WOULDBLOCK => Ok(false),
        Err(errno) => Err(std::io::Error::from(errno)).context("could not lock the Brit mutation guard"),
    }
}

fn would_block() -> std::io::Error {
    std::io::Error::from(rustix::io::Errno::WOULDBLOCK)
}

fn inside_hook() -> Option<u32> {
    std::env::var(HOOK_MARKER).ok()?.parse().ok()
}

/// This process's ancestors, nearest first, from the `/proc` parent chain.
fn ancestors() -> Vec<u32> {
    let mut out = Vec::new();
    let mut pid = std::os::unix::process::parent_id();
    while pid > 1 && out.len() < 64 && !out.contains(&pid) {
        out.push(pid);
        let Some(parent) = fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| parent_of(&stat))
        else {
            break;
        };
        pid = parent;
    }
    out
}

/// The parent pid in a `/proc/<pid>/stat` line: the second field after the command name.
fn parent_of(stat: &str) -> Option<u32> {
    stat.get(stat.rfind(')')? + 2..)?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// Why waiting for these holders can never succeed, if it cannot.
fn reentrant(holders: &[(u32, bool)], level: Level) -> Option<String> {
    let me = std::process::id();
    if let Some(holder) = holders.iter().find(|(pid, _)| *pid == me) {
        return Some(format!(
            "the Brit mutation guard is held by this command itself ({})",
            describe(holder.0, role(level, holder.1))
        ));
    }
    let marker = inside_hook();
    let lineage = ancestors();
    holders
        .iter()
        .find(|(pid, _)| Some(*pid) == marker || lineage.contains(pid))
        .map(|holder| {
            format!(
                "the Brit mutation guard is held by {}, the command this hook runs under; a hook cannot wait for its own command's guard",
                describe(holder.0, role(level, holder.1))
            )
        })
}

fn lock(file: &fs::File, level: Level, mode: Mode, deadline: &Deadline) -> Result<()> {
    let mut retried = false;
    let mut announced = false;
    let mut pause = Duration::from_millis(20);
    loop {
        if try_lock(file, mode)? {
            return Ok(());
        }
        let holders = holders_of(file);
        if let Some(reason) = reentrant(&holders, level) {
            return Err(would_block()).context(format!("another Brit mutation is active; {reason}"));
        }
        if holders.is_empty() && !retried {
            // The holder may have released between the attempt and the lookup.
            retried = true;
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        let unidentified_under_hook = holders.is_empty() && inside_hook().is_some();
        if deadline.expired() || unidentified_under_hook {
            let because = if unidentified_under_hook && !deadline.expired() {
                format!(
                    "; not waiting, because this command runs under a Brit hook ({HOOK_MARKER} is set) and the holder may be that command"
                )
            } else {
                String::new()
            };
            return Err(would_block()).with_context(|| {
                format!(
                    "another Brit mutation is active{}; held by {}{because}; retry with --wait[=SECONDS] or BRIT_MUTATION_WAIT=SECONDS",
                    deadline.waited(),
                    render(&holders, level)
                )
            });
        }
        if !announced {
            eprintln!(
                "brit: waiting for the Brit mutation guard held by {}",
                render(&holders, level)
            );
            announced = true;
        }
        if gix::interrupt::is_triggered() {
            bail!("interrupted while waiting for the Brit mutation guard; nothing was changed");
        }
        std::thread::sleep(deadline.pause(pause));
        pause = (pause * 2).min(Duration::from_millis(250));
    }
}

/// Take the repository guard exclusively, queueing at the turnstile once it has to wait.
fn lock_repository_exclusive(repo: &gix::Repository, file: &fs::File, deadline: &Deadline) -> Result<()> {
    if try_lock(file, Mode::Exclusive)? {
        return Ok(());
    }
    if !deadline.waits() {
        return lock(file, Level::Repository, Mode::Exclusive, deadline);
    }
    let turnstile = open(&repo.common_dir().join(TURNSTILE))?;
    lock(&turnstile, Level::Turnstile, Mode::Exclusive, deadline)?;
    lock(file, Level::Repository, Mode::Exclusive, deadline)
}

/// Let a shared acquirer through only when no exclusive writer is queued.
fn pass_turnstile(repo: &gix::Repository, deadline: &Deadline) -> Result<()> {
    // A hook's parent may hold the repository guard that a queued writer waits for.
    if inside_hook().is_some() {
        return Ok(());
    }
    use std::os::unix::fs::OpenOptionsExt;
    let path = repo.common_dir().join(TURNSTILE);
    let turnstile = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(&path)
    {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("could not open {}", path.display())),
    };
    lock(&turnstile, Level::Turnstile, Mode::Shared, deadline)
}

/// Take `repo`'s worktree guard (when it has a worktree and `worktree` is set), then the
/// repository guard in `mode`, under one deadline.
pub(crate) fn acquire(repo: &gix::Repository, mode: Mode, worktree: bool, wait: MutationWait) -> Result<Held> {
    let deadline = Deadline::new(wait);
    let worktree = if worktree && repo.workdir().is_some() {
        let directory = open_directory(repo.git_dir())?;
        lock(&directory, Level::Worktree, Mode::Exclusive, &deadline)?;
        Some(directory)
    } else {
        None
    };
    let file = open(&repo.common_dir().join(REPOSITORY_GUARD))?;
    match mode {
        Mode::Exclusive => lock_repository_exclusive(repo, &file, &deadline)?,
        Mode::Shared => {
            pass_turnstile(repo, &deadline)?;
            lock(&file, Level::Repository, Mode::Shared, &deadline)?;
        }
    }
    Ok(Held {
        repository: Some(file),
        _worktree: worktree,
    })
}

/// Take another worktree's guard without waiting, while the repository guard is held.
pub(crate) fn other_worktree(git_dir: &Path) -> Result<fs::File> {
    let directory = open_directory(git_dir)?;
    lock(&directory, Level::Worktree, Mode::Exclusive, &Deadline::Refuse)?;
    Ok(directory)
}

fn role(level: Level, shared: bool) -> &'static str {
    match (level, shared) {
        (Level::Repository, false) => "shared write",
        (Level::Repository, true) | (Level::Worktree, _) => "worktree write",
        (Level::Turnstile, _) => "queued shared write",
    }
}

fn render(holders: &[(u32, bool)], level: Level) -> String {
    if holders.is_empty() {
        return "an unidentified process (no holder visible in /proc/locks)".into();
    }
    holders
        .iter()
        .map(|(pid, shared)| describe(*pid, role(level, *shared)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn holders_of(file: &fs::File) -> Vec<(u32, bool)> {
    rustix::fs::fstat(file)
        .ok()
        .and_then(|stat| {
            fs::read_to_string("/proc/locks")
                .ok()
                .map(|table| lock_holders(&table, &stat))
        })
        .unwrap_or_default()
}

/// `(pid, shared)` for each granted lock on the file, from a `/proc/locks` table.
/// Device and inode must both match, so a lock on another filesystem is never named.
fn lock_holders(table: &str, stat: &rustix::fs::Stat) -> Vec<(u32, bool)> {
    let device = format!(
        "{:02x}:{:02x}:{}",
        rustix::fs::major(stat.st_dev),
        rustix::fs::minor(stat.st_dev),
        stat.st_ino
    );
    let mut holders = Vec::new();
    for line in table.lines() {
        // `N: FLOCK  ADVISORY  WRITE 1234 08:02:131074 0 EOF`; blocked waiters carry `->`.
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 6 || fields[1] != "FLOCK" || fields[5] != device {
            continue;
        }
        if let Ok(pid) = fields[4].parse::<u32>() {
            holders.push((pid, fields[3] == "READ"));
        }
    }
    holders
}

fn describe(pid: u32, role: &str) -> String {
    use std::fmt::Write as _;
    let proc = Path::new("/proc").join(pid.to_string());
    let Ok(cmdline) = fs::read(proc.join("cmdline")) else {
        return format!("pid {pid} ({role}; no longer visible)");
    };
    let argv: Vec<String> = cmdline
        .split(|byte| *byte == 0)
        .filter(|arg| !arg.is_empty())
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect();
    let mut out = format!("pid {pid} `{}` ({role}", super::super::redacted_command(&argv));
    if let Ok(cwd) = fs::read_link(proc.join("cwd")) {
        let _ = write!(out, " in {}", cwd.display());
    }
    if let Some(started) = started_at(&proc) {
        let _ = write!(out, ", since {started}");
    }
    if let Some(session) = session(&proc) {
        let _ = write!(out, ", session {session}");
    }
    out.push(')');
    out
}

/// The process start time in UTC, from `/proc/<pid>/stat` and the boot time.
fn started_at(proc: &Path) -> Option<String> {
    let stat = fs::read_to_string(proc.join("stat")).ok()?;
    // Fields after the parenthesised command name; start time is field 22 overall.
    let after_name = stat.get(stat.rfind(')')? + 2..)?;
    let ticks: u64 = after_name.split_whitespace().nth(19)?.parse().ok()?;
    let boot: i64 = fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("btime "))?
        .trim()
        .parse()
        .ok()?;
    let per_second = rustix::param::clock_ticks_per_second();
    let seconds = boot.checked_add(i64::try_from(ticks / per_second).ok()?)?;
    gix::date::Time::new(seconds, 0)
        .format(gix::date::time::format::ISO8601_STRICT)
        .ok()
}

/// A session identifier the holder's environment declares, when readable.
fn session(proc: &Path) -> Option<String> {
    let environ = fs::read(proc.join("environ")).ok()?;
    ["BRIT_SESSION=", "CLAUDE_CODE_SESSION_ID="].iter().find_map(|key| {
        environ
            .split(|byte| *byte == 0)
            .find_map(|entry| entry.strip_prefix(key.as_bytes()))
            .filter(|value| !value.is_empty())
            .map(|value| String::from_utf8_lossy(value).into_owned())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn parent_is_read_after_a_command_name_with_spaces_and_parentheses() {
        assert_eq!(super::parent_of("123 (brit (x) y) S 77 123 123 0 -1"), Some(77));
    }

    #[test]
    fn lock_holders_require_the_device_as_well_as_the_inode() {
        let stat = rustix::fs::stat("/").expect("root is stat-able");
        let here = format!(
            "{:02x}:{:02x}:{}",
            rustix::fs::major(stat.st_dev),
            rustix::fs::minor(stat.st_dev),
            stat.st_ino
        );
        let elsewhere = format!("ff:ff:{}", stat.st_ino);
        let table = format!(
            "1: FLOCK  ADVISORY  WRITE 11 {here} 0 EOF\n\
             2: FLOCK  ADVISORY  READ  22 {here} 0 EOF\n\
             3: FLOCK  ADVISORY  WRITE 33 {elsewhere} 0 EOF\n\
             4: POSIX  ADVISORY  WRITE 44 {here} 0 EOF\n"
        );
        assert_eq!(super::lock_holders(&table, &stat), vec![(11, false), (22, true)]);
    }
}
