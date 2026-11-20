//! `brit ls-remote` prints what `git ls-remote` prints, `<oid>\t<refname>` in server order with
//! peeled tags, for the remote and pattern forms agents use, and leads to `brit remote refs`.
use anyhow::Result;
#[cfg(unix)]
use cli_journey::support::{
    mock_remote::MockRemote,
    parity::{assert_help_names, assert_same, assert_same_streams},
};
use cli_journey::support::{
    parity::{brit, git, git_ok},
    test_repo::TestRepo,
};

/// Unix only: it compares the `From <url>` line for a `file://` URL built from a filesystem
/// path byte for byte, and on Windows (`file://C:\…`) it is not established that the URL's
/// password-redacting display reproduces Git's spelling.
#[cfg(unix)]
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

    // `-h` lists heads as in Git, the omitted remote is named on stderr, and --exit-code reports no match.
    for form in [
        &["ls-remote", "-h", "origin"][..],
        &["ls-remote", "-h", "origin", "dev"],
        &["ls-remote", "-b", "origin"],
        &["ls-remote"],
        &["ls-remote", "-q"],
        &["ls-remote", "--quiet", "--heads"],
        &["ls-remote", "origin"],
        &["ls-remote", "--exit-code", "origin", "nothing-matches"],
        &["ls-remote", "--exit-code", "origin", "dev"],
        &["ls-remote", "--exit-code", "--tags", "origin", "refs/heads/*"],
    ] {
        assert_same_streams(root, form)?;
    }
    let from = assert_same_streams(root, &["ls-remote", "--refs"])?;
    assert_eq!(
        from.stderr,
        format!("From {url}\n"),
        "the default remote's URL: {from:?}"
    );

    assert_help_names("ls-remote", "brit remote refs")
}

/// The `From <url>` line appears only once the default remote answered, and never shows a password.
#[test]
fn ls_remote_names_the_default_remote_only_after_it_answered_and_without_its_password() -> Result<()> {
    let local = TestRepo::new("ls-remote-unreachable")?;
    let root = local.path();
    let missing = root.join("no-such-remote.git");
    git_ok(
        root,
        &["remote", "add", "origin", missing.to_str().expect("UTF-8 path")],
    )?;
    let git_out = git(root, &["ls-remote"])?;
    let out = brit(root, &["ls-remote"])?;
    assert_ne!(git_out.code, Some(0), "{git_out:?}");
    assert_ne!(out.code, Some(0), "an unreachable remote fails: {out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(
        !git_out.stderr.contains("From "),
        "Git names no remote it could not reach: {git_out:?}"
    );
    assert!(!out.stderr.contains("From "), "nor does Brit: {out:?}");

    // Nothing listens on port 1, so the connection is refused at once.
    git_ok(
        root,
        &["remote", "set-url", "origin", "http://user:secret@127.0.0.1:1/repo.git"],
    )?;
    let out = brit(root, &["ls-remote"])?;
    assert_ne!(out.code, Some(0), "{out:?}");
    assert!(
        !out.stderr.contains("secret"),
        "the password never reaches stderr: {out:?}"
    );
    assert!(!out.stderr.contains("From "), "{out:?}");
    Ok(())
}
