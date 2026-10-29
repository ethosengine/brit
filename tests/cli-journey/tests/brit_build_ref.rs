//! Coverage tests for the brit-build-ref binary.
//!
//! Manages build/deploy/validate/reach attestation refs and signer inspection. Each leaf
//! subcommand gets its own test that exercises the operation against
//! a temp git repo and dumps captured output to staging.
//!
//! Subcommand tree (12 leaves):
//!   build   → put, get, list
//!   deploy  → put, get, list
//!   validate → put, get, list
//!   reach   → compute, get
//!   signer
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

#[test]
fn signer_is_read_only_and_matches_subsequent_build_attestations() {
    let temp = TestRepo::new("signer").expect("repo");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let key_path = temp.path().join(".git/brit/agent-key");

    let absent = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args(["signer", "--json"])
        .run()
        .expect("invoke signer without a key");
    assert!(absent.status.success(), "an absent key is an answer: {}", absent.stderr);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(absent.stdout.trim()).expect("signer JSON"),
        serde_json::json!({ "agentId": null })
    );
    assert!(!key_path.exists(), "signer must not mint a key");
    assert!(
        !temp.path().join(".git/brit").exists(),
        "signer must not create the key directory"
    );

    let manifest = "bafyreig7os3vfj4z63vgy7lpfgcnzzr2z26vrvdtheapoegvgzhnnhxkr4";
    let output = "bafyreieicbex3dkkgounj4latxya32ezpl6622lpij327mzndwptfqkf7q";
    let bootstrap = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args([
            "build",
            "put",
            "--step",
            "bootstrap",
            "--manifest",
            manifest,
            "--output",
            output,
            "--inputs-hash",
            "bootstrap-inputs",
        ])
        .run()
        .expect("bootstrap signer through a writer");
    assert!(
        bootstrap.status.success(),
        "bootstrap build put failed: {}",
        bootstrap.stderr
    );

    let signer = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args(["signer", "--json"])
        .run()
        .expect("invoke signer with a key");
    assert!(signer.status.success(), "signer failed: {}", signer.stderr);
    let signer_json: serde_json::Value = serde_json::from_str(signer.stdout.trim()).expect("signer JSON");
    let agent_id = signer_json["agentId"].as_str().expect("agentId is present");
    assert_eq!(agent_id.len(), 64, "agentId is an Ed25519 public key in hex");
    let seed_hex: String =
        fs::read(&key_path)
            .expect("writer created the key")
            .iter()
            .fold(String::with_capacity(64), |mut hex, byte| {
                use std::fmt::Write as _;
                write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
                hex
            });
    assert_eq!(seed_hex.len(), 64, "the stored seed is 32 bytes");
    assert!(
        !signer.stdout.contains(&seed_hex),
        "signer stdout must not contain the private seed"
    );
    assert!(
        !signer.stdout.contains(&key_path.display().to_string()),
        "signer stdout must not contain the key path"
    );

    let put = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args([
            "build",
            "put",
            "--step",
            "after-signer",
            "--manifest",
            manifest,
            "--output",
            output,
            "--inputs-hash",
            "after-signer-inputs",
        ])
        .run()
        .expect("invoke build put after signer");
    assert!(put.status.success(), "build put failed: {}", put.stderr);
    let get = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args(["build", "get", "--step", "after-signer"])
        .run()
        .expect("read build attestation");
    assert!(get.status.success(), "build get failed: {}", get.stderr);
    let attestation: serde_json::Value = serde_json::from_str(get.stdout.trim()).expect("attestation JSON");
    assert_eq!(
        attestation["agentId"], agent_id,
        "signer reports the identity writers use"
    );
}

#[cfg(unix)]
#[test]
fn signer_refuses_malformed_and_unreadable_keys_without_disclosing_their_location() {
    use std::os::unix::fs::symlink;

    let temp = TestRepo::new("signer-invalid").expect("repo");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let key_path = temp.path().join(".git/brit/agent-key");
    fs::create_dir_all(key_path.parent().expect("key has a parent")).expect("create key directory");
    fs::write(&key_path, b"not-a-32-byte-seed").expect("write malformed key");

    let malformed = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args(["signer", "--json"])
        .run()
        .expect("invoke signer with malformed key");
    assert!(!malformed.status.success(), "a malformed key must fail");
    assert!(malformed.stdout.is_empty(), "failure must not emit partial JSON");
    assert!(
        malformed.stderr.contains("cannot load the repository signer key"),
        "failure carries a diagnostic: {}",
        malformed.stderr
    );
    assert!(
        !malformed.stderr.contains(&key_path.display().to_string()),
        "diagnostic must not disclose the key path"
    );

    fs::remove_file(&key_path).expect("remove malformed key");
    fs::create_dir(&key_path).expect("make the key unreadable as a file");
    let unreadable = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args(["signer", "--json"])
        .run()
        .expect("invoke signer with unreadable key");
    assert!(!unreadable.status.success(), "an unreadable key must fail");
    assert!(unreadable.stdout.is_empty(), "failure must not emit partial JSON");
    assert!(
        unreadable.stderr.contains("cannot load the repository signer key"),
        "failure carries a diagnostic: {}",
        unreadable.stderr
    );
    assert!(
        !unreadable.stderr.contains(&key_path.display().to_string()),
        "diagnostic must not disclose the key path"
    );

    fs::remove_dir(&key_path).expect("remove unreadable key stand-in");
    symlink(key_path.with_file_name("missing-seed"), &key_path).expect("create broken key symlink");
    let broken = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args(["signer", "--json"])
        .run()
        .expect("invoke signer with broken key symlink");
    assert!(
        !broken.status.success(),
        "a broken key symlink must not read as absence"
    );
    assert!(broken.stdout.is_empty(), "failure must not emit partial JSON");
    assert!(
        broken.stderr.contains("cannot load the repository signer key"),
        "failure carries a diagnostic: {}",
        broken.stderr
    );
    assert!(
        !broken.stderr.contains(&key_path.display().to_string()),
        "diagnostic must not disclose the key path"
    );
}

#[cfg(unix)]
#[test]
fn first_writer_publishes_the_key_with_mode_0600_under_umask_022() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TestRepo::new("signer-mode").expect("repo");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let isolated_config = tempfile::tempdir().expect("isolated Git config");
    let mut command = std::process::Command::new("sh");
    gix_testtools::configure_git_environment(&mut command, temp.path());
    let output = command
        .env("GIT_CONFIG_GLOBAL", isolated_config.path().join("missing.gitconfig"))
        .args(["-c", "umask 022; exec \"$@\"", "brit-build-ref"])
        .arg(&bin)
        .arg("--repo")
        .arg(temp.path())
        .args([
            "build",
            "put",
            "--step",
            "mode",
            "--manifest",
            "bafyreig7os3vfj4z63vgy7lpfgcnzzr2z26vrvdtheapoegvgzhnnhxkr4",
            "--output",
            "bafyreieicbex3dkkgounj4latxya32ezpl6622lpij327mzndwptfqkf7q",
            "--inputs-hash",
            "mode-inputs",
        ])
        .output()
        .expect("run first writer under umask 022");
    assert!(
        output.status.success(),
        "build put failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mode = fs::metadata(temp.path().join(".git/brit/agent-key"))
        .expect("key metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "the published seed is private regardless of umask");
}

#[test]
fn repository_resolution_failure_creates_no_brit_state() {
    let temp = tempfile::tempdir().expect("non-repository directory");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    // Discovery must not climb out of the temporary directory into an enclosing repository.
    let ceiling = temp
        .path()
        .parent()
        .expect("temporary directory has a parent")
        .to_path_buf();

    let signer = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .env("GIT_CEILING_DIRECTORIES", &ceiling)
        .args(["signer", "--json"])
        .run()
        .expect("invoke signer outside a repository");
    assert!(
        !signer.status.success(),
        "signer must refuse an unresolved repository home"
    );

    let docs = temp.path().join("docs");
    fs::create_dir(&docs).expect("create seal input");
    fs::write(docs.join("a.md"), "alpha").expect("write seal input");
    let seal = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(temp.path())
        .env("GIT_CEILING_DIRECTORIES", &ceiling)
        .args(["meta", "seal", "--dir"])
        .arg(&docs)
        .run()
        .expect("invoke meta seal outside a repository");
    assert!(
        !seal.status.success(),
        "meta seal must refuse an unresolved repository home"
    );
    assert!(
        !temp.path().join(".git/brit").exists(),
        "failure must not create key or object state"
    );
}

#[test]
fn meta_sealed_from_a_linked_worktree_verifies_from_both_checkouts() {
    let temp = TestRepo::new("meta-worktree").expect("repo");
    let Some(bin) = brit_build_ref_bin() else {
        eprintln!("skipping: brit-build-ref binary not built (run `cargo build -p brit-build-ref --release`)");
        return;
    };
    let wt_parent = tempfile::tempdir().expect("worktree parent");
    let wt = wt_parent.path().join("linked");
    let add = gix_testtools::git_command(temp.path())
        .args(["worktree", "add", "-q", "-b", "meta-side"])
        .arg(&wt)
        .output()
        .expect("git worktree add");
    assert!(
        add.status.success(),
        "worktree add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    assert!(wt.join(".git").is_file(), "a linked worktree uses a gitfile");
    let docs = wt.join("docs");
    fs::create_dir(&docs).expect("create seal input");
    fs::write(docs.join("a.md"), "alpha").expect("write seal input");

    let seal = BritInvocation::new(&bin)
        .arg("--repo")
        .arg(&wt)
        .args(["meta", "seal", "--dir"])
        .arg(&docs)
        .run()
        .expect("seal from linked worktree");
    assert!(seal.status.success(), "meta seal failed: {}", seal.stderr);
    let cid = seal.stdout.trim();
    assert!(
        temp.path().join(".git/brit/objects").join(cid).is_file(),
        "object is stored in the common Git directory"
    );

    for repo in [temp.path(), wt.as_path()] {
        let verify = BritInvocation::new(&bin)
            .arg("--repo")
            .arg(repo)
            .args(["meta", "verify", "--cid", cid])
            .run()
            .expect("verify sealed metadata");
        assert!(
            verify.status.success(),
            "verify from {} failed: {}",
            repo.display(),
            verify.stderr
        );
    }
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
