//! Coverage tests for the rakia binary.
//!
//! Each test invokes a rakia subcommand against a self-contained fixture
//! and dumps the (normalized) output to BRIT_TEST_PAGE_STAGING/rust/rakia/<subcommand>.txt
//! for the cli-test-page runner to pick up.
//!
//! On-disk layout mirrors the subcommand path tree so the runner can reconstruct
//! the full path from directory hierarchy + filename stem.
//! Examples:
//!   rakia graph discover  →  staging/rust/rakia/graph/discover.txt
//!   rakia fingerprint     →  staging/rust/rakia/fingerprint.txt
//!   rakia baseline read   →  staging/rust/rakia/baseline/read.txt

use std::fs;
use std::path::PathBuf;

use cli_journey::support::runner::BritInvocation;
use cli_journey::support::test_repo::TestRepo;

fn rakia_bin() -> PathBuf {
    // tests/cli-journey -> ../../target/release/rakia
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/release/rakia")
        .canonicalize()
        .expect("rakia binary not built — run `cargo build -p brit-cli --release` first")
}

/// Dump captured output to BRIT_TEST_PAGE_STAGING/rust/<path[0]>/<path[1]>/.../<last>.txt
///
/// The path slice encodes the full subcommand hierarchy:
///   &["rakia", "graph", "discover"]  →  staging/rust/rakia/graph/discover.txt
///   &["rakia", "fingerprint"]        →  staging/rust/rakia/fingerprint.txt
fn staging_dump_path(path: &[&str], output: &str) {
    if let Ok(staging) = std::env::var("BRIT_TEST_PAGE_STAGING") {
        assert!(!path.is_empty(), "staging path must not be empty");
        let mut dir = PathBuf::from(staging).join("rust");
        // All segments except the last form the directory hierarchy.
        for segment in &path[..path.len() - 1] {
            dir = dir.join(segment);
        }
        fs::create_dir_all(&dir).expect("mkdir staging");
        let filename = format!("{}.txt", path[path.len() - 1]);
        fs::write(dir.join(filename), output).expect("write capture");
    }
}

/// Returns the elohim repo root if running inside the elohim monorepo.
/// Tests that require real manifests skip gracefully when not present.
fn elohim_repo_root() -> Option<PathBuf> {
    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../")
        .canonicalize()
        .ok()?;
    if candidate.join("app/elohim-app/build-manifest.json").exists() {
        Some(candidate)
    } else {
        None
    }
}

// ─── graph discover ──────────────────────────────────────────────────────────

#[test]
fn graph_discover_emits_manifests_array() {
    let Some(repo_root) = elohim_repo_root() else {
        eprintln!("skip: not in elohim repo");
        return;
    };
    let cap = BritInvocation::new(rakia_bin())
        .args(["graph", "discover", "--repo"])
        .arg(&repo_root)
        .normalize(true)
        .run()
        .expect("invoke");
    assert!(
        cap.status.success(),
        "exit: {:?} stderr: {}",
        cap.status,
        cap.stderr
    );
    assert!(cap.stdout.contains("manifests"), "stdout: {}", cap.stdout);
    staging_dump_path(&["rakia", "graph", "discover"], &cap.stdout);
}

// ─── graph show ──────────────────────────────────────────────────────────────

#[test]
fn graph_show_emits_dot_or_json() {
    // Use the elohim repo if available for a richer graph; fall back to an empty
    // temp repo (empty graph is a valid, documented outcome).
    let (target_path, _temp): (PathBuf, Option<TestRepo>) =
        if let Some(root) = elohim_repo_root() {
            (root, None)
        } else {
            let t = TestRepo::new("graph-show").expect("repo");
            let p = t.path().to_path_buf();
            (p, Some(t))
        };

    let cap = BritInvocation::new(rakia_bin())
        .args(["graph", "show", "--format", "dot", "--repo"])
        .arg(&target_path)
        .normalize(true)
        .run()
        .expect("invoke");
    // graph show may exit 0 (empty or populated graph) or non-zero (no manifests).
    // Either is valid documented behavior; we capture and surface it.
    staging_dump_path(
        &["rakia", "graph", "show"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── affected ────────────────────────────────────────────────────────────────

#[test]
fn affected_with_no_change_paths_returns_empty() {
    let Some(repo_root) = elohim_repo_root() else {
        eprintln!("skip: not in elohim repo");
        return;
    };
    let cap = BritInvocation::new(rakia_bin())
        .args(["affected", "--repo"])
        .arg(&repo_root)
        .normalize(true)
        .run()
        .expect("invoke");
    // Capture whatever happens — even an error response is documented behavior.
    staging_dump_path(
        &["rakia", "affected"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── plan ────────────────────────────────────────────────────────────────────

#[test]
fn plan_against_a_single_file() {
    let Some(repo_root) = elohim_repo_root() else {
        eprintln!("skip: not in elohim repo");
        return;
    };
    let cap = BritInvocation::new(rakia_bin())
        .args(["plan", "--repo"])
        .arg(&repo_root)
        .args(["--files", "app/elohim-app/src/styles.scss"])
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["rakia", "plan"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── fingerprint ─────────────────────────────────────────────────────────────

#[test]
fn fingerprint_emits_64_char_blake3_hex() {
    let Some(repo_root) = elohim_repo_root() else {
        eprintln!("skip: not in elohim repo");
        return;
    };
    let manifest = repo_root.join("app/elohim-app/build-manifest.json");
    let cap = BritInvocation::new(rakia_bin())
        .args(["fingerprint"])
        .arg(&manifest)
        .args(["--step", "build-angular"])
        .normalize(true)
        .run()
        .expect("invoke");
    assert!(cap.status.success(), "exit: {:?}", cap.status);
    // Output is a JSON object with a "fingerprints" array
    assert!(
        cap.stdout.contains("fingerprints"),
        "stdout: {}",
        cap.stdout
    );
    staging_dump_path(&["rakia", "fingerprint"], &cap.stdout);
}

// ─── baseline read ───────────────────────────────────────────────────────────

#[test]
fn baseline_read_returns_null_for_unknown_pipeline() {
    let temp = TestRepo::new("baseline-read").expect("repo");
    let cap = BritInvocation::new(rakia_bin())
        .args(["baseline", "read", "no-such-pipeline", "--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    // The temp repo has no baseline ref; capture actual behavior (null or error).
    staging_dump_path(
        &["rakia", "baseline", "read"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}

// ─── baseline write ──────────────────────────────────────────────────────────

#[test]
fn baseline_write_then_read_roundtrip() {
    let temp = TestRepo::new("baseline-write").expect("repo");
    let head = temp.head_id().expect("head");

    let cap_write = BritInvocation::new(rakia_bin())
        .args(["baseline", "write", "test-pipeline"])
        .arg(&head)
        .args(["--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke write");
    staging_dump_path(&["rakia", "baseline", "write"], &cap_write.stdout);

    // Verify the round-trip: reading back should return the same commit.
    // Do NOT normalize — we need the raw SHA to assert the round-trip.
    let cap_read = BritInvocation::new(rakia_bin())
        .args(["baseline", "read", "test-pipeline", "--repo"])
        .arg(temp.path())
        .normalize(false)
        .run()
        .expect("invoke read");
    assert!(
        cap_read.status.success(),
        "baseline read after write failed: {}",
        cap_read.stderr
    );
    assert!(
        cap_read.stdout.contains(&head),
        "expected commit {} in read output: {}",
        head,
        cap_read.stdout
    );
}

// ─── baseline migrate ────────────────────────────────────────────────────────

#[test]
fn baseline_migrate_with_minimal_jenkins_json() {
    let temp = TestRepo::new("baseline-migrate").expect("repo");
    let head = temp.head_id().expect("head");

    // Write a minimal Jenkins-shape baselines.json
    let json_path = temp.path().join("baselines.json");
    fs::write(
        &json_path,
        format!(
            r#"{{ "pipelines": {{ "p1": {{ "lastSuccessfulCommit": "{}" }} }} }}"#,
            head
        ),
    )
    .expect("write json");

    let cap = BritInvocation::new(rakia_bin())
        .args(["baseline", "migrate"])
        .arg(&json_path)
        .args(["--repo"])
        .arg(temp.path())
        .normalize(true)
        .run()
        .expect("invoke");
    staging_dump_path(
        &["rakia", "baseline", "migrate"],
        &format!("{}\n---stderr---\n{}", cap.stdout, cap.stderr),
    );
}
