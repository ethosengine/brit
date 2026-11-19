//! A reader that closes early (`brit … | head`) ends a command that only prints quietly with status 141,
//! the status a shell reports for Git killed by SIGPIPE, instead of `Error: Broken pipe (os error 32)`.
//! It is decided by the failed write's error kind on stdout or stderr, and only for commands classified
//! read-only; a command that changes the repository reports its own outcome (unit-tested in brit-cli).
#![cfg(target_os = "linux")]

use std::{
    io::{BufRead, BufReader, Write},
    os::fd::FromRawFd,
    process::{Command, Stdio},
};

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

/// The reader takes the first line and then closes, while Brit still has most of the listing to write.
#[test]
fn readers_closing_after_partial_output_end_read_only_listings_quietly() -> Result<()> {
    let repo = TestRepo::new("broken-pipe-partial")?;
    let head = String::from_utf8(
        gix_testtools::git_command(repo.path())
            .args(["rev-parse", "HEAD"])
            .output()?
            .stdout,
    )?;
    // Far more than a pipe buffers, so Brit is still writing when the reader goes away.
    let mut update = gix_testtools::git_command(repo.path())
        .args(["update-ref", "--stdin"])
        .stdin(Stdio::piped())
        .spawn()?;
    {
        let mut stdin = update.stdin.take().expect("piped stdin");
        for index in 0..4000 {
            writeln!(
                stdin,
                "create refs/heads/closed-reader/a-branch-name-long-enough-to-fill-the-pipe-{index:05} {}",
                head.trim()
            )?;
        }
    }
    ensure!(update.wait()?.success(), "git update-ref --stdin");

    for args in [&["branch", "list"][..], &["for-each-ref"][..]] {
        let mut command = Command::new(brit_bin().expect("built Brit"));
        gix_testtools::configure_git_environment(&mut command, repo.path());
        let mut child = command
            .current_dir(repo.path())
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut first = String::new();
        let mut reader = BufReader::new(child.stdout.take().expect("piped stdout"));
        reader.read_line(&mut first)?;
        drop(reader);
        let out = child.wait_with_output()?;
        let stderr = String::from_utf8(out.stderr)?;
        ensure!(!first.is_empty(), "brit {args:?} printed before the reader closed");
        assert_eq!(out.status.code(), Some(141), "brit {args:?}: {stderr}");
        // `branch list` clears its progress line on stderr (`ESC[2K` and a carriage return) whether or not the reader stays.
        let reported = stderr.replace("\x1b[2K", "").replace('\r', "");
        assert!(reported.is_empty(), "brit {args:?} ends quietly: {stderr:?}");
    }
    Ok(())
}
