//! `brit rev-parse` prints what `git rev-parse` prints for the forms agents use, from a main
//! worktree, its subdirectory, and a linked worktree, and its help leads to `brit revision resolve`.
use anyhow::Result;
use cli_journey::support::{
    parity::{assert_help_names, assert_same, assert_same_streams, brit, git, git_ok},
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

/// Flags apply from their position, `--verify` lets exactly one revision and nothing else reach stdout,
/// and the failures a script checks exit and write to stderr like Git.
#[test]
fn rev_parse_reads_arguments_in_order_like_git() -> Result<()> {
    let repo = TestRepo::new("rev-parse-order")?;
    let root = repo.path();
    repo.commit_file("sub/f", "f\n")?;
    repo.commit_file("g", "g\n")?;
    git_ok(root, &["switch", "-q", "-c", "side", "HEAD~1"])?;
    repo.commit_file("s", "s\n")?;
    git_ok(root, &["switch", "-q", "main"])?;

    let forms: &[&[&str]] = &[
        // --verify: one revision, nothing else on stdout.
        &["rev-parse", "--verify", "HEAD", "--", "x"],
        &["rev-parse", "--verify", "HEAD", "--"],
        &["rev-parse", "--verify", "^HEAD"],
        &["rev-parse", "--verify", "HEAD~1..HEAD"],
        &["rev-parse", "--verify", "-q", "HEAD~1..HEAD"],
        &["rev-parse", "--verify", "main...side"],
        &["rev-parse", "--verify", "missing", "--", "x"],
        &["rev-parse", "HEAD", "--verify"],
        // `--` is kept, and a missing revision before it is a bad revision.
        &["rev-parse", "HEAD", "--"],
        &["rev-parse", "HEAD", "--", "x", "y"],
        &["rev-parse", "--", "x"],
        &["rev-parse", "missing", "--", "x"],
        &["rev-parse", "main...side"],
        &["rev-parse", "side...main"],
        // --short: Git's 4-digit minimum and positional effect.
        &["rev-parse", "--short=", "HEAD"],
        &["rev-parse", "--short=-1", "HEAD"],
        &["rev-parse", "--short=0", "HEAD"],
        &["rev-parse", "HEAD", "--short", "HEAD"],
        &["rev-parse", "HEAD", "--short=5", "main"],
        // Repeated path queries print once per occurrence.
        &["rev-parse", "--git-dir", "--git-dir"],
        &["rev-parse", "--git-dir", "HEAD", "--git-dir"],
        // Diagnostics name the missing path.
        &["rev-parse", "HEAD:missing"],
        &["rev-parse", "HEAD:g", "HEAD:missing"],
        &["rev-parse", ":missing"],
        &["rev-parse", "HEAD", "missing"],
        &["rev-parse", "sub/f", "--git-dir"],
    ];
    for form in forms {
        assert_same_streams(root, form)?;
    }

    // Inside the Git directory there is no worktree. Only `--show-toplevel` answers there: the other
    // queries fail at Brit's discovery, a recorded difference.
    let git_dir = root.join(".git");
    for form in [
        &["rev-parse", "--show-toplevel"][..],
        &["rev-parse", "--show-toplevel", "HEAD"][..],
        &["rev-parse", "--verify", "--show-toplevel"][..],
    ] {
        assert_same_streams(&git_dir, form)?;
    }
    // Below it the worktree is still out of reach, and the Git directory is found.
    for form in [
        &["rev-parse", "--show-toplevel"][..],
        &["rev-parse", "--git-dir", "HEAD"][..],
    ] {
        assert_same_streams(&git_dir.join("refs"), form)?;
    }

    // Global -v buffers output: a failure still forwards what was printed before it.
    let verbose = brit(root, &["-v", "rev-parse", "HEAD", "missing"])?;
    let expected = git(root, &["rev-parse", "HEAD", "missing"])?;
    assert_eq!(
        (&verbose.stdout, verbose.code),
        (&expected.stdout, expected.code),
        "brit -v: {verbose:?}"
    );
    assert!(
        verbose.stderr.contains("ambiguous argument 'missing'"),
        "brit -v forwards the diagnostic: {verbose:?}"
    );
    Ok(())
}

/// From `/outer/sub/.git`, where that directory is itself the top of a worktree with its own `.git`,
/// Brit selects the same repository as Git: the inner one, never the enclosing `/outer`. A selector
/// that skipped the child `.git` would make a mutating command write to the outer repository.
#[test]
fn a_worktree_named_dot_git_inside_another_repository_is_selected_like_git() -> Result<()> {
    let outer = TestRepo::new("rev-parse-outer")?;
    outer.commit_file("o", "o\n")?;
    let inner_top = outer.path().join("sub/.git");
    std::fs::create_dir_all(&inner_top)?;
    git_ok(&inner_top, &["init", "-q", "--initial-branch=inner"])?;
    std::fs::write(inner_top.join("i"), "i\n")?;
    git_ok(&inner_top, &["add", "i"])?;
    git_ok(&inner_top, &["commit", "-q", "-m", "inner"])?;
    assert_ne!(
        git_ok(&inner_top, &["rev-parse", "HEAD"])?,
        git_ok(outer.path(), &["rev-parse", "HEAD"])?,
        "the two repositories must be told apart by HEAD"
    );
    for form in [
        &["rev-parse", "HEAD"][..],
        &["rev-parse", "--show-toplevel", "--git-dir", "--absolute-git-dir"],
        &["rev-parse", "--abbrev-ref", "HEAD"],
        &["for-each-ref", "--format=%(refname)"],
    ] {
        assert_same_streams(&inner_top, form)?;
    }
    Ok(())
}
