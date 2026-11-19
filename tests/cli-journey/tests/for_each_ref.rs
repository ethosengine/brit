//! `brit for-each-ref` prints what `git for-each-ref` prints for the patterns, `--format` atoms and
//! `--sort` keys agents use, refuses an unimplemented atom by name, and leads to `brit branch list`.
use anyhow::Result;
use cli_journey::support::{
    parity::{assert_help_names, assert_same, brit, git_ok},
    test_repo::TestRepo,
};

fn commit_at(root: &std::path::Path, message: &str, date: &str) -> Result<String> {
    let out = gix_testtools::git_command(root)
        .args(["commit", "-q", "--allow-empty", "-m", message])
        .env("GIT_COMMITTER_DATE", date)
        .output()?;
    anyhow::ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    git_ok(root, &["rev-parse", "HEAD"])
}

#[test]
fn for_each_ref_matches_git() -> Result<()> {
    let repo = TestRepo::new("for-each-ref")?;
    let root = repo.path();
    let old = commit_at(root, "old subject\n\nbody line", "1000000100 +0000")?;
    git_ok(root, &["branch", "feat/old"])?;
    let newer = commit_at(root, "newer subject", "1000000300 +0200")?;
    git_ok(root, &["branch", "explore/0.7.1-a"])?;
    commit_at(root, "newest subject", "1000000500 -0500")?;
    git_ok(root, &["tag", "-a", "v1", "-m", "tag subject", &old])?;
    git_ok(root, &["tag", "light", &newer])?;
    git_ok(root, &["update-ref", "refs/remotes/origin/main", &newer])?;
    git_ok(
        root,
        &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"],
    )?;
    git_ok(root, &["update-ref", "refs/notes/brit/build/abc", &old])?;
    // `heads/main` is ambiguous with a tag named `main`, so it needs its `heads/` prefix.
    git_ok(root, &["tag", "main", &old])?;

    let forms: &[&[&str]] = &[
        &["for-each-ref"],
        &["for-each-ref", "refs/heads"],
        &["for-each-ref", "refs/heads/", "refs/tags/"],
        &["for-each-ref", "refs/*/main"],
        &["for-each-ref", "refs/hea"],
        &["for-each-ref", "refs/heads/explore/0.7.1*", "refs/heads/*0.7.1*"],
        &["for-each-ref", "--format=%(refname)", "refs/notes/brit/build/"],
        &["for-each-ref", "--format=%(refname:short)"],
        &[
            "for-each-ref",
            "--format=%(refname:short) %(objectname:short) %(committerdate:short) [%(subject)] %(objecttype) %% %41",
        ],
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname) %(committerdate:iso) %(committerdate)",
        ],
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname) %(committerdate:iso)",
        ],
        &["for-each-ref", "--sort=committerdate", "--format=%(refname)"],
        &["for-each-ref", "--sort=-refname"],
        &[
            "for-each-ref",
            "--sort=refname",
            "--sort=-committerdate",
            "--format=%(refname)",
            "refs/heads",
        ],
    ];
    for form in forms {
        assert_same(root, form)?;
    }

    let refused = brit(root, &["for-each-ref", "--format=%(refname) %(authorname)"])?;
    assert_ne!(
        refused.code,
        Some(0),
        "an unimplemented atom must be refused: {refused:?}"
    );
    assert!(
        refused.stderr.contains("%(authorname)"),
        "the refusal names the atom: {refused:?}"
    );
    assert!(
        refused.stdout.is_empty(),
        "nothing is listed before the refusal: {refused:?}"
    );

    assert_help_names("for-each-ref", "brit branch list")
}
