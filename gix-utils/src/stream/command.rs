//! Capture both converter output streams in the same operation budget.
use super::{Budget, Spool, check, copy};
use std::{
    io::{self, Read, Seek},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Completed command output, positioned at the beginning and still charged as scratch.
pub struct CommandOutput {
    /// The process exit status.
    pub status: ExitStatus,
    /// Standard output, without a whole-output memory allocation.
    pub stdout: Spool,
    /// Standard error, charged together with standard output.
    pub stderr: Spool,
}

struct OwnedChild {
    child: Child,
    group_live: bool,
}
impl OwnedChild {
    fn stop(&mut self) {
        #[cfg(target_os = "linux")]
        if self.group_live
            && let Some(pid) = rustix::process::Pid::from_raw(self.child.id() as i32)
        {
            // The leader remains unreaped until this group has been retired, so
            // its PID cannot have been recycled into a foreign process group.
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
        self.group_live = false;
        let _ = self.child.kill();
    }
    fn poll(&mut self) -> io::Result<Option<ExitStatus>> {
        #[cfg(target_os = "linux")]
        {
            use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
            let pid = Pid::from_raw(self.child.id() as i32).expect("owned child PID");
            if waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            )?
            .is_none()
            {
                return Ok(None);
            }
            self.stop();
            self.child.wait().map(Some)
        }
        #[cfg(not(target_os = "linux"))]
        self.child.try_wait()
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        self.stop();
        let _ = self.child.wait();
    }
}

/// Run an owned converter with bounded stdout/stderr capture and interruption polling.
/// Linux descendants inherit a dedicated process group, retired before reaping its leader.
/// The caller configures stdin; no input is retained or implicitly piped here.
pub fn command_output(
    command: &mut Command,
    budget: &Arc<Budget>,
    interrupt: &AtomicBool,
) -> io::Result<CommandOutput> {
    check(interrupt)?;
    let stdout = budget.spool()?;
    let stderr = budget.spool()?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = OwnedChild {
        child: command.spawn()?,
        group_live: true,
    };
    let output = child.child.stdout.take().expect("piped stdout");
    let error = child.child.stderr.take().expect("piped stderr");
    let failed = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let capture = |mut input: Box<dyn Read + Send>, mut spool: Spool| {
            let result = copy(&mut input, &mut spool, interrupt);
            if result.is_err() {
                failed.store(true, Ordering::Release);
            }
            result?;
            spool.rewind()?;
            Ok::<_, io::Error>(spool)
        };
        let stdout = scope.spawn(move || capture(Box::new(output), stdout));
        let stderr = scope.spawn(move || capture(Box::new(error), stderr));
        let status = loop {
            if interrupt.load(Ordering::Relaxed) || failed.load(Ordering::Acquire) {
                child.stop();
            }
            match child.poll() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    child.stop();
                    break Err(error);
                }
            }
        };
        // A converter may exit while its inherited pipes are held by a descendant.
        child.stop();
        let stdout = stdout
            .join()
            .map_err(|_| io::Error::other("Converter stdout reader panicked"))?;
        let stderr = stderr
            .join()
            .map_err(|_| io::Error::other("Converter stderr reader panicked"))?;
        check(interrupt)?;
        Ok(CommandOutput {
            status: status?,
            stdout: stdout?,
            stderr: stderr?,
        })
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::stream::{SCRATCH_LIMIT, ScratchLimit};

    #[test]
    fn captures_both_streams_and_preserves_exit_status() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let budget = Budget::new(root.path())?;
        let mut result = command_output(
            Command::new("sh").args(["-c", "printf out; printf err >&2; exit 17"]),
            &budget,
            &AtomicBool::new(false),
        )?;
        assert_eq!(result.status.code(), Some(17));
        let mut out = String::new();
        let mut err = String::new();
        result.stdout.read_to_string(&mut out)?;
        result.stderr.read_to_string(&mut err)?;
        assert_eq!((out.as_str(), err.as_str()), ("out", "err"));
        assert_eq!(budget.usage().0, 6);
        drop(result);
        assert_eq!(budget.usage().0, 0);
        Ok(())
    }

    #[test]
    fn shared_quota_failure_kills_a_still_writing_converter() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let budget = Budget::new(root.path())?;
        let _retained = budget.charge(SCRATCH_LIMIT - 1024)?;
        let start = std::time::Instant::now();
        let error = command_output(
            Command::new("sh").args(["-c", "while :; do printf 'expanding-output'; done"]),
            &budget,
            &AtomicBool::new(false),
        )
        .err()
        .expect("output exceeds shared scratch quota");
        assert!(error.get_ref().is_some_and(|error| error.is::<ScratchLimit>()));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(budget.usage().0, SCRATCH_LIMIT - 1024);
        Ok(())
    }

    #[test]
    fn interruption_retires_owned_pipe_holders_within_one_second() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let budget = Budget::new(root.path())?;
        let interrupt = AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(50));
                interrupt.store(true, Ordering::Relaxed);
            });
            let start = std::time::Instant::now();
            assert!(command_output(Command::new("sh").args(["-c", "sleep 30 & wait"]), &budget, &interrupt).is_err());
            assert!(start.elapsed() < Duration::from_secs(1));
        });
        assert_eq!(budget.usage().0, 0);
        Ok(())
    }

    #[test]
    fn exited_converter_cannot_leave_descendants_holding_capture_pipes() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let budget = Budget::new(root.path())?;
        let start = std::time::Instant::now();
        let result = command_output(
            Command::new("sh").args(["-c", "sleep 30 & exit 0"]),
            &budget,
            &AtomicBool::new(false),
        )?;
        assert!(result.status.success());
        assert!(start.elapsed() < Duration::from_secs(1));
        Ok(())
    }
}
