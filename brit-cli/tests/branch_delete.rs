//! A developer deletes or renames a branch with Git's safety semantics.
use std::{path::Path, process::Command};

use gix_testtools::{git, git_command, tempfile, TestResult};

fn brit(root: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command
        .current_dir(root)
        .args(args)
        .output()
        .expect("run Brit in disposable repository")
}

/// Run stock Git expecting a refusal, so it can act as the oracle for a state it must not change.
fn git_refuses(root: &Path, args: &[&str]) -> TestResult {
    let output = git_command(root).args(args).output()?;
    assert!(!output.status.success(), "stock git must refuse {args:?}: {output:?}");
    Ok(())
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn has_ref(root: &Path, name: &str) -> gix_testtools::Result<bool> {
    Ok(!git(root, &format!("for-each-ref {name}"))?.trim().is_empty())
}

fn branch_config(root: &Path, name: &str) -> gix_testtools::Result<String> {
    let output = git_command(root)
        .args(["config", "--local", "--get-regexp", &format!("^branch\\.{name}\\.")])
        .output()?;
    Ok(String::from_utf8(output.stdout)?)
}

/// `main` with one commit, and `topic` one commit ahead of it (unmerged).
fn fixture(root: &Path) -> gix_testtools::Result<String> {
    git(root, "init")?;
    git(root, "commit --allow-empty -m base")?;
    let main = git(root, "symbolic-ref --short HEAD")?.trim().to_owned();
    git(root, "checkout -q -b topic")?;
    git(root, "commit --allow-empty -m topic")?;
    git(root, &format!("checkout -q {main}"))?;
    Ok(main)
}

#[test]
fn delete_merged_branch_removes_ref_reflog_and_config() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    git(root, "branch merged")?;
    git(root, "config branch.merged.description kept-out")?;
    git(root, "config branch.merged.rebase true")?;
    git(root, "config branch.topic.description unrelated")?;
    git(root, "config user.useConfigOnly false")?;
    assert!(root.join(".git/logs/refs/heads/merged").is_file());
    let index = std::fs::read(root.join(".git/index"))?;
    let head = std::fs::read(root.join(".git/HEAD"))?;
    let tip = git(root, "rev-parse --short merged")?;

    let output = brit(root, &["branch", "--delete", "merged"]);
    assert!(output.status.success(), "delete merged: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!("Deleted branch merged (was {}).", tip.trim())),
        "{output:?}"
    );
    assert!(!has_ref(root, "refs/heads/merged")?);
    assert!(!root.join(".git/logs/refs/heads/merged").exists(), "reflog removed");
    assert_eq!(branch_config(root, "merged")?, "", "branch section removed");
    assert_eq!(branch_config(root, "topic")?, "branch.topic.description unrelated\n");
    assert_eq!(git(root, "config --local user.useConfigOnly")?.trim(), "false");
    assert_eq!(std::fs::read(root.join(".git/index"))?, index);
    assert_eq!(std::fs::read(root.join(".git/HEAD"))?, head);
    git(root, "fsck --no-progress")?;
    Ok(())
}

#[test]
fn delete_short_flag_handles_packed_refs() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    git(root, "branch packed")?;
    git(root, "pack-refs --all")?;
    assert!(!root.join(".git/refs/heads/packed").exists());
    let output = brit(root, &["branch", "-d", "packed"]);
    assert!(output.status.success(), "delete packed: {output:?}");
    assert!(!has_ref(root, "refs/heads/packed")?);
    assert!(has_ref(root, "refs/heads/topic")?, "sibling packed refs survive");
    Ok(())
}

#[test]
fn delete_refuses_unmerged_branch_unless_forced() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    let tip = git(root, "rev-parse topic")?;
    git_refuses(root, &["branch", "-d", "topic"])?;
    let output = brit(root, &["branch", "-d", "topic"]);
    assert!(!output.status.success(), "must refuse unmerged: {output:?}");
    assert!(stderr(&output).contains("not fully merged"), "{output:?}");
    assert!(stderr(&output).contains("brit branch -D topic"), "{output:?}");
    assert_eq!(git(root, "rev-parse topic")?, tip, "refused branch untouched");
    assert!(root.join(".git/logs/refs/heads/topic").is_file());

    git(root, "branch -f other topic")?;
    let output = brit(root, &["branch", "-D", "topic"]);
    assert!(output.status.success(), "force delete: {output:?}");
    assert!(!has_ref(root, "refs/heads/topic")?);
    let output = brit(root, &["branch", "--delete", "--force", "other"]);
    assert!(output.status.success(), "long force delete: {output:?}");
    assert!(!has_ref(root, "refs/heads/other")?);
    Ok(())
}

#[test]
fn delete_measures_merged_against_upstream_before_head() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    let main = fixture(root)?;
    let topic = git(root, "rev-parse topic")?.trim().to_owned();
    let base = git(root, "rev-parse HEAD")?.trim().to_owned();
    git(root, "config remote.origin.url /nonexistent")?;
    git(root, "config remote.origin.fetch +refs/heads/*:refs/remotes/origin/*")?;
    for name in ["topic", "behind"] {
        git(root, &format!("config branch.{name}.remote origin"))?;
        git(root, &format!("config branch.{name}.merge refs/heads/{name}"))?;
    }
    // `topic` is unmerged into HEAD but fully contained in its upstream: Git deletes with a warning.
    git(root, &format!("update-ref refs/remotes/origin/topic {topic}"))?;
    let output = brit(root, &["branch", "-d", "topic"]);
    assert!(output.status.success(), "merged to upstream: {output:?}");
    assert!(stderr(&output).contains("not yet merged to HEAD"), "{output:?}");
    assert!(!has_ref(root, "refs/heads/topic")?);

    // `behind` is merged into HEAD but ahead of its upstream: Git refuses, measuring the upstream.
    git(root, &format!("checkout -q -b behind {main}"))?;
    git(root, "commit --allow-empty -m ahead")?;
    git(root, &format!("checkout -q {main}"))?;
    git(root, "merge -q --ff-only behind")?;
    git(root, &format!("update-ref refs/remotes/origin/behind {base}"))?;
    git_refuses(root, &["branch", "-d", "behind"])?;
    let output = brit(root, &["branch", "-d", "behind"]);
    assert!(!output.status.success(), "unmerged to upstream: {output:?}");
    assert!(stderr(&output).contains("not fully merged"), "{output:?}");
    assert!(has_ref(root, "refs/heads/behind")?);
    Ok(())
}

#[test]
fn delete_refuses_branch_checked_out_in_any_worktree_even_when_forced() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    let main = fixture(root)?;
    git(root, "worktree add -q -b side linked")?;
    let linked = root.join("linked");
    for args in [["branch", "-d", "side"], ["branch", "-D", "side"]] {
        git_refuses(root, &args)?;
        let output = brit(root, &args);
        assert!(!output.status.success(), "must refuse {args:?}: {output:?}");
        let message = stderr(&output);
        assert!(
            message.contains("cannot delete branch 'side' used by worktree at"),
            "{output:?}"
        );
        assert!(message.contains("linked"), "names the worktree: {output:?}");
        assert!(has_ref(root, "refs/heads/side")?);
    }
    let output = brit(&linked, &["branch", "-D", &main]);
    assert!(!output.status.success(), "main checkout's branch: {output:?}");
    assert!(stderr(&output).contains(&format!("cannot delete branch '{main}' used by worktree at")));
    assert!(has_ref(root, &format!("refs/heads/{main}"))?);

    let output = brit(root, &["branch", "-d", "missing"]);
    assert!(!output.status.success(), "missing branch: {output:?}");
    assert!(stderr(&output).contains("branch 'missing' not found"), "{output:?}");

    let output = brit(&linked, &["branch", "-d", "topic"]);
    assert!(
        !output.status.success(),
        "unmerged relative to the linked HEAD: {output:?}"
    );
    git(&linked, "merge -q --ff-only topic")?;
    let output = brit(&linked, &["branch", "-d", "topic"]);
    assert!(output.status.success(), "merged into the linked HEAD: {output:?}");
    assert!(!has_ref(root, "refs/heads/topic")?);
    Ok(())
}

#[test]
fn delete_of_several_names_is_all_or_nothing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    git(root, "branch one")?;
    git(root, "branch two")?;
    let output = brit(root, &["branch", "-d", "one", "topic", "two"]);
    assert!(!output.status.success(), "one refusal refuses all: {output:?}");
    for name in ["one", "topic", "two"] {
        assert!(has_ref(root, &format!("refs/heads/{name}"))?, "{name} kept");
    }
    let output = brit(root, &["branch", "-d", "one", "two"]);
    assert!(output.status.success(), "{output:?}");
    assert!(!has_ref(root, "refs/heads/one")? && !has_ref(root, "refs/heads/two")?);
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn deletion_obeys_shared_guard() -> TestResult {
    use std::{io::Read, process::Stdio};
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    fixture(root)?;
    git(root, "branch merged")?;
    let mut command = Command::new("flock");
    gix_testtools::configure_git_environment(&mut command, root);
    let mut holder = command
        .arg(root.join(".git/brit-mutation.guard"))
        .args(["sh", "-c", "printf ready; cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut ready = [0; 5];
    holder.stdout.take().expect("piped stdout").read_exact(&mut ready)?;
    for args in [
        vec!["branch", "-d", "merged"],
        vec!["branch", "--move", "merged", "renamed"],
    ] {
        let result = brit(root, &args);
        assert!(!result.status.success(), "must refuse active guard: {result:?}");
        assert!(
            stderr(&result).contains("another Brit mutation is active"),
            "{result:?}"
        );
    }
    assert!(has_ref(root, "refs/heads/merged")?);
    drop(holder.stdin.take());
    assert!(holder.wait()?.success());
    Ok(())
}

/// Two byte-identical repositories (fixed dates), so Brit and stock Git can each act on one.
fn twin_fixtures(parent: &Path) -> gix_testtools::Result<(std::path::PathBuf, std::path::PathBuf, String)> {
    let (ours, theirs) = (parent.join("brit"), parent.join("git"));
    let mut main = String::new();
    for root in [&ours, &theirs] {
        std::fs::create_dir(root)?;
        main = fixture(root)?;
        git(root, "config branch.topic.remote .")?;
        git(root, &format!("config branch.topic.merge refs/heads/{main}"))?;
        git(root, "config branch.topic.description carried")?;
    }
    Ok((ours, theirs, main))
}

type RenamedState = (String, Vec<u8>, String, Vec<u8>);

fn renamed_state(root: &Path, name: &str) -> gix_testtools::Result<RenamedState> {
    Ok((
        git(root, "for-each-ref --format=%(refname)%09%(objectname)")?,
        std::fs::read(root.join(format!(".git/logs/refs/heads/{name}")))?,
        git(root, "config --local --list")?,
        std::fs::read(root.join(".git/HEAD"))?,
    ))
}

#[test]
fn move_matches_git_for_ref_reflog_config_and_head() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (ours, theirs, main) = twin_fixtures(dir.path())?;
    for (old, new) in [("topic", "renamed/topic"), (main.as_str(), "trunk")] {
        let output = brit(&ours, &["branch", "--move", old, new]);
        assert!(output.status.success(), "rename {old}: {output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let oracle = git_command(&theirs).args(["branch", "-m", old, new]).output()?;
        assert!(oracle.status.success(), "{oracle:?}");
        assert!(!has_ref(&ours, &format!("refs/heads/{old}"))?);
        assert!(
            !ours.join(format!(".git/logs/refs/heads/{old}")).exists(),
            "old reflog moved"
        );
        assert_eq!(
            renamed_state(&ours, new)?,
            renamed_state(&theirs, new)?,
            "{old} -> {new}: refs, carried reflog, config and HEAD match git"
        );
    }
    assert_eq!(git(&ours, "symbolic-ref HEAD")?.trim(), "refs/heads/trunk");
    assert_eq!(branch_config(&ours, "renamed/topic")?.lines().count(), 3);
    let output = brit(&ours, &["branch", "-m", "trunk", "short"]);
    assert!(output.status.success(), "short flag: {output:?}");
    git(&ours, "fsck --no-progress")?;
    Ok(())
}

#[test]
fn move_refuses_existing_missing_and_invalid_names() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    let main = fixture(root)?;
    let refs = git(root, "for-each-ref")?;
    git_refuses(root, &["branch", "-m", "topic", &main])?;
    for (args, expected) in [
        (vec!["branch", "-m", "topic", main.as_str()], "already exists"),
        (vec!["branch", "-m", "missing", "anything"], "no branch named 'missing'"),
        (vec!["branch", "-m", "topic", "bad..name"], ""),
        (vec!["branch", "-m", "topic", "HEAD"], ""),
    ] {
        let output = brit(root, &args);
        assert!(!output.status.success(), "must refuse {args:?}: {output:?}");
        assert!(stderr(&output).contains(expected), "{output:?}");
        assert_eq!(git(root, "for-each-ref")?, refs, "refusal leaves refs untouched");
    }
    assert!(root.join(".git/logs/refs/heads/topic").is_file());
    Ok(())
}

#[test]
fn move_updates_every_worktree_with_the_branch_checked_out() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    let main = fixture(root)?;
    git(root, "worktree add -q -b side linked")?;
    let linked = root.join("linked");
    let output = brit(root, &["branch", "-m", "side", "moved-side"]);
    assert!(output.status.success(), "rename linked checkout's branch: {output:?}");
    assert_eq!(git(&linked, "symbolic-ref HEAD")?.trim(), "refs/heads/moved-side");
    assert_eq!(git(root, "symbolic-ref HEAD")?.trim(), format!("refs/heads/{main}"));
    let output = brit(&linked, &["branch", "-m", &main, "moved-main"]);
    assert!(
        output.status.success(),
        "rename main checkout's branch from linked: {output:?}"
    );
    assert_eq!(git(root, "symbolic-ref HEAD")?.trim(), "refs/heads/moved-main");
    assert_eq!(git(&linked, "symbolic-ref HEAD")?.trim(), "refs/heads/moved-side");
    git(root, "worktree list")?;
    git(root, "fsck --no-progress")?;
    Ok(())
}
