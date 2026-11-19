//! `brit merge-base --is-ancestor <A> <B>` answers like Git through its exit code alone: 0 when `A` is
//! an ancestor of `B` (or the same commit), 1 when it is not, 128 for a revision that names no commit.
use anyhow::Result;
use cli_journey::support::{
    parity::{assert_same_streams, git_ok},
    test_repo::TestRepo,
};

#[test]
fn merge_base_is_ancestor_matches_git() -> Result<()> {
    let repo = TestRepo::new("merge-base-is-ancestor")?;
    let root = repo.path();
    repo.commit_file("a", "a\n")?;
    repo.commit_file("b", "b\n")?;
    git_ok(root, &["tag", "-a", "v1", "-m", "annotated", "HEAD~1"])?;
    git_ok(root, &["switch", "-q", "-c", "side", "HEAD~1"])?;
    repo.commit_file("s", "s\n")?;
    git_ok(root, &["switch", "-q", "main"])?;

    for (args, code) in [
        (&["merge-base", "--is-ancestor", "HEAD~1", "HEAD"][..], 0),
        (&["merge-base", "--is-ancestor", "HEAD~2", "side"], 0),
        (&["merge-base", "--is-ancestor", "v1", "main"], 0),
        (&["merge-base", "--is-ancestor", "HEAD", "HEAD"], 0),
        (&["merge-base", "--is-ancestor", "HEAD", "HEAD~1"], 1),
        (&["merge-base", "--is-ancestor", "side", "main"], 1),
        (&["merge-base", "--is-ancestor", "main", "side"], 1),
        (&["merge-base", "--is-ancestor", "nope", "HEAD"], 128),
        (&["merge-base", "--is-ancestor", "HEAD", "nope"], 128),
        (&["merge-base", "--is-ancestor", "HEAD^{tree}", "HEAD"], 128),
        (&["merge-base", "--is-ancestor", "HEAD"], 129),
        (&["merge-base", "--is-ancestor", "HEAD~1", "HEAD", "side"], 128),
    ] {
        let out = assert_same_streams(root, args)?;
        assert_eq!(out.code, Some(code), "{args:?}: {out:?}");
        assert!(out.stdout.is_empty(), "the answer is the exit code alone: {out:?}");
    }
    Ok(())
}
