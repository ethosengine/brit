//! `brit ls-remote` prints what `git ls-remote` prints, `<oid>\t<refname>` in server order with
//! peeled tags, for the remote and pattern forms agents use, and leads to `brit remote refs`.
use anyhow::Result;
use cli_journey::support::{
    mock_remote::MockRemote,
    parity::{assert_help_names, assert_same, git_ok},
    test_repo::TestRepo,
};

#[test]
fn ls_remote_matches_git() -> Result<()> {
    let remote = MockRemote::new("ls-remote")?;
    let source = TestRepo::new("ls-remote-source")?;
    let src = source.path();
    source.commit_file("a", "a\n")?;
    git_ok(src, &["branch", "dev"])?;
    git_ok(src, &["branch", "feat/steward-arrival-2"])?;
    git_ok(src, &["branch", "handoff/x"])?;
    git_ok(src, &["tag", "-a", "v1", "-m", "annotated", "HEAD~1"])?;
    git_ok(src, &["tag", "light"])?;
    let url = remote.url();
    git_ok(
        src,
        &[
            "push",
            "-q",
            &url,
            "refs/heads/*:refs/heads/*",
            "refs/tags/*:refs/tags/*",
        ],
    )?;
    git_ok(remote.path(), &["symbolic-ref", "HEAD", "refs/heads/main"])?;

    let local = TestRepo::new("ls-remote-local")?;
    let root = local.path();
    git_ok(root, &["remote", "add", "origin", &url])?;
    let path = remote.path().to_str().expect("UTF-8 path");

    let forms: &[&[&str]] = &[
        &["ls-remote"],
        &["ls-remote", "origin"],
        &["ls-remote", "origin", "refs/heads/dev"],
        &["ls-remote", "origin", "dev"],
        &["ls-remote", "origin", "refs/heads/main", "refs/heads/dev"],
        &[
            "ls-remote",
            "origin",
            "refs/heads/dev",
            "refs/heads/feat/steward-arrival*",
        ],
        &["ls-remote", "origin", "refs/heads/*", "refs/tags/v1*"],
        &["ls-remote", "origin", "refs/tags/*"],
        &["ls-remote", "origin", "refs/tags/v1"],
        &["ls-remote", "origin", "nothing-matches"],
        &["ls-remote", "--heads", "origin"],
        &["ls-remote", "--heads", "origin", "dev"],
        &["ls-remote", "--tags", "origin"],
        &["ls-remote", "--refs", "origin"],
        &["ls-remote", &url, "refs/heads/dev"],
        &["ls-remote", path, "dev"],
    ];
    for form in forms {
        let out = assert_same(root, form)?;
        assert!(
            out.stdout.lines().all(|line| line.split_once('\t').is_some()),
            "every line is `<oid>\\t<refname>`: {out:?}"
        );
    }

    assert_help_names("ls-remote", "brit remote refs")
}
