//! Linux hook launch barrier and durable outcome receipts for internal authoring.
//! The caller is the supervisor, outside the child's process group. Its flock is
//! held until the group is quiescent and an exit receipt has been synchronized.
//! Detached/daemonized writers are unsupported, as are hostile external writers.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
};

/// Unknown outcomes are never implicitly retried.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationOutcome {
    Succeeded,
    Failed,
    Unknown,
}
impl InvocationOutcome {
    /// Whether execution durably completed successfully.
    pub fn success(self) -> bool {
        self == Self::Succeeded
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    pid: u32,
    start: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    version: u32,
    boot: String,
    supervisor: Identity,
    directory: [u64; 2],
    lease: [u64; 2],
    invocation: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Vec<u8>,
    checksum: String,
}

fn digest(bytes: &[u8]) -> Result<String> {
    let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
    hash.update(bytes);
    Ok(hash.try_finalize()?.to_string())
}
fn sync_dir(path: &Path) -> Result<()> {
    super::super::switch::sync_dir(path)
}
fn write<T: Serialize>(dir: &Path, name: &str, value: &T) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    let bytes = serde_json::to_vec(&Envelope {
        checksum: digest(&payload)?,
        payload,
    })?;
    let temp = dir.join(format!("{name}.pending"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(&bytes)?;
    super::super::switch::sync_file(&file, &temp)?;
    fs::rename(temp, dir.join(name))?;
    sync_dir(dir)
}
fn read<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> Result<T> {
    let path = dir.join(name);
    let metadata = fs::symlink_metadata(&path)?;
    ensure!(
        metadata.is_file()
            && metadata.permissions().mode().trailing_zeros() >= 6
            && metadata.uid() == fs::metadata(dir)?.uid(),
        "unsafe invocation receipt"
    );
    let envelope: Envelope = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        digest(&envelope.payload)? == envelope.checksum,
        "invocation receipt checksum mismatch"
    );
    Ok(serde_json::from_slice(&envelope.payload)?)
}
fn stat(pid: u32) -> Result<Option<(String, u32, String)>> {
    let text = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(text) => text,
        // A process can disappear between opening and reading its proc entry.
        // Linux reports ESRCH for that race, instead of ENOENT from open.
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                || e.raw_os_error() == Some(nix::errno::Errno::ESRCH as i32) =>
        {
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    };
    let fields: Vec<_> = text
        .rsplit_once(')')
        .context("malformed process identity")?
        .1
        .split_whitespace()
        .collect();
    ensure!(fields.len() > 19, "incomplete process identity");
    Ok(Some((fields[19].into(), fields[2].parse()?, fields[0].into())))
}
fn identity(pid: u32) -> Result<Identity> {
    Ok(Identity {
        pid,
        start: stat(pid)?.context("process disappeared before launch receipt")?.0,
    })
}
fn group_quiescent(child: &Identity) -> Result<()> {
    // A reused PID/group is foreign, even if it is now a different executable.
    if let Some((start, _, state)) = stat(child.pid)? {
        ensure!(start == child.start, "child PID was reused; quiescence is ambiguous");
        ensure!(
            state == "Z" || state == "X",
            "hook child still live; waiting, no recovery mutation allowed"
        );
    }
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        if let Some((_, group, state)) = stat(pid)? {
            ensure!(
                group != child.pid || state == "Z" || state == "X",
                "hook process group {} is still live; no recovery mutation allowed",
                child.pid
            );
        }
    }
    Ok(())
}
fn file_identity(metadata: &fs::Metadata) -> [u64; 2] {
    [metadata.dev(), metadata.ino()]
}

fn lease(dir: &Path, create: bool) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    if create {
        options.create_new(true);
    }
    let file = options.open(dir.join("lease"))?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.permissions().mode().trailing_zeros() >= 6
            && metadata.uid() == fs::metadata(dir)?.uid(),
        "unsafe invocation lease"
    );
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .context("invocation supervisor is live; no recovery mutation allowed")?;
    Ok(file)
}

fn checkpoint(dir: &Path, point: &str) -> Result<()> {
    #[cfg(feature = "switch-test-faults")]
    if std::env::var("BRIT_AUTHORING_INVOCATION_PAUSE").as_deref() == Ok(point) {
        write(dir, "checkpoint", &point)?;
        loop {
            ensure!(
                !gix::interrupt::is_triggered(),
                "invocation interrupted; explicit recovery required"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    let _ = (dir, point);
    Ok(())
}

/// Owns the launch lease from durable preparation through terminal receipt.
pub struct PreparedInvocation {
    directory: PathBuf,
    _lease: fs::File,
}

/// Prepare durable identity BEFORE the session may record a pending invocation.
/// An interruption before this returns cannot have executed a hook.
pub fn prepare(dir: &Path) -> Result<PreparedInvocation> {
    fs::DirBuilder::new().mode(0o700).create(dir)?;
    sync_dir(dir.parent().context("invocation has no parent")?)?;
    let _lease = lease(dir, true)?;
    let start = Start {
        version: 1,
        boot: fs::read_to_string("/proc/sys/kernel/random/boot_id")?,
        supervisor: identity(std::process::id())?,
        directory: file_identity(&fs::symlink_metadata(dir)?),
        lease: file_identity(&_lease.metadata()?),
        invocation: dir
            .file_name()
            .context("missing invocation ID")?
            .as_encoded_bytes()
            .to_vec(),
    };
    write(dir, "start.json", &start)?;
    Ok(PreparedInvocation {
        directory: dir.to_owned(),
        _lease,
    })
}

/// Execute only a prepared, leased invocation, releasing the child after its
/// durable PID/start/group receipt. No path can reopen and rerun a prepared ID.
pub(super) fn run(command: Command, input: Option<&[u8]>, prepared: PreparedInvocation) -> Result<InvocationOutcome> {
    run_owned(command, input, prepared, false)
}

/// Editors use the same launch barrier, lease and quiescence receipts as hooks.
pub(crate) fn run_editor(command: Command, prepared: PreparedInvocation, terminal: bool) -> Result<InvocationOutcome> {
    run_owned(command, None, prepared, terminal)
}

struct Terminal {
    file: fs::File,
    original: rustix::process::Pid,
    restored: bool,
}
impl Terminal {
    fn handoff(pid: u32) -> Result<Self> {
        let file = fs::OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let original = rustix::termios::tcgetpgrp(&file)?;
        ensure!(
            original == rustix::process::getpgrp(),
            "editor supervisor is not in the foreground"
        );
        Self::set(
            &file,
            rustix::process::Pid::from_raw(i32::try_from(pid)?).context("invalid editor PID")?,
        )?;
        Ok(Self {
            file,
            original,
            restored: false,
        })
    }
    fn set(file: &fs::File, group: rustix::process::Pid) -> Result<()> {
        use nix::sys::signal::{SigSet, SigmaskHow, Signal};
        let mut blocked = SigSet::empty();
        blocked.add(Signal::SIGTTOU);
        let previous = blocked.thread_swap_mask(SigmaskHow::SIG_BLOCK)?;
        let result = rustix::termios::tcsetpgrp(file, group);
        previous.thread_set_mask()?;
        Ok(result?)
    }
    fn restore(&mut self) -> Result<()> {
        Self::set(&self.file, self.original)?;
        self.restored = true;
        Ok(())
    }
    fn wait(
        &mut self,
        child: &mut std::process::Child,
        identity: &Identity,
        interrupt: &mut Interrupt,
    ) -> Result<std::process::ExitStatus> {
        use nix::{
            sys::{
                signal::{Signal, killpg, raise},
                wait::{WaitPidFlag, WaitStatus, waitpid},
            },
            unistd::Pid,
        };
        use std::os::unix::process::ExitStatusExt;
        let pid = Pid::from_raw(i32::try_from(child.id())?);
        loop {
            interrupt.poll(identity)?;
            match waitpid(pid, Some(WaitPidFlag::WUNTRACED | WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(_, code)) => return Ok(std::process::ExitStatus::from_raw(code << 8)),
                Ok(WaitStatus::Signaled(_, signal, core)) => {
                    return Ok(std::process::ExitStatus::from_raw(
                        signal as i32 | if core { 128 } else { 0 },
                    ));
                }
                Ok(WaitStatus::Stopped(_, _)) => {
                    // Restore the caller's terminal and suspend this supervisor
                    // so the shell can implement Ctrl-Z/fg normally. The lease
                    // remains held across suspension; no recovery may race it.
                    self.restore()?;
                    // Stop the calling thread synchronously. A process-directed
                    // kill may reach another thread while this one keeps running
                    // and wrongly resumes the editor before the group stops.
                    raise(Signal::SIGSTOP)?;
                    while rustix::termios::tcgetpgrp(&self.file)? != self.original {
                        // `bg` must not steal the terminal from the shell.
                        raise(Signal::SIGSTOP)?;
                    }
                    Self::set(
                        &self.file,
                        rustix::process::Pid::from_raw(pid.as_raw()).context("editor PID")?,
                    )?;
                    self.restored = false;
                    killpg(pid, Signal::SIGCONT)?;
                }
                Err(nix::errno::Errno::EINTR) => continue,
                Err(error) => return Err(error.into()),
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        if !self.restored {
            let _ = self.restore();
        }
    }
}

/// Forward an explicit supervisor interrupt only to its still-owned group.
/// An uncooperative group remains UNKNOWN and blocks subsequent recovery.
#[derive(Default)]
struct Interrupt {
    sent: Option<std::time::Instant>,
}
impl Interrupt {
    fn poll(&mut self, child: &Identity) -> Result<()> {
        if gix::interrupt::is_triggered() && self.sent.is_none() {
            if let Some((start, group, _)) = stat(child.pid)? {
                ensure!(
                    start == child.start && group == child.pid,
                    "invocation process ownership changed"
                );
                match nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(i32::try_from(child.pid)?),
                    nix::sys::signal::Signal::SIGINT,
                ) {
                    Ok(()) | Err(nix::errno::Errno::ESRCH) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            self.sent = Some(std::time::Instant::now());
        }
        ensure!(
            self.sent
                .is_none_or(|sent| sent.elapsed() < std::time::Duration::from_secs(5)),
            "invocation interrupted; outcome UNKNOWN; owned group may still be live; stop it explicitly before recovery"
        );
        Ok(())
    }
}

fn run_owned(
    mut command: Command,
    input: Option<&[u8]>,
    prepared: PreparedInvocation,
    interactive_terminal: bool,
) -> Result<InvocationOutcome> {
    let dir = prepared.directory.as_path();
    checkpoint(dir, "after-start")?;
    command.process_group(0);
    let mut child = command.spawn().context("spawn gated hook")?;
    // On every error before release, dropping stdin closes the barrier. The
    // child cannot execute merely because its PID was allocated.
    let child_identity = match identity(child.id()).and_then(|id| {
        write(dir, "launch.json", &id)?;
        Ok(id)
    }) {
        Ok(id) => id,
        Err(error) => {
            drop(child.stdin.take());
            let _ = child.wait();
            return Err(error);
        }
    };
    checkpoint(dir, "after-launch")?;
    let mut terminal = if interactive_terminal {
        Some(Terminal::handoff(child.id())?)
    } else {
        None
    };
    let mut stdin = child.stdin.take().context("gated hook requires piped stdin")?;
    let mut interrupt = Interrupt::default();
    let flags = rustix::fs::fcntl_getfl(&stdin)?;
    rustix::fs::fcntl_setfl(&stdin, flags | rustix::fs::OFlags::NONBLOCK)?;
    let input_result = (|| -> std::io::Result<()> {
        for mut bytes in [b"go\n".as_slice(), input.unwrap_or_default()] {
            while !bytes.is_empty() {
                if gix::interrupt::is_triggered() {
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                match stdin.write(bytes) {
                    Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
                    Ok(written) => bytes = &bytes[written..],
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    })();
    drop(stdin);
    // Even a test checkpoint interruption must pass through owned-group
    // forwarding and quiescence before the supervisor releases its lease.
    let checkpoint_result = checkpoint(dir, "after-release");
    let status = match &mut terminal {
        Some(terminal) => terminal.wait(&mut child, &child_identity, &mut interrupt)?,
        None => loop {
            interrupt.poll(&child_identity)?;
            if let Some(status) = child.try_wait().context("wait for hook child")? {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        },
    };
    if let Some(terminal) = &mut terminal {
        terminal.restore()?;
    }
    if interrupt.sent.is_some() {
        while group_quiescent(&child_identity).is_err() {
            interrupt.poll(&child_identity)?;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    } else {
        group_quiescent(&child_identity)?;
    }
    checkpoint_result?;
    ensure!(
        interrupt.sent.is_none(),
        "invocation interrupted; outcome UNKNOWN; explicit recovery required"
    );
    checkpoint(dir, "after-wait")?;
    let outcome = if status.success()
        && input_result
            .as_ref()
            .err()
            .is_none_or(|e| e.kind() == std::io::ErrorKind::BrokenPipe)
    {
        InvocationOutcome::Succeeded
    } else {
        InvocationOutcome::Failed
    };
    write(dir, "exit.json", &outcome)?;
    Ok(outcome)
}

/// Acquire the invocation lease and prove quiescence before reading its outcome.
/// Missing exit evidence remains UNKNOWN; this function never invokes anything.
pub fn inspect(dir: &Path) -> Result<InvocationOutcome> {
    let metadata = fs::symlink_metadata(dir)?;
    ensure!(
        metadata.is_dir() && metadata.permissions().mode().trailing_zeros() >= 6,
        "unsafe invocation directory"
    );
    let _lease = lease(dir, false)?;
    let start: Start = read(dir, "start.json")?;
    ensure!(
        start.directory == file_identity(&metadata)
            && start.lease == file_identity(&_lease.metadata()?)
            && start.invocation == dir.file_name().context("missing invocation ID")?.as_encoded_bytes(),
        "invocation ownership changed"
    );
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let member = fs::symlink_metadata(entry.path())?;
        ensure!(
            member.is_file() && member.permissions().mode().trailing_zeros() >= 6 && member.uid() == metadata.uid(),
            "unsafe invocation member"
        );
        let name = entry.file_name();
        ensure!(
            [
                "lease",
                "checkpoint",
                "start.json",
                "launch.json",
                "exit.json",
                "start.json.pending",
                "launch.json.pending",
                "exit.json.pending"
            ]
            .iter()
            .any(|expected| name == *expected),
            "unexpected invocation member"
        );
    }
    let exit_present = match fs::symlink_metadata(dir.join("exit.json")) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };
    ensure!(start.version == 1, "unsupported invocation version");
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    if start.boot == boot {
        if !exit_present && let Some((identity, _, state)) = stat(start.supervisor.pid)? {
            ensure!(
                identity != start.supervisor.start || state == "Z" || state == "X",
                "invocation supervisor still live; unknown outcome requires quiescence"
            );
        }
        match fs::symlink_metadata(dir.join("launch.json")) {
            Ok(_) => group_quiescent(&read::<Identity>(dir, "launch.json")?)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ensure!(!exit_present, "exit receipt without launch identity");
                // With no durable launch receipt, the barrier was never opened.
                // Refuse while the original supervisor identity could still act.
                if let Some((identity, _, state)) = stat(start.supervisor.pid)? {
                    ensure!(
                        identity != start.supervisor.start || state == "Z" || state == "X",
                        "launch supervisor still live; outcome unknown"
                    );
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    match fs::symlink_metadata(dir.join("exit.json")) {
        Ok(_) => read(dir, "exit.json"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(InvocationOutcome::Unknown),
        Err(e) => Err(e.into()),
    }
}
