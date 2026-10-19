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

use std::{fs, path::PathBuf};

use cli_journey::support::{
    runner::{auxiliary_bin, BritInvocation},
    test_repo::TestRepo,
};

fn brit_build_ref_bin() -> Option<PathBuf> {
    auxiliary_bin("BRIT_BUILD_REF_BIN", "brit-build-ref")
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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

#[test]
fn build_find_by_inputs_hash_hits_across_two_commits_with_identical_closure() {
    // F1 of the evidence-by-identity plan: a gate asks "has THIS input closure been built?",
    // and the answer must not depend on which commit carried it. Two commits whose step
    // closure is byte-identical share one attestation lookup; a different closure finds none.
    let temp = TestRepo::new("build-find").expect("repo");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let first = temp
        .commit_file("src/lib.rs", "pub fn one() {}\n")
        .expect("first commit");
    let second = temp
        .commit_file("docs/README.md", "docs only — the step's closure is unchanged\n")
        .expect("second commit");
    assert_ne!(first, second);

    let manifest = "bafyreig7os3vfj4z63vgy7lpfgcnzzr2z26vrvdtheapoegvgzhnnhxkr4";
    let output = "bafyreieicbex3dkkgounj4latxya32ezpl6622lpij327mzndwptfqkf7q";
    let put = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args([
            "build",
            "put",
            "--step",
            "elohim-edge:cargo-build-storage",
            "--manifest",
            manifest,
            "--output",
            output,
            "--inputs-hash",
            "closure-a",
            "--duration-ms",
            "1875000",
            "--commit",
        ])
        .arg(&first)
        .run()
        .expect("invoke put");
    assert!(put.status.success(), "put failed: {}", put.stderr);

    let hit = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args([
            "build",
            "find",
            "--step",
            "elohim-edge:cargo-build-storage",
            "--inputs-hash",
            "closure-a",
        ])
        .run()
        .expect("invoke find");
    assert!(hit.status.success(), "find failed: {}", hit.stderr);
    let rows: serde_json::Value = serde_json::from_str(hit.stdout.trim()).expect("json array");
    let rows = rows.as_array().expect("array");
    assert_eq!(rows.len(), 1, "one attestation for closure-a: {}", hit.stdout);
    assert_eq!(rows[0]["commit"], first);
    assert_eq!(rows[0]["attestation"]["inputsHash"], "closure-a");
    assert_eq!(rows[0]["attestation"]["success"], true);
    assert_eq!(rows[0]["attestation"]["stepName"], "elohim-edge:cargo-build-storage");
    assert!(
        rows[0]["attestation"]["agentId"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "the workspace key signs every attestation: {}",
        hit.stdout
    );

    let miss = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args([
            "build",
            "find",
            "--step",
            "elohim-edge:cargo-build-storage",
            "--inputs-hash",
            "closure-b",
        ])
        .run()
        .expect("invoke find miss");
    assert!(
        miss.status.success(),
        "a miss is an answer, not a failure: {}",
        miss.stderr
    );
    assert_eq!(miss.stdout.trim(), "[]");
    staging_dump_path(
        &["brit-build-ref", "build", "find"],
        &format!("{}\n---stderr---\n{}", hit.stdout, hit.stderr),
    );
}

// ============================================================================
// deploy subcommands
// ============================================================================

#[test]
fn deploy_list_in_empty_repo() {
    let temp = TestRepo::new("deploy-list").expect("repo");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
        .args([
            "validate",
            "get",
            "--step",
            "no-such-step",
            "--check",
            "lint@v1",
            "--repo",
        ])
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let cap = BritInvocation::new(bin)
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

// ============================================================================
// covenant subcommands
// ============================================================================

fn covenant_put(
    bin: &std::path::Path,
    repo: &std::path::Path,
    session: &str,
    kind: &str,
    quote: &str,
    payload: &str,
) -> serde_json::Value {
    let cap = BritInvocation::new(bin)
        .arg("--repo")
        .arg(repo)
        .args([
            "covenant",
            "put",
            "--session",
            session,
            "--kind",
            kind,
            "--party",
            "human:matthew",
            "--quote",
            quote,
            "--payload-json",
            payload,
            "--output-json",
        ])
        .run()
        .expect("invoke covenant put");
    assert!(cap.status.success(), "covenant put failed: {}", cap.stderr);
    serde_json::from_str(cap.stdout.trim()).expect("put prints {cid, clause}")
}

fn covenant_list(bin: &std::path::Path, repo: &std::path::Path, session: &str) -> Vec<serde_json::Value> {
    let cap = BritInvocation::new(bin)
        .arg("--repo")
        .arg(repo)
        .args(["covenant", "list", "--session", session, "--output-json"])
        .run()
        .expect("invoke covenant list");
    assert!(cap.status.success(), "covenant list failed: {}", cap.stderr);
    serde_json::from_str::<serde_json::Value>(cap.stdout.trim())
        .expect("json array")
        .as_array()
        .expect("array")
        .clone()
}

#[test]
fn covenant_put_list_show_roundtrip() {
    let temp = TestRepo::new("covenant").expect("repo");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    assert_eq!(covenant_list(&bin, temp.path(), "s1"), Vec::<serde_json::Value>::new());

    let first = covenant_put(
        &bin,
        temp.path(),
        "s1",
        "affirmsStewardOfRecord",
        "I am the steward",
        "{}",
    );
    let second = covenant_put(
        &bin,
        temp.path(),
        "s1",
        "declaresPurpose",
        "docs only today",
        r#"{"purpose":"docs","writeSet":["docs/**"]}"#,
    );
    assert_ne!(first["cid"], second["cid"]);
    assert_eq!(first["clause"]["party"], "human:matthew");
    assert_eq!(first["clause"]["validUntil"], serde_json::Value::Null);

    let rows = covenant_list(&bin, temp.path(), "s1");
    assert_eq!(rows.len(), 2, "two puts, two clauses: {rows:?}");
    assert_eq!(rows[0]["cid"], first["cid"]);
    assert_eq!(rows[1]["cid"], second["cid"]);
    assert!(covenant_list(&bin, temp.path(), "other-session").is_empty());

    let cid = first["cid"].as_str().unwrap();
    let show = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args(["covenant", "show", "--cid", cid, "--output-json"])
        .run()
        .expect("invoke show");
    assert!(show.status.success(), "show failed: {}", show.stderr);
    let shown: serde_json::Value = serde_json::from_str(show.stdout.trim()).unwrap();
    assert_eq!(shown, first);

    let missing = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args(["covenant", "show", "--cid", "bafyreimissing"])
        .run()
        .expect("invoke show missing");
    assert!(!missing.status.success(), "an unknown clause is a failure");

    let bad = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args([
            "covenant",
            "put",
            "--session",
            "s1",
            "--kind",
            "variesRule",
            "--party",
            "p",
            "--quote",
            "q",
            "--payload-json",
            "{}",
        ])
        .run()
        .expect("invoke bad put");
    assert!(!bad.status.success(), "variesRule with an empty payload is malformed");
    staging_dump_path(
        &["brit-build-ref", "covenant", "list"],
        &serde_json::to_string_pretty(&rows).unwrap(),
    );
}

#[test]
fn covenant_put_works_from_a_linked_worktree() {
    let temp = TestRepo::new("covenant-wt").expect("repo");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let wt_parent = tempfile::tempdir().expect("tempdir");
    let wt = wt_parent.path().join("wt");
    let st = std::process::Command::new("git")
        .arg("-C")
        .arg(temp.path())
        .args(["worktree", "add", "-q", "-b", "side"])
        .arg(&wt)
        .status()
        .expect("git worktree add");
    assert!(st.success());
    assert!(wt.join(".git").is_file(), "linked worktree has a .git file");

    let put = covenant_put(&bin, &wt, "s-wt", "affirmsStewardOfRecord", "from the worktree", "{}");
    assert!(
        temp.path().join(".git/brit/agent-key").is_file(),
        "key lands in the common git dir"
    );
    // Refs are shared across worktrees: the main checkout sees the clause too.
    let rows = covenant_list(&bin, temp.path(), "s-wt");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["cid"], put["cid"]);
}
