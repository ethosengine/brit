//! Integration tests for ContentFingerprint::from_repo_globs.
//! These run only when the `repo` feature is enabled.

#![cfg(feature = "repo")]

use std::collections::BTreeMap;
use std::process::Command;

use brit_graph::fingerprint::ContentFingerprint;
use tempfile::TempDir;

/// Initialize a temp git repo with a few files committed.
/// Returns (TempDir keep-alive, repo path, head ObjectId).
fn init_repo_with_files(files: &[(&str, &str)]) -> (TempDir, std::path::PathBuf, gix::ObjectId) {
    let dir = TempDir::new().expect("temp");
    let path = dir.path().to_path_buf();
    Command::new("git").args(["init", "-q"]).current_dir(&path).status().expect("init");
    Command::new("git").args(["config", "user.email", "t@t.t"]).current_dir(&path).status().expect("");
    Command::new("git").args(["config", "user.name", "t"]).current_dir(&path).status().expect("");

    for (rel, contents) in files {
        let abs = path.join(rel);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&abs, contents).expect("write");
        Command::new("git").args(["add", rel]).current_dir(&path).status().expect("add");
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
    assert!(fp.inputs.contains_key("src/foo.ts"), "found keys: {:?}", fp.inputs.keys().collect::<Vec<_>>());
}

#[test]
fn deterministic_across_calls_same_inputs() {
    let (_keep, path, head) = init_repo_with_files(&[
        ("src/a.ts", "a\n"),
        ("src/b.ts", "b\n"),
    ]);
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

    assert_ne!(fp_a.cid, fp_b.cid, "different content must produce different fingerprint");
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
    let (_keep, path, head) = init_repo_with_files(&[
        ("src/foo.ts", "ts\n"),
        ("src/bar.rs", "rs\n"),
        ("README.md", "md\n"),
    ]);
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
    assert!(matches!(err, brit_graph::fingerprint::FingerprintError::InvalidGlob { .. }));
}
