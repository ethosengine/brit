//! Daily-driving remote queries and fetches use Brit's native command grammar.
use std::{path::Path, process::Command};

use gix_testtools::{git, git_command, tempfile, TestResult};

fn brit(root: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command
        .current_dir(root)
        .args(args)
        .output()
        .expect("run isolated Brit")
}

#[test]
fn remote_refs_and_fetch_refresh_the_configured_tracking_branch() -> TestResult {
    let remote = tempfile::tempdir()?;
    let local = tempfile::tempdir()?;
    git(remote.path(), "init")?;
    git(remote.path(), "commit --allow-empty -m first")?;
    git(local.path(), "init")?;
    let output = git_command(local.path())
        .args(["remote", "add", "origin"])
        .arg(remote.path())
        .output()?;
    assert!(output.status.success(), "configure local remote: {output:?}");

    for message in ["second", "third"] {
        git(remote.path(), &format!("commit --allow-empty -m {message}"))?;
        let expected = git(remote.path(), "rev-parse HEAD")?;
        let refs = brit(local.path(), &["remote", "-n", "origin", "refs"]);
        assert!(refs.status.success(), "advertisement: {refs:?}");
        let advertisement = String::from_utf8(refs.stdout)?;
        assert!(
            advertisement
                .lines()
                .any(|line| { line.starts_with(expected.trim()) && line.ends_with("refs/heads/main") }),
            "remote head must be visible: {advertisement}"
        );
        let output = brit(local.path(), &["fetch", "-r", "origin"]);
        assert!(output.status.success(), "configured fetch: {output:?}");
        assert_eq!(git(local.path(), "rev-parse refs/remotes/origin/main")?, expected);
    }
    git(remote.path(), "commit --allow-empty -m fourth")?;
    let expected = git(remote.path(), "rev-parse HEAD")?;
    let output = brit(
        local.path(),
        &["fetch", "-r", "origin", "+refs/heads/main:refs/remotes/origin/main"],
    );
    assert!(output.status.success(), "explicit mapping: {output:?}");
    assert_eq!(git(local.path(), "rev-parse refs/remotes/origin/main")?, expected);
    Ok(())
}

/// A remote with `main` and `dev`, and a local repository whose `origin` tracks both.
fn two_branch_fixture() -> gix_testtools::Result<(tempfile::TempDir, tempfile::TempDir)> {
    let remote = tempfile::tempdir()?;
    let local = tempfile::tempdir()?;
    git(remote.path(), "init")?;
    git(remote.path(), "commit --allow-empty -m first")?;
    git(remote.path(), "branch dev")?;
    git(local.path(), "init")?;
    let output = git_command(local.path())
        .args(["remote", "add", "origin"])
        .arg(remote.path())
        .output()?;
    assert!(output.status.success(), "configure local remote: {output:?}");
    let output = brit(local.path(), &["fetch", "-r", "origin"]);
    assert!(output.status.success(), "initial configured fetch: {output:?}");
    Ok((remote, local))
}

fn advance(remote: &Path, branch: &str, message: &str) -> gix_testtools::Result<String> {
    git(remote, &format!("switch -q {branch}"))?;
    git(remote, &format!("commit --allow-empty -m {message}"))?;
    git(remote, "rev-parse HEAD")
}

fn rev(repo: &Path, name: &str) -> Option<String> {
    git(repo, &format!("rev-parse --verify -q {name}")).ok()
}

/// Git (since 1.8.4) updates `refs/remotes/<remote>/<branch>` when a fetch names a branch
/// that the remote's configured fetch refspec maps there, even though the command-line
/// refspec has no destination. Other tracking refs stay where they were.
#[test]
fn branch_fetch_updates_the_configured_tracking_ref_opportunistically() -> TestResult {
    let (remote, local) = two_branch_fixture()?;
    let main_before = git(local.path(), "rev-parse refs/remotes/origin/main")?;
    advance(remote.path(), "main", "main-moves")?;

    for (round, args) in [
        &["fetch", "-r", "origin", "dev"][..],
        &["fetch", "origin", "dev"][..],
        &["fetch", "-r", "origin", "refs/heads/dev"][..],
    ]
    .into_iter()
    .enumerate()
    {
        let expected = advance(remote.path(), "dev", &format!("dev-{round}"))?;
        let output = brit(local.path(), args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert_eq!(
            rev(local.path(), "refs/remotes/origin/dev"),
            Some(expected),
            "{args:?} must advance the tracking ref: {output:?}"
        );
        assert_eq!(
            rev(local.path(), "refs/remotes/origin/main"),
            Some(main_before.clone()),
            "{args:?} must not fetch branches it did not name"
        );
    }
    Ok(())
}

/// An explicit destination keeps its meaning, and a remote whose configured refspecs do not
/// map the branch (or a bare URL, which has none) gains no tracking ref.
#[test]
fn branch_fetch_without_a_configured_mapping_creates_no_tracking_ref() -> TestResult {
    let (remote, local) = two_branch_fixture()?;
    let dev_before = git(local.path(), "rev-parse refs/remotes/origin/dev")?;
    let expected = advance(remote.path(), "dev", "dev-moves")?;

    let output = brit(local.path(), &["fetch", "-r", "origin", "dev:refs/heads/copy"]);
    assert!(output.status.success(), "explicit destination: {output:?}");
    assert_eq!(rev(local.path(), "refs/heads/copy"), Some(expected.clone()));
    assert_eq!(
        rev(local.path(), "refs/remotes/origin/dev"),
        Some(dev_before.clone()),
        "an explicit destination is the only ref written"
    );

    let url = remote.path().to_str().expect("utf-8 temporary path");
    let output = brit(local.path(), &["fetch", "-r", url, "dev"]);
    assert!(output.status.success(), "URL fetch: {output:?}");

    let output = git_command(local.path())
        .args(["remote", "add", "narrow"])
        .arg(remote.path())
        .output()?;
    assert!(output.status.success(), "configure narrow remote: {output:?}");
    git(
        local.path(),
        "config remote.narrow.fetch +refs/heads/main:refs/remotes/narrow/main",
    )?;
    let output = brit(local.path(), &["fetch", "-r", "narrow", "dev"]);
    assert!(output.status.success(), "narrow remote fetch: {output:?}");
    let refs = git(local.path(), "for-each-ref --format=%(refname) refs/remotes")?;
    assert_eq!(
        refs.lines().collect::<Vec<_>>(),
        ["refs/remotes/origin/dev", "refs/remotes/origin/main"],
        "no tracking ref without a configured mapping"
    );
    assert_eq!(
        rev(local.path(), "refs/remotes/origin/dev"),
        Some(dev_before),
        "a URL or unmapped fetch leaves origin's tracking ref where it was"
    );
    Ok(())
}

fn add_remote(local: &Path, name: &str, url: &Path) -> TestResult {
    let output = git_command(local).args(["remote", "add", name]).arg(url).output()?;
    assert!(output.status.success(), "configure {name}: {output:?}");
    Ok(())
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// `brit fetch <remote> <refspec>` is Git's spelling: a first positional naming a configured
/// remote selects it. With `-r`, every positional is a refspec, and a refspec naming a ref
/// the remote does not advertise is an error, never a silent success.
#[test]
fn positional_remote_selects_the_named_remote_and_unknown_refs_refuse() -> TestResult {
    let (_origin, local) = two_branch_fixture()?;
    let upstream = tempfile::tempdir()?;
    git(upstream.path(), "init")?;
    git(upstream.path(), "commit --allow-empty -m upstream-first")?;
    git(upstream.path(), "branch dev")?;
    add_remote(local.path(), "upstream", upstream.path())?;
    let origin_dev = rev(local.path(), "refs/remotes/origin/dev");
    let expected = advance(upstream.path(), "dev", "upstream-dev")?;

    let output = brit(local.path(), &["fetch", "upstream", "dev"]);
    assert!(output.status.success(), "positional remote: {output:?}");
    assert_eq!(rev(local.path(), "refs/remotes/upstream/dev"), Some(expected));
    assert_eq!(
        rev(local.path(), "refs/remotes/origin/dev"),
        origin_dev,
        "origin untouched"
    );

    for (args, name) in [
        (&["fetch", "-r", "origin", "upstream"][..], "upstream"),
        (&["fetch", "-r", "origin", "nosuch"][..], "nosuch"),
        (&["fetch", "origin", "nosuch:refs/heads/nosuch"][..], "nosuch"),
    ] {
        let output = brit(local.path(), args);
        assert!(!output.status.success(), "{args:?} must refuse: {output:?}");
        assert!(
            stderr(&output).contains("couldn't find remote ref") && stderr(&output).contains(name),
            "{args:?} names the missing ref: {}",
            stderr(&output)
        );
    }
    assert_eq!(rev(local.path(), "refs/heads/nosuch"), None);
    Ok(())
}

/// A name that is both a tag and a branch resolves as Git resolves it, to one best match
/// (tags come first), so the branch's tracking ref moves exactly when Git moves it.
#[test]
fn ambiguous_short_name_resolves_like_git() -> TestResult {
    let (remote, brit_local) = two_branch_fixture()?;
    let git_local = tempfile::tempdir()?;
    git(git_local.path(), "init")?;
    add_remote(git_local.path(), "origin", remote.path())?;
    git(remote.path(), "tag dup")?;
    git(remote.path(), "branch dup")?;
    for local in [brit_local.path(), git_local.path()] {
        let output = brit(local, &["fetch", "-r", "origin"]);
        assert!(output.status.success(), "seed tracking refs: {output:?}");
    }
    let moved = advance(remote.path(), "dup", "branch-moves")?;

    let output = brit(brit_local.path(), &["fetch", "origin", "dup"]);
    assert!(output.status.success(), "brit: {output:?}");
    git(git_local.path(), "fetch -q origin dup")?;
    assert_eq!(
        rev(brit_local.path(), "refs/remotes/origin/dup"),
        rev(git_local.path(), "refs/remotes/origin/dup"),
        "origin/dup moves exactly as Git moves it"
    );
    assert_ne!(
        rev(git_local.path(), "refs/remotes/origin/dup"),
        Some(moved),
        "Git resolves dup to the tag and leaves the branch's tracking ref alone"
    );
    Ok(())
}

/// A non-fast-forward update the refspec does not force is rejected, the ref is named, and
/// the fetch fails, for the configured, the implicit and the explicit forms alike.
#[test]
fn rejected_updates_fail_the_fetch_and_name_the_ref() -> TestResult {
    let (remote, local) = two_branch_fixture()?;
    git(
        local.path(),
        "config remote.origin.fetch refs/heads/*:refs/remotes/origin/*",
    )?;
    advance(remote.path(), "dev", "dev-moves")?;
    let output = brit(local.path(), &["fetch", "-r", "origin"]);
    assert!(output.status.success(), "fast-forward is accepted: {output:?}");
    let before = rev(local.path(), "refs/remotes/origin/dev");
    git(remote.path(), "switch -q dev")?;
    git(remote.path(), "reset -q --hard HEAD~1")?;
    git(remote.path(), "commit --allow-empty -m rewritten")?;

    for args in [
        &["fetch", "-r", "origin"][..],
        &["fetch", "-r", "origin", "dev"][..],
        &["fetch", "-r", "origin", "dev:refs/remotes/origin/dev"][..],
    ] {
        let output = brit(local.path(), args);
        assert!(!output.status.success(), "{args:?} must fail: {output:?}");
        assert!(
            stderr(&output).contains("refs/remotes/origin/dev") && stderr(&output).contains("non-fast-forward"),
            "{args:?} names the rejected ref: {}",
            stderr(&output)
        );
        assert_eq!(rev(local.path(), "refs/remotes/origin/dev"), before, "{args:?}");
    }
    Ok(())
}

/// Every configured refspec that maps the branch is updated, and a configured negative
/// refspec excluding it leaves its tracking ref alone (conservative, pinned).
#[test]
fn several_configured_mappings_and_negations() -> TestResult {
    let (remote, local) = two_branch_fixture()?;
    git(
        local.path(),
        "config --add remote.origin.fetch +refs/heads/dev:refs/remotes/mirror/dev",
    )?;
    let expected = advance(remote.path(), "dev", "dev-moves")?;
    let output = brit(local.path(), &["fetch", "-r", "origin", "dev"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(rev(local.path(), "refs/remotes/origin/dev"), Some(expected.clone()));
    assert_eq!(rev(local.path(), "refs/remotes/mirror/dev"), Some(expected.clone()));

    let output = brit(local.path(), &["remote", "-n", "origin", "ref-map", "-u"]);
    assert!(
        output.status.success(),
        "ref-map with two mappings of one tip: {output:?}"
    );

    git(local.path(), "config --unset-all remote.origin.fetch")?;
    git(
        local.path(),
        "config --add remote.origin.fetch +refs/heads/*:refs/remotes/origin/*",
    )?;
    git(local.path(), "config --add remote.origin.fetch ^refs/heads/dev")?;
    advance(remote.path(), "dev", "dev-moves-again")?;
    let output = brit(local.path(), &["fetch", "-r", "origin", "dev"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        rev(local.path(), "refs/remotes/origin/dev"),
        Some(expected),
        "a negated branch keeps its tracking ref"
    );
    Ok(())
}
