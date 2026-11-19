//! `brit-verify` reports a repository failure with its causes, last, through the shared printer.

use std::process::Command;

#[test]
fn an_unresolvable_revision_reports_the_cause_chain() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .output()
        .expect("git runs");
    assert!(init.status.success());
    let output = Command::new(env!("CARGO_BIN_EXE_brit-verify"))
        .args(["no-such-revision", "--repo"])
        .arg(dir.path())
        .env("RUST_BACKTRACE", "1")
        .env("RUST_LIB_BACKTRACE", "1")
        .env_remove("BRIT_BACKTRACE")
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
