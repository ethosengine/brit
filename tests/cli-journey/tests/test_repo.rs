//! Self-tests for the TestRepo helper.

use cli_journey::support::test_repo::TestRepo;

#[test]
fn test_repo_initializes_with_one_commit() {
    let repo = TestRepo::new("base").expect("init");
    assert!(repo.path().exists());
    assert!(repo.path().join(".git").exists());
    let head = repo.head_id().expect("head");
    assert_eq!(head.len(), 40, "git SHA-1 hex length");
}

#[test]
fn test_repo_commit_file_returns_stable_sha_with_static_env() {
    // Two TestRepos with identical fixture content should produce identical SHAs
    // because both use the static-git-environment for author/committer/dates.
    let a = TestRepo::new("a").expect("a");
    let b = TestRepo::new("b").expect("b");
    let sha_a = a.commit_file("foo.txt", "hello\n").expect("commit a");
    let sha_b = b.commit_file("foo.txt", "hello\n").expect("commit b");
    assert_eq!(sha_a, sha_b, "deterministic SHA across instances");
}

#[test]
fn test_repo_drop_cleans_up_path() {
    let path = {
        let repo = TestRepo::new("ephemeral").expect("init");
        repo.path().to_path_buf()
    };
    // After drop, the temp dir is gone
    assert!(!path.exists(), "temp dir removed on drop");
}
