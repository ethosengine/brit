//! `brit rev-parse` prints what `git rev-parse` prints for the forms agents use, from a main
//! worktree, its subdirectory, and a linked worktree, and its help leads to `brit revision resolve`.
use anyhow::Result;
use cli_journey::support::{
    parity::{assert_help_names, assert_same, brit, git_ok},
    test_repo::TestRepo,
};

#[test]
fn rev_parse_matches_git_in_main_and_linked_worktrees() -> Result<()> {
    let repo = TestRepo::new("rev-parse")?;
    let root = repo.path();
    repo.commit_file("sub/f", "f\n")?;
    git_ok(root, &["tag", "-a", "v1", "-m", "annotated subject"])?;
    git_ok(root, &["tag", "light", "HEAD~1"])?;
    git_ok(root, &["update-ref", "refs/remotes/origin/main", "HEAD~1"])?;
    let elsewhere = tempfile::tempdir()?;
    let worktree = elsewhere.path().join("wt");
    git_ok(
        root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "topic",
            worktree.to_str().expect("UTF-8 path"),
        ],
    )?;
    std::fs::create_dir_all(worktree.join("sub"))?;

    let forms: &[&[&str]] = &[
        &["rev-parse", "HEAD"],
        &["rev-parse", "HEAD", "main", "v1", "light"],
        &["rev-parse", "HEAD^{tree}", "HEAD:sub/f", "v1^{commit}", "origin/main"],
        &["rev-parse", "HEAD~1..HEAD"],
        &["rev-parse", "^HEAD~1", "HEAD"],
        &["rev-parse", "--short", "HEAD"],
        &["rev-parse", "--short=12", "HEAD"],
        &["rev-parse", "--short=2", "origin/main"],
        &["rev-parse", "--short", "HEAD", "main"],
        &["rev-parse", "--verify", "HEAD"],
        &["rev-parse", "--verify", "HEAD^{commit}"],
        &["rev-parse", "--verify", "-q", "HEAD^{commit}"],
        &["rev-parse", "--verify", "--quiet", "--short", "HEAD"],
        &["rev-parse", "--verify", "nope"],
        &["rev-parse", "--verify", "-q", "nope"],
        &["rev-parse", "--verify", "HEAD", "main"],
        &["rev-parse", "nope"],
        &["rev-parse", "HEAD", "--", "x"],
        &["rev-parse", "--abbrev-ref", "HEAD"],
        &[
            "rev-parse",
            "--abbrev-ref",
            "HEAD",
            "v1",
            "main",
            "origin/main",
            "refs/heads/main",
            "HEAD~1",
        ],
        &["rev-parse", "--show-toplevel"],
        &["rev-parse", "--show-toplevel", "HEAD"],
        &["rev-parse", "--git-dir"],
        &["rev-parse", "--absolute-git-dir"],
        &["rev-parse", "--git-common-dir"],
        &["rev-parse", "--git-dir", "--git-common-dir", "--show-toplevel"],
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
            "--git-dir",
            "--show-toplevel",
        ],
        &[
            "rev-parse",
            "--path-format=relative",
            "--git-common-dir",
            "--show-toplevel",
        ],
    ];
    for cwd in [
        root.to_owned(),
        root.join("sub"),
        worktree.clone(),
        worktree.join("sub"),
    ] {
        for form in forms {
            assert_same(&cwd, form)?;
        }
    }

    // In a linked worktree the private and the shared Git directories differ.
    let dirs = brit(&worktree, &["rev-parse", "--git-dir", "--git-common-dir"])?;
    let lines: Vec<_> = dirs.stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{dirs:?}");
    assert_ne!(
        lines[0], lines[1],
        "linked worktree: --git-dir and --git-common-dir must differ"
    );

    let quiet = brit(root, &["rev-parse", "--verify", "-q", "nope"])?;
    assert!(quiet.stderr.is_empty(), "--verify -q is silent: {quiet:?}");

    assert_help_names("rev-parse", "brit revision resolve")
}
