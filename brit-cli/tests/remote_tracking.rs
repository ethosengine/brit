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
