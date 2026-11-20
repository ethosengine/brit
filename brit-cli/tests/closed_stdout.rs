//! A reader that closes stdout early ends a read-only command quietly with Git's `SIGPIPE` status, never with a
//! panic report or Rust's 101.

use std::process::{Command, Output, Stdio};

/// Run `command` with stdout connected to a pipe whose reader is already closed, so the first write fails with `EPIPE`.
#[allow(
    clippy::incompatible_msrv,
    reason = "test-only: `std::io::pipe` needs Rust 1.87 and the tests run on the stable toolchain"
)]
fn run_with_stdout_closed(mut command: Command) -> Output {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    command.stdout(Stdio::from(writer));
    command.output().expect("runs")
}

fn assert_quiet_sigpipe_status(name: &str, output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(141),
        "{name}: Git's SIGPIPE status, not a panic's 101: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "{name}: no panic text: {stderr}");
    assert!(stderr.trim().is_empty(), "{name}: ends quietly: {stderr}");
}

#[test]
fn brit_ends_quietly_when_its_reader_closed_stdout() {
    let dir = std::env::temp_dir();
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, dir.as_path());
    command.arg("--cli-surface-json").current_dir(dir.as_path());
    assert_quiet_sigpipe_status("brit", &run_with_stdout_closed(command));
}

#[test]
fn brit_rebase_retained_ends_quietly_when_its_reader_closed_stdout() {
    let dir = std::env::temp_dir();
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit-rebase-retained"));
    command.arg("--help").current_dir(dir.as_path());
    assert_quiet_sigpipe_status("brit-rebase-retained", &run_with_stdout_closed(command));
}
