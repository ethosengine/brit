//! A reader that closes early (`brit … | head`) ends a command that only prints quietly with status 141,
//! the status a shell reports for Git killed by SIGPIPE, instead of `Error: Broken pipe (os error 32)`.
//! It is decided by the failed write's error kind on stdout or stderr, only for commands classified
//! read-only, and only when the command did not fail for its own reason. This never gives a mutating
//! command 141; how a mutating command orders its own writes (like `push --porcelain`'s report before
//! its rejection) is outside this change.
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

/// Run `brit <args>` in `cwd` with stdout (`which == 1`) or stderr (`which == 2`) connected to a pipe
/// whose reading end is already closed; the other stream is captured.
fn brit_with_closed(cwd: &std::path::Path, which: u8, args: &[&str]) -> Result<std::process::Output> {
    let mut fds = [0; 2];
    // SAFETY: `fds` has room for the two descriptors `pipe` writes.
    ensure!(unsafe { libc::pipe(fds.as_mut_ptr()) } == 0, "pipe(2) failed");
    // SAFETY: both descriptors were just created by `pipe` and are owned here exactly once.
    let (reader, writer) = unsafe { (std::fs::File::from_raw_fd(fds[0]), std::fs::File::from_raw_fd(fds[1])) };
    drop(reader);
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, cwd);
    command.current_dir(cwd).args(args);
    if which == 1 {
        command.stdout(writer);
    } else {
        command.stderr(writer);
    }
    Ok(command.output()?)
}

/// A command that fails keeps its own status even when its reader is gone, where Git, killed by SIGPIPE
/// at its write, would report 141. (A failure at the write itself, like `rev-parse HEAD missing` with stdout
/// closed before the object name is written, is the closed reader and ends with 141.)
#[test]
fn a_closed_reader_never_replaces_a_genuine_failure() -> Result<()> {
    let repo = TestRepo::new("broken-pipe-failure")?;
    for (which, args, code) in [
        (2, &["rev-parse", "--verify", "missing"][..], 128),
        (1, &["-v", "rev-parse", "HEAD", "missing"][..], 128),
        // Usage errors write to a closed stderr without panicking.
        (2, &["check-ref-format"][..], 129),
        (2, &["merge-base", "--is-ancestor", "HEAD"][..], 129),
        (2, &["merge-base", "--is-ancestor", "nope", "HEAD"][..], 128),
        (2, &["rev-list", "--count"][..], 129),
        (2, &["rev-list", "--count", "nope"][..], 128),
        (2, &["-C", "does-not-exist", "rev-parse", "HEAD"][..], 128),
    ] {
        let out = brit_with_closed(repo.path(), which, args)?;
        assert_eq!(
            out.status.code(),
            Some(code),
            "brit {args:?} with fd {which} closed: {out:?}"
        );
    }
    Ok(())
}
