//! Cooperative writer guards for one repository and its linked worktrees.
//!
//! Two advisory `flock` files order every Brit writer:
//!
//! - `<common-dir>/brit-mutation.guard` is the repository guard. A writer of shared state
//!   (references, packed-refs, configuration, worktree registrations, switch/pull/replay
//!   journals, object-store maintenance) holds it exclusively. A worktree-local writer holds
//!   it shared, so it excludes every shared writer but not other worktree-local writers.
//! - The worktree's private Git directory (`<git-dir>` itself, locked as a directory) is one
//!   worktree's guard. A worktree-local writer holds it exclusively, serializing writers of
//!   that worktree's index, HEAD and files. Locking the existing directory adds no file.
//!
//! Locks are always taken repository guard first, so the two levels cannot deadlock.
//! A refusal names the holders the kernel records for the guard in `/proc/locks`, which
//! never goes stale and needs no extra files in the repository.
use super::super::MutationWait;
use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

const REPOSITORY_GUARD: &str = "brit-mutation.guard";

/// A worktree-local writer's guards: the repository guard shared, its worktree's exclusive.
/// Field order releases the worktree guard before the repository guard.
pub struct WorktreeGuard {
    _worktree: fs::File,
    _repository: fs::File,
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

#[derive(Clone, Copy)]
enum Mode {
    Exclusive,
    Shared,
}

/// Which guard a file is, so a holder's lock reads as the write it performs.
#[derive(Clone, Copy)]
enum Level {
    Repository,
    Worktree,
}

fn lock(file: &fs::File, level: Level, mode: Mode, wait: MutationWait) -> Result<()> {
    use rustix::fs::FlockOperation;
    let operation = match mode {
        Mode::Exclusive => FlockOperation::NonBlockingLockExclusive,
        Mode::Shared => FlockOperation::NonBlockingLockShared,
    };
    let started = Instant::now();
    let mut announced = false;
    let mut pause = Duration::from_millis(20);
    loop {
        let errno = match rustix::fs::flock(file, operation) {
            Ok(()) => return Ok(()),
            Err(errno) if errno == rustix::io::Errno::WOULDBLOCK => errno,
            Err(errno) => return Err(std::io::Error::from(errno)).context("could not lock the Brit mutation guard"),
        };
        let expired = match wait {
            MutationWait::Refuse => true,
            MutationWait::For(budget) => started.elapsed() >= budget,
            MutationWait::Forever => false,
        };
        if expired {
            let waited = match wait {
                MutationWait::Refuse => String::new(),
                _ => format!(" after waiting {}s", started.elapsed().as_secs()),
            };
            return Err(std::io::Error::from(errno)).with_context(|| {
                format!(
                    "another Brit mutation is active{waited}; held by {}; retry with --wait[=SECONDS] or BRIT_MUTATION_WAIT=SECONDS",
                    holders(file, level)
                )
            });
        }
        if !announced {
            eprintln!(
                "brit: waiting for the Brit mutation guard held by {}",
                holders(file, level)
            );
            announced = true;
        }
        if gix::interrupt::is_triggered() {
            bail!("interrupted while waiting for the Brit mutation guard; nothing was changed");
        }
        std::thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_millis(250));
    }
}

/// Hold the repository guard exclusively for a shared write, waiting per `wait`.
pub(crate) fn exclusive(repo: &gix::Repository, wait: MutationWait) -> Result<fs::File> {
    let file = open(&repo.common_dir().join(REPOSITORY_GUARD))?;
    lock(&file, Level::Repository, Mode::Exclusive, wait)?;
    Ok(file)
}

/// Hold the repository guard shared and `repo`'s worktree guard exclusively.
pub(crate) fn worktree(repo: &gix::Repository, wait: MutationWait) -> Result<WorktreeGuard> {
    let repository = open(&repo.common_dir().join(REPOSITORY_GUARD))?;
    lock(&repository, Level::Repository, Mode::Shared, wait)?;
    let worktree = open_directory(repo.git_dir())?;
    lock(&worktree, Level::Worktree, Mode::Exclusive, wait)?;
    Ok(WorktreeGuard {
        _worktree: worktree,
        _repository: repository,
    })
}

/// Describe the processes the kernel records as holding `file`'s lock.
fn holders(file: &fs::File, level: Level) -> String {
    let described = match rustix::fs::fstat(file) {
        Ok(stat) => lock_holders(&stat)
            .into_iter()
            .map(|(pid, shared)| {
                let role = match (level, shared) {
                    (Level::Repository, false) => "shared write",
                    (Level::Repository, true) | (Level::Worktree, _) => "worktree write",
                };
                describe(pid, role)
            })
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    if described.is_empty() {
        "an unidentified process (no holder visible in /proc/locks)".into()
    } else {
        described.join("; ")
    }
}

/// `(pid, shared)` for each granted lock on the file, from `/proc/locks`.
fn lock_holders(stat: &rustix::fs::Stat) -> Vec<(u32, bool)> {
    let Ok(table) = fs::read_to_string("/proc/locks") else {
        return Vec::new();
    };
    let device = format!(
        "{:02x}:{:02x}:{}",
        rustix::fs::major(stat.st_dev),
        rustix::fs::minor(stat.st_dev),
        stat.st_ino
    );
    let inode = format!(":{}", stat.st_ino);
    let mut exact = Vec::new();
    let mut by_inode = Vec::new();
    for line in table.lines() {
        // `N: FLOCK  ADVISORY  WRITE 1234 08:02:131074 0 EOF`; blocked waiters carry `->`.
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 6 || fields[1] != "FLOCK" {
            continue;
        }
        let Ok(pid) = fields[4].parse::<u32>() else {
            continue;
        };
        let holder = (pid, fields[3] == "READ");
        if fields[5] == device {
            exact.push(holder);
        } else if fields[5].ends_with(&inode) {
            by_inode.push(holder);
        }
    }
    // Some filesystems (overlays) report a different device; the inode alone still identifies.
    if exact.is_empty() { by_inode } else { exact }
}

fn describe(pid: u32, role: &str) -> String {
    let proc = Path::new("/proc").join(pid.to_string());
    let Ok(cmdline) = fs::read(proc.join("cmdline")) else {
        return format!("pid {pid} ({role}; no longer visible)");
    };
    let mut command = cmdline
        .split(|byte| *byte == 0)
        .filter(|arg| !arg.is_empty())
        .map(String::from_utf8_lossy)
        .collect::<Vec<_>>()
        .join(" ");
    if command.chars().count() > 160 {
        command = command.chars().take(157).collect::<String>() + "...";
    }
    let mut out = format!("pid {pid} `{command}` ({role}");
    if let Ok(cwd) = fs::read_link(proc.join("cwd")) {
        out.push_str(&format!(" in {}", cwd.display()));
    }
    if let Some(started) = started_at(&proc) {
        out.push_str(&format!(", since {started}"));
    }
    if let Some(session) = session(&proc) {
        out.push_str(&format!(", session {session}"));
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
