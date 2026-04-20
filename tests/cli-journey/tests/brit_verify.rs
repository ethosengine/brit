//! Coverage tests for the brit-verify binary.
//!
//! brit-verify is a single-purpose binary (not subcommand-tree); takes a
//! REV positional argument and verifies pillar trailers on commits at that rev.
//!
//! Usage: brit-verify <commit-rev> [--repo <path>]
//! Exit codes: 0 = valid, 1 = validation failure, 2 = usage error, 3 = repo error.
//!
//! Staging layout: a single entry for the binary itself (no subcommands):
//!   BRIT_TEST_PAGE_STAGING/rust/brit-verify.txt

use std::fs;
use std::path::PathBuf;

use cli_journey::support::runner::BritInvocation;
use cli_journey::support::test_repo::TestRepo;

fn brit_verify_bin() -> PathBuf {
    // tests/cli-journey -> ../../target/release/brit-verify
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/release/brit-verify")
        .canonicalize()
        .expect("brit-verify binary not built — run `cargo build -p brit-verify --release` first")
}

/// Dump captured output to BRIT_TEST_PAGE_STAGING/rust/brit-verify.txt.
///
/// brit-verify is a single-leaf binary with no subcommands, so we use a
/// flat staging path rather than a subdirectory hierarchy.
fn staging_dump(output: &str) {
    if let Ok(staging) = std::env::var("BRIT_TEST_PAGE_STAGING") {
        let dir = PathBuf::from(&staging).join("rust");
        fs::create_dir_all(&dir).expect("mkdir staging/rust");
        fs::write(dir.join("brit-verify.txt"), output).expect("write capture");
    }
}

// ─── no args ─────────────────────────────────────────────────────────────────

#[test]
fn brit_verify_with_no_args_exits_2_and_prints_usage() {
    let cap = BritInvocation::new(brit_verify_bin())
        .normalize(true)
        .run()
        .expect("invoke");
    assert_eq!(
        cap.status.code(),
        Some(2),
        "expected exit 2 (usage error), got {:?}; stderr: {}",
        cap.status.code(),
        cap.stderr
    );
    assert!(
        cap.stderr.contains("missing <commit-rev>") || cap.stderr.contains("Usage:"),
        "expected usage message in stderr: {}",
        cap.stderr
    );
}

// ─── missing trailers (real repo HEAD) ───────────────────────────────────────

#[test]
fn brit_verify_commit_without_trailers_exits_1() {
    // A fresh temp repo has a bare "init" commit with no pillar trailers.
    let temp = TestRepo::new("verify-no-trailers").expect("repo");
    let head = temp.head_id().expect("head");

    let cap = BritInvocation::new(brit_verify_bin())
        .arg(&head)
        .args(["--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    assert_eq!(
        cap.status.code(),
        Some(1),
        "expected exit 1 (validation failure), got {:?}; stderr: {}",
        cap.status.code(),
        cap.stderr
    );
    assert!(
        cap.stderr.contains("pillar validation failed"),
        "expected validation failure message in stderr: {}",
        cap.stderr
    );
    // Capture validation-failure output as the primary staging artifact.
    staging_dump(&format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr));
}

// ─── valid trailers ───────────────────────────────────────────────────────────

#[test]
fn brit_verify_commit_with_valid_trailers_exits_0() {
    // Create a commit whose message contains all three required pillar trailers.
    let temp = TestRepo::new("verify-valid-trailers").expect("repo");

    // Commit with pillar trailers in the message body.
    let msg = "feat: add something\n\nLamad: learning-path\nShefa: economy-unit\nQahal: governance-node";
    let _sha = temp
        .commit_file_with_message("sentinel.txt", "x", msg)
        .expect("commit with trailers");
    let head = temp.head_id().expect("head");

    let cap = BritInvocation::new(brit_verify_bin())
        .arg(&head)
        .args(["--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    assert_eq!(
        cap.status.code(),
        Some(0),
        "expected exit 0 (valid), got {:?}; stderr: {}",
        cap.status.code(),
        cap.stderr
    );
    assert!(
        cap.stdout.contains("pillar trailers valid"),
        "expected success message in stdout: {}",
        cap.stdout
    );
    // Overwrite with the more interesting passing output as the staging artifact.
    staging_dump(&format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr));
}
