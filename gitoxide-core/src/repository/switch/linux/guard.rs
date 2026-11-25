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
//! Pushes also hold `<common-dir>/brit-objects.pin` shared through the pre-push hook and send.
//! Pack consolidation tries to take it exclusively without waiting. A push keeps this pin
//! when it releases the repository guard, so other worktrees can still commit and fetch.
//!
//! Order: a worktree guard is always taken before the repository guard, and nobody waits for
//! a worktree guard while holding the repository guard. Waiting for the repository guard while
//! holding a worktree guard is allowed (push re-acquires the repository guard after its
//! pre-push hook while still holding its worktree), so no cycle can form.
//!
//! A process never waits for a guard held by itself or by one of its ancestors: a hook that
//! runs a Brit writer would otherwise wait forever on the command it runs under. Every hook
//! and editor Brit spawns carries [`HOOK_MARKER`]: the spawning process's pid and start time
//! and exactly which guards it holds. A marker is honoured only when that pid with that start
//! time is in this process's parent chain; a forged or inherited-by-a-daemon marker is ignored.
//! Under an honoured marker:
//! - a request the ancestor's holdings make impossible (its worktree guard; the repository guard
//!   in a mode incompatible with the ancestor's) refuses at once, before any queueing;
//! - a shared request passes the turnstile when the ancestor holds the repository guard shared
//!   (a queued writer may be waiting for that very ancestor);
//! - no single acquisition waits longer than the hook cap ([`HOOK_WAIT`], default 30 s), which
//!   breaks any longer cycle through other processes.
//!
//! The `/proc/locks` ancestor check still covers hook children whose environment was cleared.
//!
//! A refusal names the holders the kernel records in `/proc/locks`, which never goes stale and
//! needs no extra files in the repository. Holder command lines are reduced to the program
//! name and Brit subcommand path, so option values and positionals (credentials) never leak.
use super::super::MutationWait;
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Environment variable naming the Brit process whose hook or editor this process runs under,
/// with its start time and the guards it held when it spawned the hook.
pub const HOOK_MARKER: &str = "BRIT_GUARD_HELD_BY";

/// Environment variable capping, in seconds, any single guard wait under an honoured
/// [`HOOK_MARKER`] (default 30).
pub const HOOK_WAIT: &str = "BRIT_HOOK_GUARD_WAIT";

const DEFAULT_HOOK_WAIT: Duration = Duration::from_secs(30);

const REPOSITORY_GUARD: &str = "brit-mutation.guard";
const TURNSTILE: &str = "brit-mutation.turnstile";

/// What one [`Held`] holds, as published in the hook marker.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Holding {
    common: PathBuf,
    repository: Option<Mode>,
    worktree: Option<PathBuf>,
}

static HOLDINGS: Mutex<BTreeMap<u64, Holding>> = Mutex::new(BTreeMap::new());
static NEXT_HOLDING: AtomicU64 = AtomicU64::new(0);

fn record(holding: Holding) -> u64 {
    let id = NEXT_HOLDING.fetch_add(1, Ordering::Relaxed);
    HOLDINGS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(id, holding);
    id
}

fn update(id: u64, repository: Option<Mode>) {
    if let Some(holding) = HOLDINGS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get_mut(&id)
    {
        holding.repository = repository;
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

/// Guards released together when dropped.
#[derive(Debug)]
pub struct Held {
    id: u64,
    // Dropped in order: repository guard first, then the worktree guard.
    repository: Option<fs::File>,
    _worktree: Option<fs::File>,
}

impl Drop for Held {
    fn drop(&mut self) {
        HOLDINGS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

/// A worktree-local writer's guards: the repository guard shared, its worktree's exclusive.
pub type WorktreeGuard = Held;

impl Held {
    /// Release the repository guard, keeping the worktree guard.
    pub fn release_repository(&mut self) {
        self.repository = None;
        update(self.id, None);
    }

    /// Re-take the repository guard in `mode` while the worktree guard stays held.
    pub(crate) fn reacquire_repository(
        &mut self,
        repo: &gix::Repository,
        mode: Mode,
        wait: MutationWait,
    ) -> Result<()> {
        if let Some(refusal) = self_deadlock(repo, Some(mode), false) {
            return Err(would_block()).context(refusal);
        }
        let deadline = Deadline::new(wait);
        let file = open(&repo.common_dir().join(REPOSITORY_GUARD))?;
        lock_repository(repo, &file, mode, &deadline)?;
        self.repository = Some(file);
        update(self.id, Some(mode));
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    ObjectPin,
}

/// One deadline across every lock of an acquisition.
enum Deadline {
    Refuse,
    At(Instant, Instant),
    Forever(Instant),
    /// Shortened to the hook cap: the holder may be waiting for this hook's command.
    Capped(Instant, Instant, Duration),
}

impl Deadline {
    fn new(wait: MutationWait) -> Self {
        let now = Instant::now();
        let cap = hook_cap();
        match (wait, cap) {
            (MutationWait::Refuse, _) => Self::Refuse,
            (MutationWait::For(budget), Some(cap)) if budget > cap => Self::Capped(now, now + cap, cap),
            (MutationWait::Forever, Some(cap)) => Self::Capped(now, now + cap, cap),
            (MutationWait::For(budget), _) => Self::At(now, now + budget),
            (MutationWait::Forever, None) => Self::Forever(now),
        }
    }

    fn grace(grace: Duration) -> Self {
        let now = Instant::now();
        Self::At(now, now + grace)
    }

    fn capped(&self) -> String {
        match self {
            Self::Capped(_, _, cap) => format!(
                "; the wait was capped at {}s because the holder may itself be waiting for the command this hook runs under ({HOOK_WAIT} sets the cap)",
                cap.as_secs()
            ),
            _ => String::new(),
        }
    }

    fn expired(&self) -> bool {
        match self {
            Self::Refuse => true,
            Self::At(_, end) | Self::Capped(_, end, _) => Instant::now() >= *end,
            Self::Forever(_) => false,
        }
    }

    fn waits(&self) -> bool {
        !matches!(self, Self::Refuse)
    }

    fn waited(&self) -> String {
        match self {
            Self::Refuse => String::new(),
            Self::At(start, _) | Self::Forever(start) | Self::Capped(start, _, _) => {
                format!(" after waiting {}s", start.elapsed().as_secs())
            }
        }
    }

    fn pause(&self, pause: Duration) -> Duration {
        match self {
            Self::At(_, end) | Self::Capped(_, end, _) => pause.min(end.saturating_duration_since(Instant::now())),
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

/// The start time of `pid` in clock ticks since boot (`/proc/<pid>/stat` field 22).
fn start_ticks(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.get(stat.rfind(')')? + 2..)?
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

fn hex(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn unhex(text: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let bytes = (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    Some(std::ffi::OsString::from_vec(bytes).into())
}

/// The marker for a hook or editor this process spawns: `v1 <pid> <start> <holding>...`,
/// each holding `<n|s|x>:<hex common dir>:<hex worktree git dir or ->`.
pub fn hook_marker() -> String {
    use std::fmt::Write as _;
    let pid = std::process::id();
    let mut out = format!("v1 {pid} {}", start_ticks(pid).unwrap_or(0));
    for holding in HOLDINGS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
    {
        let mode = match holding.repository {
            None => 'n',
            Some(Mode::Shared) => 's',
            Some(Mode::Exclusive) => 'x',
        };
        let worktree = holding.worktree.as_deref().map_or_else(|| "-".to_owned(), hex);
        let _ = write!(out, " {mode}:{}:{worktree}", hex(&holding.common));
    }
    out
}

/// The marker this process inherited, as far as it can be trusted.
enum Marker {
    /// None, malformed, or naming a process that is not this process's ancestor.
    Ignored,
    /// `/proc` cannot prove or disprove ancestry: only the wait cap applies.
    Unprovable,
    /// The named process (with that start time) is an ancestor holding these guards.
    Proven(u32, Vec<Holding>),
}

fn parse_marker(text: &str) -> Option<(u32, u64, Vec<Holding>)> {
    let mut fields = text.split(' ');
    (fields.next()? == "v1").then_some(())?;
    let pid = fields.next()?.parse().ok()?;
    let start = fields.next()?.parse().ok()?;
    let holdings = fields
        .map(|field| {
            let mut parts = field.split(':');
            let repository = match parts.next()? {
                "n" => None,
                "s" => Some(Mode::Shared),
                "x" => Some(Mode::Exclusive),
                _ => return None,
            };
            let common = unhex(parts.next()?)?;
            let worktree = match parts.next()? {
                "-" => None,
                hexed => Some(unhex(hexed)?),
            };
            Some(Holding {
                common,
                repository,
                worktree,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some((pid, start, holdings))
}

fn marker() -> &'static Marker {
    static MARKER: OnceLock<Marker> = OnceLock::new();
    MARKER.get_or_init(|| {
        let Ok(text) = std::env::var(HOOK_MARKER) else {
            return Marker::Ignored;
        };
        let Some((pid, start, holdings)) = parse_marker(&text) else {
            return Marker::Ignored;
        };
        if start_ticks(std::process::id()).is_none() {
            return Marker::Unprovable;
        }
        if ancestors().contains(&pid) && start_ticks(pid) == Some(start) {
            Marker::Proven(pid, holdings)
        } else {
            Marker::Ignored
        }
    })
}

/// The longest any single acquisition may wait while running under a Brit hook.
fn hook_cap() -> Option<Duration> {
    match marker() {
        Marker::Ignored => None,
        Marker::Unprovable | Marker::Proven(..) => Some(
            std::env::var(HOOK_WAIT)
                .ok()
                .and_then(|value| value.trim().parse().ok())
                .map_or(DEFAULT_HOOK_WAIT, Duration::from_secs),
        ),
    }
}

/// Whether the proven ancestor holds `repo`'s repository guard shared.
fn ancestor_holds_shared(repo: &gix::Repository) -> bool {
    let Marker::Proven(_, holdings) = marker() else {
        return false;
    };
    let common = canonical(repo.common_dir());
    holdings
        .iter()
        .any(|holding| holding.common == common && holding.repository == Some(Mode::Shared))
}

/// Appended when a hook asks for its own command's worktree guard.
const STAGING_HINT: &str = "; a hook that stages files must write the index directly (for example with `git add`), not through Brit (see docs/mutation-guards.md)";

/// A request the proven ancestor's holdings make impossible to grant while it waits for us.
fn self_deadlock(repo: &gix::Repository, repository: Option<Mode>, worktree: bool) -> Option<String> {
    let Marker::Proven(pid, holdings) = marker() else {
        return None;
    };
    let common = canonical(repo.common_dir());
    let git_dir = canonical(repo.git_dir());
    let held = holdings.iter().find_map(|holding| {
        if holding.common != common {
            return None;
        }
        if worktree && repo.workdir().is_some() && holding.worktree.as_deref() == Some(git_dir.as_path()) {
            return Some("this worktree's guard");
        }
        match (repository, holding.repository) {
            (Some(Mode::Exclusive), Some(Mode::Exclusive | Mode::Shared))
            | (Some(Mode::Shared), Some(Mode::Exclusive)) => Some("the repository guard"),
            _ => None,
        }
    })?;
    let hint = if held == "this worktree's guard" {
        STAGING_HINT
    } else {
        ""
    };
    Some(format!(
        "another Brit mutation is active; {held} is held by {}, the command this hook runs under; a hook cannot wait for its own command's guard{hint}",
        describe(*pid, "hook's command")
    ))
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
    let lineage = ancestors();
    holders
        .iter()
        .find(|(pid, _)| lineage.contains(pid))
        .map(|holder| {
            format!(
                "the Brit mutation guard is held by {}, the command this hook runs under; a hook cannot wait for its own command's guard{}",
                describe(holder.0, role(level, holder.1)),
                if matches!(level, Level::Worktree) { STAGING_HINT } else { "" }
            )
        })
}

/// How a lock wait treats its holders.
#[derive(Clone, Copy, Default)]
struct Waiting<'a> {
    /// Proceed without the lock when every holder is a stopped process (a fairness lock only).
    pass_stopped: bool,
    /// The repository guard a turnstile holder is queued for, to name what really blocks.
    queued_for: Option<&'a fs::File>,
}

fn lock(file: &fs::File, level: Level, mode: Mode, deadline: &Deadline) -> Result<()> {
    lock_with(file, level, mode, deadline, Waiting::default()).map(|_| ())
}

/// `Ok(true)` when the lock is held, `Ok(false)` when passed because its holders are stopped.
fn lock_with(file: &fs::File, level: Level, mode: Mode, deadline: &Deadline, waiting: Waiting<'_>) -> Result<bool> {
    let describe_holders = |holders: &[(u32, bool)]| match waiting.queued_for {
        Some(repository) => {
            let mut blocking = holders_of(repository);
            if blocking.is_empty() {
                // The holder may have released between the attempt and the lookup.
                std::thread::sleep(Duration::from_millis(5));
                blocking = holders_of(repository);
            }
            let blocking = if blocking.is_empty() {
                "nobody now (the queued writer is about to take it)".to_owned()
            } else {
                render(&blocking, Level::Repository)
            };
            format!(
                "{}, queued for the repository guard, which is held by {blocking}",
                render(holders, level)
            )
        }
        None => render(holders, level),
    };
    let mut retried = false;
    let mut announced = false;
    let mut pause = Duration::from_millis(20);
    loop {
        if try_lock(file, mode)? {
            return Ok(true);
        }
        let holders = holders_of(file);
        if let Some(reason) = reentrant(&holders, level) {
            return Err(would_block()).context(format!("another Brit mutation is active; {reason}"));
        }
        if waiting.pass_stopped && !holders.is_empty() && holders.iter().all(|(pid, _)| stopped(*pid)) {
            return Ok(false);
        }
        if holders.is_empty() && !retried {
            // The holder may have released between the attempt and the lookup.
            retried = true;
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        if deadline.expired() {
            return Err(would_block()).with_context(|| {
                format!(
                    "another Brit mutation is active{}; held by {}{}; retry with --wait[=SECONDS] or BRIT_MUTATION_WAIT=SECONDS",
                    deadline.waited(),
                    describe_holders(&holders),
                    deadline.capped()
                )
            });
        }
        if !announced {
            eprintln!(
                "brit: waiting for the Brit mutation guard held by {}",
                describe_holders(&holders)
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

fn lock_repository(repo: &gix::Repository, file: &fs::File, mode: Mode, deadline: &Deadline) -> Result<()> {
    match mode {
        Mode::Exclusive => lock_repository_exclusive(repo, file, deadline),
        Mode::Shared => {
            pass_turnstile(repo, file, deadline)?;
            lock(file, Level::Repository, Mode::Shared, deadline)
        }
    }
}

/// How long a shared acquirer that asked not to wait still waits at an engaged turnstile:
/// the turnstile is a fairness device, not a holder.
const TURNSTILE_GRACE: Duration = Duration::from_secs(1);

/// Take the repository guard exclusively, queueing at the turnstile once it has to wait.
fn lock_repository_exclusive(repo: &gix::Repository, file: &fs::File, deadline: &Deadline) -> Result<()> {
    if try_lock(file, Mode::Exclusive)? {
        return Ok(());
    }
    if !deadline.waits() {
        return lock(file, Level::Repository, Mode::Exclusive, deadline);
    }
    // A turnstile that cannot be opened (another user's checkout) only loses fairness.
    let turnstile = open(&repo.common_dir().join(TURNSTILE)).ok();
    if let Some(turnstile) = &turnstile {
        let waiting = Waiting {
            pass_stopped: true,
            queued_for: Some(file),
        };
        lock_with(turnstile, Level::Turnstile, Mode::Exclusive, deadline, waiting)?;
    }
    lock(file, Level::Repository, Mode::Exclusive, deadline)
}

/// Let a shared acquirer through only when no exclusive writer is queued.
fn pass_turnstile(repo: &gix::Repository, repository: &fs::File, deadline: &Deadline) -> Result<()> {
    // A queued writer may be waiting for the shared hold of the command this hook runs under.
    if ancestor_holds_shared(repo) {
        return Ok(());
    }
    use std::os::unix::fs::OpenOptionsExt;
    // Absent, or not ours to open: no exclusive writer is queued that we could honour.
    let Ok(turnstile) = fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(repo.common_dir().join(TURNSTILE))
    else {
        return Ok(());
    };
    let grace;
    let deadline = if deadline.waits() {
        deadline
    } else {
        grace = Deadline::grace(TURNSTILE_GRACE);
        &grace
    };
    let waiting = Waiting {
        pass_stopped: true,
        queued_for: Some(repository),
    };
    lock_with(&turnstile, Level::Turnstile, Mode::Shared, deadline, waiting).map(|_| ())
}

/// Whether `pid` is stopped (`T`) or traced (`t`), per `/proc/<pid>/stat`.
fn stopped(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.get(stat.rfind(')')? + 2..)?
                .split_whitespace()
                .next()
                .map(|state| matches!(state, "T" | "t"))
        })
        .unwrap_or(false)
}

/// Take `repo`'s worktree guard (when it has a worktree and `worktree` is set), then the
/// repository guard in `mode`, under one deadline.
pub(crate) fn acquire(repo: &gix::Repository, mode: Mode, worktree: bool, wait: MutationWait) -> Result<Held> {
    if let Some(refusal) = self_deadlock(repo, Some(mode), worktree) {
        return Err(would_block()).context(refusal);
    }
    let deadline = Deadline::new(wait);
    let worktree = if worktree && repo.workdir().is_some() {
        let directory = open_directory(repo.git_dir())?;
        lock(&directory, Level::Worktree, Mode::Exclusive, &deadline)?;
        Some(directory)
    } else {
        None
    };
    let file = open(&repo.common_dir().join(REPOSITORY_GUARD))?;
    lock_repository(repo, &file, mode, &deadline)?;
    let id = record(Holding {
        common: canonical(repo.common_dir()),
        repository: Some(mode),
        worktree: worktree.as_ref().map(|_| canonical(repo.git_dir())),
    });
    Ok(Held {
        id,
        repository: Some(file),
        _worktree: worktree,
    })
}

/// Take another worktree's guard (`target` opened on it) without waiting, while the
/// repository guard is held.
pub(crate) fn other_worktree(target: &gix::Repository) -> Result<Held> {
    if let Some(refusal) = self_deadlock(target, None, true) {
        return Err(would_block()).context(refusal);
    }
    let directory = open_directory(target.git_dir())?;
    lock(&directory, Level::Worktree, Mode::Exclusive, &Deadline::Refuse)?;
    let id = record(Holding {
        common: canonical(target.common_dir()),
        repository: None,
        worktree: Some(canonical(target.git_dir())),
    });
    Ok(Held {
        id,
        repository: None,
        _worktree: Some(directory),
    })
}

fn role(level: Level, shared: bool) -> &'static str {
    match (level, shared) {
        (Level::Repository, false) => "holds the repository guard exclusively",
        (Level::Repository, true) => "holds the repository guard shared",
        (Level::Worktree, _) => "holds this worktree's guard",
        (Level::Turnstile, _) => "queued for the repository guard",
        (Level::ObjectPin, _) => "holds the object pin",
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

pub(crate) fn describe_object_pin_holders(file: &fs::File) -> String {
    render(&holders_of(file), Level::ObjectPin)
}

fn holders_of(file: &fs::File) -> Vec<(u32, bool)> {
    rustix::fs::fstat(file)
        .ok()
        .and_then(|stat| {
            fs::read_to_string("/proc/locks")
                .ok()
                .map(|table| lock_holders(&table, &stat, |pid| holds_open(pid, &stat)))
        })
        .unwrap_or_default()
}

/// Whether `pid` has the file described by `stat` open, from its `/proc/<pid>/fd` entries.
fn holds_open(pid: u32, stat: &rustix::fs::Stat) -> bool {
    use std::os::unix::fs::MetadataExt;
    fs::read_dir(format!("/proc/{pid}/fd")).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            fs::metadata(entry.path()).is_ok_and(|meta| meta.dev() == stat.st_dev && meta.ino() == stat.st_ino)
        })
    })
}

/// `(pid, shared)` for each granted lock on the file, from a `/proc/locks` table.
/// An entry whose device and inode both match is the file's. An entry matching only the inode
/// (some filesystems report a different device there, such as btrfs subvolumes) counts only
/// when `confirm` shows that process has this very file open.
fn lock_holders(table: &str, stat: &rustix::fs::Stat, confirm: impl Fn(u32) -> bool) -> Vec<(u32, bool)> {
    let device = format!(
        "{:02x}:{:02x}:{}",
        rustix::fs::major(stat.st_dev),
        rustix::fs::minor(stat.st_dev),
        stat.st_ino
    );
    let inode = stat.st_ino.to_string();
    let mut holders = Vec::new();
    for line in table.lines() {
        // `N: FLOCK  ADVISORY  WRITE 1234 08:02:131074 0 EOF`; blocked waiters carry `->`.
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 6 || fields[1] != "FLOCK" {
            continue;
        }
        let Ok(pid) = fields[4].parse::<u32>() else {
            continue;
        };
        let same_inode = fields[5].rsplit(':').next() == Some(inode.as_str());
        if fields[5] == device || (same_inode && confirm(pid)) {
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
    fn hook_markers_parse_back_including_odd_paths() {
        let holdings = vec![
            super::Holding {
                common: "/repo/with space:colon/.git".into(),
                repository: Some(super::Mode::Shared),
                worktree: Some("/repo/.git/worktrees/x".into()),
            },
            super::Holding {
                common: "/other/.git".into(),
                repository: None,
                worktree: None,
            },
        ];
        let encoded = format!(
            "v1 42 7 s:{}:{} n:{}:-",
            super::hex(&holdings[0].common),
            super::hex(holdings[0].worktree.as_deref().expect("set")),
            super::hex(&holdings[1].common)
        );
        assert_eq!(super::parse_marker(&encoded), Some((42, 7, holdings)));
        for malformed in ["", "v2 1 2", "v1 x 2", "v1 1 2 q:00:-", "v1 1 2 s:0:-", "42"] {
            assert_eq!(super::parse_marker(malformed), None, "{malformed:?}");
        }
    }

    #[test]
    fn parent_is_read_after_a_command_name_with_spaces_and_parentheses() {
        assert_eq!(super::parent_of("123 (brit (x) y) S 77 123 123 0 -1"), Some(77));
    }

    #[test]
    fn lock_holders_match_by_device_or_by_a_confirmed_open_file() {
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
             4: FLOCK  ADVISORY  WRITE 44 {elsewhere} 0 EOF\n\
             5: POSIX  ADVISORY  WRITE 55 {here} 0 EOF\n\
             6: FLOCK  ADVISORY  WRITE 66 ff:ff:1{} 0 EOF\n",
            stat.st_ino
        );
        assert_eq!(
            super::lock_holders(&table, &stat, |pid| pid == 33 || pid == 66),
            vec![(11, false), (22, true), (33, false)],
            "another device counts only when confirmed, and only for the same inode"
        );
    }
}
