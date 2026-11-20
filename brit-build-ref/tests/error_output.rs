//! A failed `brit-build-ref` ends its stderr with the error, never a stack frame, whatever
//! `RUST_BACKTRACE` says; a backtrace is printed only on `BRIT_BACKTRACE`, and before the error.

use std::process::{Command, Output};

fn signer_outside_a_repository(brit_backtrace: bool) -> Output {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"));
    gix_testtools::configure_git_environment(&mut command, dir.path());
    command
        .env("RUST_BACKTRACE", "1")
        .env("RUST_LIB_BACKTRACE", "1")
        .env_remove("BRIT_BACKTRACE");
    if brit_backtrace {
        command.env("BRIT_BACKTRACE", "1");
    }
    command
        .arg("--repo")
        .arg(dir.path())
        .arg("signer")
        .current_dir(dir.path())
        .output()
        .expect("brit-build-ref runs")
}

fn last_line(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .to_string()
}

#[test]
fn a_failure_ends_with_the_error_without_a_backtrace() {
    let output = signer_outside_a_repository(false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert_eq!(output.status.code(), Some(1), "the exit code stays anyhow's 1");
    assert!(stderr.starts_with("Error: "), "{stderr}");
    assert!(
        !stderr.contains("Stack backtrace"),
        "no backtrace under RUST_BACKTRACE: {stderr}"
    );
    let last = last_line(&output);
    assert!(
        !last.contains("<unknown>") && !last.contains("__libc") && !last.trim_start().starts_with("at "),
        "the last line is the error: {stderr}"
    );
}

#[test]
fn brit_backtrace_prints_it_before_the_error() {
    let with = signer_outside_a_repository(true);
    let stderr = String::from_utf8_lossy(&with.stderr);
    assert!(stderr.contains("Stack backtrace:"), "{stderr}");
    assert_eq!(
        last_line(&with),
        last_line(&signer_outside_a_repository(false)),
        "{stderr}"
    );
}

#[test]
fn a_closed_stderr_keeps_exit_code_one() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"));
    gix_testtools::configure_git_environment(&mut command, dir.path());
    let mut child = command
        .arg("--repo")
        .arg(dir.path())
        .arg("signer")
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("brit-build-ref runs");
    // Nobody reads stderr: the report hits a closed pipe and must not panic (exit 101).
    drop(child.stderr.take());
    assert_eq!(child.wait().expect("brit-build-ref exits").code(), Some(1));
}

#[test]
fn a_lookup_miss_still_exits_zero_with_stderr_closed() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    gix_testtools::git(dir.path(), "init").expect("git init");
    gix_testtools::git(dir.path(), "commit --allow-empty -m one").expect("git commit");
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit-build-ref"));
    gix_testtools::configure_git_environment(&mut command, dir.path());
    let mut child = command
        .arg("--repo")
        .arg(dir.path())
        .args(["build", "get", "--step", "never-built"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("brit-build-ref runs");
    // The miss line goes to a closed pipe: an answer that exits 0 must not become a panic (101).
    drop(child.stderr.take());
    assert_eq!(child.wait().expect("brit-build-ref exits").code(), Some(0));
}
