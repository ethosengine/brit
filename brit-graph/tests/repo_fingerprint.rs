//! Integration tests for ContentFingerprint::from_repo_globs.
//! These run only when the `repo` feature is enabled.

#![cfg(feature = "repo")]

use std::{collections::BTreeMap, process::Command};

use brit_graph::fingerprint::ContentFingerprint;
use tempfile::TempDir;

/// Initialize a temp git repo with a few files committed.
/// Returns (TempDir keep-alive, repo path, head ObjectId).
fn init_repo_with_files(files: &[(&str, &str)]) -> (TempDir, std::path::PathBuf, gix::ObjectId) {
    let dir = TempDir::new().expect("temp");
    let path = dir.path().to_path_buf();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&path)
        .status()
        .expect("init");
    Command::new("git")
        .args(["config", "user.email", "t@t.t"])
        .current_dir(&path)
        .status()
        .expect("");
    Command::new("git")
        .args(["config", "user.name", "t"])
        .current_dir(&path)
        .status()
        .expect("");

    for (rel, contents) in files {
        let abs = path.join(rel);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&abs, contents).expect("write");
        Command::new("git")
            .args(["add", rel])
            .current_dir(&path)
            .status()
            .expect("add");
    }
    Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(&path)
        .status()
        .expect("commit");

    let repo = gix::open(&path).expect("open");
    let head_id = repo.head_id().expect("head_id").detach();

    (dir, path, head_id)
}

#[test]
fn empty_patterns_produces_empty_inputs_fingerprint() {
    let (_keep, path, head) = init_repo_with_files(&[("a.txt", "hello\n")]);
    let repo = gix::open(&path).expect("open");
    let fp = ContentFingerprint::from_repo_globs(&repo, head, &[]).expect("compute");
    assert!(fp.inputs.is_empty(), "no patterns -> no inputs");

    // Same as compute(empty)
    let baseline = ContentFingerprint::compute(&BTreeMap::new());
    assert_eq!(fp.cid, baseline.cid);
}

#[test]
fn single_pattern_matches_one_file() {
    let (_keep, path, head) = init_repo_with_files(&[
        ("src/foo.ts", "console.log('foo');\n"),
        ("src/bar.rs", "fn bar() {}\n"),
        ("README.md", "# project\n"),
    ]);
    let repo = gix::open(&path).expect("open");

    let patterns = vec!["src/**/*.ts".to_string()];
    let fp = ContentFingerprint::from_repo_globs(&repo, head, &patterns).expect("compute");

    // Only foo.ts should be in the inputs
    assert_eq!(fp.inputs.len(), 1, "one .ts file");
    assert!(
        fp.inputs.contains_key("src/foo.ts"),
        "found keys: {:?}",
        fp.inputs.keys().collect::<Vec<_>>()
    );
}

#[test]
fn deterministic_across_calls_same_inputs() {
    let (_keep, path, head) = init_repo_with_files(&[("src/a.ts", "a\n"), ("src/b.ts", "b\n")]);
    let repo = gix::open(&path).expect("open");

    let patterns = vec!["src/**/*.ts".to_string()];
    let fp1 = ContentFingerprint::from_repo_globs(&repo, head, &patterns).expect("1");
    let fp2 = ContentFingerprint::from_repo_globs(&repo, head, &patterns).expect("2");

    assert_eq!(fp1.cid, fp2.cid, "deterministic");
    assert_eq!(fp1.inputs.len(), fp2.inputs.len());
}

#[test]
fn different_content_different_fingerprint() {
    // Same patterns, same paths, different file CONTENT -> different fingerprint.
    // This is the property that the OLD pattern-bytes hashing did NOT have.
    let (_keep_a, path_a, head_a) = init_repo_with_files(&[("src/foo.ts", "version 1\n")]);
    let (_keep_b, path_b, head_b) = init_repo_with_files(&[("src/foo.ts", "version 2\n")]);

    let repo_a = gix::open(&path_a).expect("a");
    let repo_b = gix::open(&path_b).expect("b");

    let patterns = vec!["src/**/*.ts".to_string()];
    let fp_a = ContentFingerprint::from_repo_globs(&repo_a, head_a, &patterns).expect("a");
    let fp_b = ContentFingerprint::from_repo_globs(&repo_b, head_b, &patterns).expect("b");

    assert_ne!(
        fp_a.cid, fp_b.cid,
        "different content must produce different fingerprint"
    );
}

#[test]
fn no_matching_files_is_empty_fingerprint() {
    let (_keep, path, head) = init_repo_with_files(&[("README.md", "x")]);
    let repo = gix::open(&path).expect("open");
    let patterns = vec!["src/**/*.ts".to_string()];
    let fp = ContentFingerprint::from_repo_globs(&repo, head, &patterns).expect("compute");
    assert!(fp.inputs.is_empty());
}

#[test]
fn multiple_patterns_combine() {
    let (_keep, path, head) =
        init_repo_with_files(&[("src/foo.ts", "ts\n"), ("src/bar.rs", "rs\n"), ("README.md", "md\n")]);
    let repo = gix::open(&path).expect("open");
    let patterns = vec!["src/**/*.ts".to_string(), "src/**/*.rs".to_string()];
    let fp = ContentFingerprint::from_repo_globs(&repo, head, &patterns).expect("compute");
    assert_eq!(fp.inputs.len(), 2);
    assert!(fp.inputs.contains_key("src/foo.ts"));
    assert!(fp.inputs.contains_key("src/bar.rs"));
}

#[test]
fn invalid_glob_returns_error() {
    let (_keep, path, head) = init_repo_with_files(&[("a.txt", "x")]);
    let repo = gix::open(&path).expect("open");
    let patterns = vec!["[invalid".to_string()];
    let err = ContentFingerprint::from_repo_globs(&repo, head, &patterns).unwrap_err();
    assert!(matches!(
        err,
        brit_graph::fingerprint::FingerprintError::InvalidGlob { .. }
    ));
}

/// Fingerprint material for a step whose globs reach no gitlink, pinned before
/// gitlink inputs existed. Any change here re-keys every existing evidence row.
const PINNED_NO_GITLINK_CID: &str = "bafyreigdvcc4q5lxgpfyltrexvuk7tabld5epekgubhxwl7crzzezbvsw4";

fn pinned_fixture_files() -> [(&'static str, &'static str); 3] {
    [
        ("src/a.ts", "a\n"),
        ("src/nested/b.rs", "fn b() {}\n"),
        ("README.md", "# readme\n"),
    ]
}

#[test]
fn fingerprint_without_gitlink_is_byte_identical_to_pre_gitlink_contract() {
    let (_keep, path, head) = init_repo_with_files(&pinned_fixture_files());
    let repo = gix::open(&path).expect("open");
    let patterns = vec!["src/**".to_string(), "README.md".to_string()];
    let fp = ContentFingerprint::from_repo_globs(&repo, head, &patterns).expect("compute");
    assert_eq!(fp.cid.to_string(), PINNED_NO_GITLINK_CID);
}

/// Commit a superproject holding `files` plus a gitlink at `elohim/rakia`
/// pinned to `pin`. The pinned commit object does not exist anywhere: the
/// submodule is neither cloned nor checked out.
fn init_repo_with_gitlink(files: &[(&str, &str)], pin: &str) -> (TempDir, std::path::PathBuf, gix::ObjectId) {
    let (keep, path, _) = init_repo_with_files(files);
    let status = Command::new("git")
        .args([
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{pin},elohim/rakia"),
        ])
        .current_dir(&path)
        .status()
        .expect("update-index");
    assert!(status.success(), "record gitlink");
    let status = Command::new("git")
        .args(["commit", "-q", "-m", "pin rakia"])
        .current_dir(&path)
        .status()
        .expect("commit");
    assert!(status.success(), "commit gitlink");
    let repo = gix::open(&path).expect("open");
    let head = repo.head_id().expect("head_id").detach();
    (keep, path, head)
}

const PIN_A: &str = "1111111111111111111111111111111111111111";
const PIN_B: &str = "2222222222222222222222222222222222222222";

fn gitlink_fp(pin: &str, patterns: &[&str]) -> ContentFingerprint {
    let (_keep, path, head) = init_repo_with_gitlink(&pinned_fixture_files(), pin);
    let repo = gix::open(&path).expect("open");
    let patterns: Vec<String> = patterns.iter().map(|p| (*p).to_string()).collect();
    ContentFingerprint::from_repo_globs(&repo, head, &patterns).expect("compute")
}

#[test]
fn crossing_glob_keys_the_gitlink_pin_without_submodule_objects() {
    let fp = gitlink_fp(PIN_A, &["elohim/rakia/schemas/**"]);
    assert!(fp.inputs.is_empty(), "no blob lives under an unchecked-out gitlink");
    assert_eq!(
        fp.gitlinks,
        BTreeMap::from([("elohim/rakia".to_string(), PIN_A.to_string())])
    );
    assert_eq!(fp.input_count(), 1);
    assert_ne!(
        fp.cid,
        ContentFingerprint::compute(&BTreeMap::new()).cid,
        "a reached gitlink is not the empty fingerprint"
    );
}

#[test]
fn moving_only_the_pin_changes_a_crossing_fingerprint() {
    let a = gitlink_fp(PIN_A, &["elohim/rakia/schemas/**", "src/**"]);
    let b = gitlink_fp(PIN_B, &["elohim/rakia/schemas/**", "src/**"]);
    assert_eq!(a.inputs, b.inputs, "blob inputs are unchanged");
    assert_ne!(a.cid, b.cid, "the pin move must re-key the step");
}

#[test]
fn gitlink_counts_once_however_many_globs_cross_it() {
    let fp = gitlink_fp(
        PIN_A,
        &["elohim/rakia/schemas/**", "elohim/rakia/**", "elohim/**", "**/*.json"],
    );
    assert_eq!(fp.gitlinks.len(), 1);
    assert_eq!(fp.input_count(), 1);
}

#[test]
fn non_crossing_step_in_a_superproject_keeps_its_pre_gitlink_fingerprint() {
    for pin in [PIN_A, PIN_B] {
        let fp = gitlink_fp(pin, &["src/**", "README.md", "elohim/other/**", "elohim/rakia.md"]);
        assert!(fp.gitlinks.is_empty(), "pin {pin}: no glob reaches elohim/rakia");
        assert_eq!(fp.cid.to_string(), PINNED_NO_GITLINK_CID, "pin {pin}");
    }
}

#[test]
fn gitlink_input_is_domain_separated_from_a_blob_at_the_same_path() {
    let gitlink = gitlink_fp(PIN_A, &["elohim/rakia"]);
    assert_eq!(gitlink.gitlinks.len(), 1);
    // A blob at the gitlink's path whose bytes spell the pin, or the material
    // a naive concatenation would produce, must not collide.
    for bytes in [PIN_A.as_bytes().to_vec(), format!("gitlink\0{PIN_A}").into_bytes()] {
        let blob = ContentFingerprint::compute(&BTreeMap::from([("elohim/rakia".to_string(), bytes)]));
        assert_ne!(gitlink.cid, blob.cid);
    }
}
