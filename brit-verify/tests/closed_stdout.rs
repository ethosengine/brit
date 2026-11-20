//! A reader that closes stdout early ends `brit-verify` quietly with Git's `SIGPIPE` status, never with a panic.

use std::process::{Command, Stdio};

#[allow(
    clippy::incompatible_msrv,
    reason = "test-only: `std::io::pipe` needs Rust 1.87 and the tests run on the stable toolchain"
)]
#[test]
fn a_closed_stdout_ends_quietly_with_the_sigpipe_status() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    gix_testtools::git(dir.path(), "init").expect("git init");
    let mut commit = Command::new("git");
    gix_testtools::configure_git_environment(&mut commit, dir.path());
    let status = commit
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "subject\n\nLamad: a\nShefa: b\nQahal: c\n",
        ])
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .current_dir(dir.path())
        .status()
        .expect("git commit runs");
    assert!(status.success(), "the fixture commit is made");

    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit-verify"));
    gix_testtools::configure_git_environment(&mut command, dir.path());
    let output = command
        .args(["HEAD", "--repo"])
        .arg(dir.path())
        .current_dir(dir.path())
        .stdout(Stdio::from(writer))
        .output()
        .expect("brit-verify runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(141),
        "Git's SIGPIPE status, not a panic's 101: {stderr}"
    );
    assert!(stderr.trim().is_empty(), "ends quietly, no panic text: {stderr}");
}
