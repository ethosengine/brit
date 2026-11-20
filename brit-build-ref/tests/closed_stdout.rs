//! A reader that closes stdout early ends `brit-build-ref` quietly with Git's `SIGPIPE` status (read-only commands) or
//! success (a command that had already done its work), never with a panic.

use std::process::{Command, Stdio};

fn repository() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    gix_testtools::git(dir.path(), "init").expect("git init");
    dir
}

/// `brit-build-ref <args>` in `dir`, with stdout connected to a pipe whose reader is already closed.
#[allow(
    clippy::incompatible_msrv,
    reason = "test-only: `std::io::pipe` needs Rust 1.87 and the tests run on the stable toolchain"
)]
fn run_with_stdout_closed(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"));
    gix_testtools::configure_git_environment(&mut command, dir);
    command
        .arg("--repo")
        .arg(dir)
        .args(args)
        .env_remove("BRIT_BACKTRACE")
        .current_dir(dir)
        .stdout(Stdio::from(writer));
    command.output().expect("brit-build-ref runs")
}

#[test]
fn a_read_only_command_ends_with_the_sigpipe_status_and_no_panic() {
    let dir = repository();
    for args in [&["signer"][..], &["signer", "--json"]] {
        let output = run_with_stdout_closed(dir.path(), args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(141),
            "{args:?}: Git's SIGPIPE status, not a panic's 101: {stderr}"
        );
        assert!(
            stderr.trim().is_empty(),
            "{args:?}: ends quietly, no panic text: {stderr}"
        );
    }
}
