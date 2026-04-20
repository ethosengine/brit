//! Coverage tests for the brit-build-ref binary.
//!
//! Manages build/deploy/validate/reach attestation refs. Each leaf
//! subcommand gets its own test that exercises the operation against
//! a temp git repo and dumps captured output to staging.
//!
//! Subcommand tree (11 leaves):
//!   build   → put, get, list
//!   deploy  → put, get, list
//!   validate → put, get, list
//!   reach   → compute, get
//!
//! Staging layout (per leaf):
//!   BRIT_TEST_PAGE_STAGING/rust/brit-build-ref/<group>/<leaf>.txt

use std::fs;
use std::path::PathBuf;

use cli_journey::support::runner::BritInvocation;
use cli_journey::support::test_repo::TestRepo;

fn brit_build_ref_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/release/brit-build-ref")
        .canonicalize()
        .expect("brit-build-ref binary not built — run `cargo build -p brit-build-ref --release` first")
}

/// Dump captured output to staging at a nested path.
///
/// `path` is a slice of path segments relative to `BRIT_TEST_PAGE_STAGING/rust/`.
/// Example: `staging_dump_path(&["brit-build-ref", "build", "list"], output)`
/// writes to `staging/rust/brit-build-ref/build/list.txt`.
fn staging_dump_path(path: &[&str], output: &str) {
    if let Ok(staging) = std::env::var("BRIT_TEST_PAGE_STAGING") {
        let mut dir = PathBuf::from(staging).join("rust");
        // All segments except the last form the directory path.
        for segment in &path[..path.len() - 1] {
            dir = dir.join(segment);
        }
        fs::create_dir_all(&dir).expect("mkdir staging");
        let filename = format!("{}.txt", path[path.len() - 1]);
        fs::write(dir.join(filename), output).expect("write capture");
    }
}

// ============================================================================
// build subcommands
// ============================================================================

#[test]
fn build_list_in_empty_repo() {
    let temp = TestRepo::new("build-list").expect("repo");
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["build", "list", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit-build-ref", "build", "list"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn build_get_for_unknown_step() {
    let temp = TestRepo::new("build-get").expect("repo");
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["build", "get", "--step", "no-such-step", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit-build-ref", "build", "get"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn build_put_help_only() {
    // build put requires --step, --manifest, --output, --inputs-hash; capture
    // --help to document the subcommand exists without needing valid CID fixtures.
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["build", "put", "--help"])
        .normalize(true)
        .run()
        .expect("invoke");
    assert!(
        cap.status.success(),
        "expected exit 0 for --help, got {:?}; stderr: {}",
        cap.status.code(),
        cap.stderr
    );
    staging_dump_path(
        &["brit-build-ref", "build", "put"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ============================================================================
// deploy subcommands
// ============================================================================

#[test]
fn deploy_list_in_empty_repo() {
    let temp = TestRepo::new("deploy-list").expect("repo");
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["deploy", "list", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit-build-ref", "deploy", "list"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn deploy_get_for_unknown_target() {
    let temp = TestRepo::new("deploy-get").expect("repo");
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["deploy", "get", "--step", "no-such-step", "--env", "prod", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit-build-ref", "deploy", "get"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn deploy_put_help_only() {
    // deploy put requires --step, --env, --artifact, --endpoint, --health-check-epr;
    // capture --help to document the subcommand surface without complex fixtures.
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["deploy", "put", "--help"])
        .normalize(true)
        .run()
        .expect("invoke");
    assert!(
        cap.status.success(),
        "expected exit 0 for --help, got {:?}; stderr: {}",
        cap.status.code(),
        cap.stderr
    );
    staging_dump_path(
        &["brit-build-ref", "deploy", "put"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ============================================================================
// validate subcommands
// ============================================================================

#[test]
fn validate_list_in_empty_repo() {
    let temp = TestRepo::new("validate-list").expect("repo");
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["validate", "list", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit-build-ref", "validate", "list"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn validate_get_for_unknown_step() {
    let temp = TestRepo::new("validate-get").expect("repo");
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["validate", "get", "--step", "no-such-step", "--check", "lint@v1", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit-build-ref", "validate", "get"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn validate_put_help_only() {
    // validate put requires --step, --check, --artifact, --result;
    // capture --help to document the subcommand surface without valid CID fixtures.
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["validate", "put", "--help"])
        .normalize(true)
        .run()
        .expect("invoke");
    assert!(
        cap.status.success(),
        "expected exit 0 for --help, got {:?}; stderr: {}",
        cap.status.code(),
        cap.stderr
    );
    staging_dump_path(
        &["brit-build-ref", "validate", "put"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ============================================================================
// reach subcommands
// ============================================================================

#[test]
fn reach_compute_for_unknown_step() {
    // reach compute --step is required; invoke against a temp repo where the step
    // doesn't exist to observe the error path and document the exit behaviour.
    let temp = TestRepo::new("reach-compute").expect("repo");
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["reach", "compute", "--step", "no-such-step", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit-build-ref", "reach", "compute"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

#[test]
fn reach_get_for_unknown_step() {
    let temp = TestRepo::new("reach-get").expect("repo");
    let cap = BritInvocation::new(brit_build_ref_bin())
        .args(["reach", "get", "--step", "no-such-step", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["brit-build-ref", "reach", "get"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}
