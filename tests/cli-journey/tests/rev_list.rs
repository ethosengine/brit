//! `brit rev-list --count` prints what `git rev-list --count` prints for a revision, `A..B`, `^A` and
//! `--left-right A...B`, fails like Git for an unknown revision, and leads to `brit revision list`.
use anyhow::Result;
use cli_journey::support::{
    parity::{assert_help_names, assert_same_streams, git_ok},
    test_repo::TestRepo,
};

#[test]
fn rev_list_count_matches_git() -> Result<()> {
    let repo = TestRepo::new("rev-list-count")?;
    let root = repo.path();
    repo.commit_file("a", "a\n")?;
    repo.commit_file("b", "b\n")?;
    git_ok(root, &["switch", "-q", "-c", "side", "HEAD~1"])?;
    repo.commit_file("s1", "s\n")?;
    repo.commit_file("s2", "s\n")?;
    git_ok(root, &["switch", "-q", "main"])?;
    // A merge, so that both sides share history below a merge base and above it.
    git_ok(root, &["switch", "-q", "-c", "merged", "main"])?;
    git_ok(root, &["merge", "-q", "--no-edit", "--no-ff", "side"])?;
    repo.commit_file("m", "m\n")?;
    git_ok(root, &["switch", "-q", "main"])?;
    repo.commit_file("c", "c\n")?;
    git_ok(root, &["tag", "-a", "v1", "-m", "annotated", "side"])?;

    for args in [
        &["rev-list", "--count", "HEAD"][..],
        &["rev-list", "--count", "side"],
        &["rev-list", "--count", "v1"],
        &["rev-list", "--count", "main..side"],
        &["rev-list", "--count", "side..main"],
        &["rev-list", "--count", "main..merged"],
        &["rev-list", "--count", "HEAD", "^side"],
        &["rev-list", "--count", "main", "side"],
        &["rev-list", "--count", "main...side"],
        &["rev-list", "--left-right", "--count", "main...side"],
        &["rev-list", "--left-right", "--count", "side...main"],
        &["rev-list", "--left-right", "--count", "merged...main"],
        &["rev-list", "--left-right", "--count", "main..side"],
        &["rev-list", "--count", "--left-right", "main...merged"],
        &["rev-list", "HEAD", "--count"],
        &["rev-list", "--count", "nope"],
        &["rev-list", "--count", "HEAD~20"],
        &["rev-list", "--count"],
    ] {
        let out = assert_same_streams(root, args)?;
        if out.code == Some(0) {
            assert!(!out.stdout.is_empty(), "{args:?} prints a count: {out:?}");
        }
    }

    assert_help_names("rev-list", "brit revision list")
}
