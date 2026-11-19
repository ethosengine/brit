//! A reader that closes early (`brit … | head`) ends Brit quietly with status 141, the status a
//! shell reports for Git killed by SIGPIPE, instead of `Error: Broken pipe (os error 32)`.
#![cfg(target_os = "linux")]

use std::{os::fd::FromRawFd, process::Command};

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

/// Run `brit <args>` in `cwd` with stdout connected to a pipe whose reading end is already closed.
fn brit_into_closed_pipe(cwd: &std::path::Path, args: &[&str]) -> Result<(Option<i32>, String)> {
    let mut fds = [0; 2];
    // SAFETY: `fds` has room for the two descriptors `pipe` writes.
    ensure!(unsafe { libc::pipe(fds.as_mut_ptr()) } == 0, "pipe(2) failed");
    // SAFETY: both descriptors were just created by `pipe` and are owned here exactly once.
    let (reader, writer) = unsafe { (std::fs::File::from_raw_fd(fds[0]), std::fs::File::from_raw_fd(fds[1])) };
    drop(reader);
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, cwd);
    let out = command.current_dir(cwd).args(args).stdout(writer).output()?;
    Ok((out.status.code(), String::from_utf8(out.stderr)?))
}

#[test]
fn closed_readers_end_brit_quietly() -> Result<()> {
    let repo = TestRepo::new("broken-pipe")?;
    for index in 0..50 {
        let made = gix_testtools::git_command(repo.path())
            .args(["branch", &format!("b{index}")])
            .status()?;
        ensure!(made.success(), "git branch b{index}");
    }
    for args in [&["branch", "list"][..], &["log"][..], &["--cli-surface-json"][..]] {
        let (code, stderr) = brit_into_closed_pipe(repo.path(), args)?;
        assert!(
            !stderr.contains("Broken pipe"),
            "brit {args:?} must not report the closed reader: {stderr}"
        );
        assert_eq!(code, Some(141), "brit {args:?}: {stderr}");
    }
    Ok(())
}
