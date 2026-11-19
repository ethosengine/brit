//! `brit-verify` reports through the shared printer: its causes, last, and never a panic, even when
//! nobody reads its stderr.

use std::process::{Command, Stdio};

fn brit_verify(dir: &std::path::Path, args: &[&std::ffi::OsStr]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit-verify"));
    gix_testtools::configure_git_environment(&mut command, dir);
    command
        .args(args)
        .env("RUST_BACKTRACE", "1")
        .env("RUST_LIB_BACKTRACE", "1")
        .env_remove("BRIT_BACKTRACE")
        .current_dir(dir);
    command
}

fn repository() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    gix_testtools::git(dir.path(), "init").expect("git init");
    dir
}

#[test]
fn an_unresolvable_revision_reports_the_cause_chain() {
    let dir = repository();
    let output = brit_verify(
        dir.path(),
        &["no-such-revision".as_ref(), "--repo".as_ref(), dir.path().as_os_str()],
    )
    .output()
    .expect("brit-verify runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "repository errors keep exit 3: {stderr}");
    assert!(
        stderr.starts_with("Error: failed to resolve rev no-such-revision\n\nCaused by:\n"),
        "the context leads and the gix error follows as its cause: {stderr}"
    );
    assert!(!stderr.contains("Stack backtrace"), "{stderr}");
}

/// Run with stderr's reader already closed, so every report hits a closed pipe.
fn exit_code_with_stderr_closed(mut command: Command) -> Option<i32> {
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("brit-verify runs");
    drop(child.stderr.take());
    child.wait().expect("brit-verify exits").code()
}

#[test]
fn a_closed_stderr_keeps_the_documented_exit_codes() {
    let dir = repository();
    assert_eq!(
        exit_code_with_stderr_closed(brit_verify(dir.path(), &[])),
        Some(2),
        "a usage error still exits 2, not a panic's 101"
    );
    assert_eq!(
        exit_code_with_stderr_closed(brit_verify(
            dir.path(),
            &["no-such-revision".as_ref(), "--repo".as_ref(), dir.path().as_os_str()],
        )),
        Some(3),
        "a repository error still exits 3, not a panic's 101"
    );
}
