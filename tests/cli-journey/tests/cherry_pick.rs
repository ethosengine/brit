//! Native single-commit picks, compared with Git and checked at the public CLI.
#![cfg(target_os = "linux")]

use std::{fs, path::Path};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn pick(root: &Path, revision: &str) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("required binary"))
        .current_dir(root)
        .args(["cherry-pick", revision])
        .env("GIT_COMMITTER_NAME", "Current Committer")
        .env("GIT_COMMITTER_EMAIL", "current@example.org")
        .run()
}

fn pick_success(root: &Path, revision: &str) -> Result<()> {
    let output = pick(root, revision)?;
    ensure!(output.status.success(), "cherry-pick {revision}: {}", output.stderr);
    Ok(())
}

fn fixture() -> Result<TestRepo> {
    let repo = TestRepo::new("cherry-pick")?;
    repo.commit_file("selected", "base\n")?;
    repo.commit_file("neighbor", "base neighbor\n")?;
    git(repo.path(), &["switch", "-c", "source"])?;
    repo.commit_file("selected", "picked\n")?;
    git(repo.path(), &["switch", "main"])?;
    repo.commit_file("destination", "destination change\n")?;
    Ok(repo)
}

fn git_result_tree(root: &Path) -> Result<Vec<u8>> {
    git(root, &["switch", "-c", "git-result"])?;
    git(root, &["cherry-pick", "source"])?;
    let tree_id = git(root, &["rev-parse", "HEAD^{tree}"])?;
    git(root, &["switch", "main"])?;
    Ok(tree_id)
}

#[test]
fn file_change_matches_git() -> Result<()> {
    let repo = fixture()?;
    let root = repo.path();
    let expected = git_result_tree(root)?;
    let parent_id = git(root, &["rev-parse", "HEAD"])?;
    pick_success(root, "source")?;
    assert_eq!(git(root, &["rev-parse", "HEAD^{tree}"])?, expected, "Git tree parity");
    assert_eq!(
        git(root, &["rev-parse", "HEAD^"])?,
        parent_id,
        "parent is the prior tip"
    );
    assert_eq!(
        git(root, &["symbolic-ref", "HEAD"])?,
        b"refs/heads/main\n",
        "main stays checked out"
    );
    assert_eq!(fs::read(root.join("selected"))?, b"picked\n", "checkout is updated");
    assert_eq!(git(root, &["write-tree"])?, expected, "index is updated");
    Ok(())
}

#[test]
fn uninitialised_gitlink_pin_move_matches_git() -> Result<()> {
    let repo = TestRepo::new("cherry-pick-gitlink")?;
    let root = repo.path();
    let set_pin = |pin| -> Result<()> {
        git(root, &["update-index", "--add", "--cacheinfo", pin])?;
        git(root, &["commit", "-m", "set pin"])?;
        Ok(())
    };
    set_pin("160000,1111111111111111111111111111111111111111,sub")?;
    git(root, &["switch", "-c", "source"])?;
    set_pin("160000,2222222222222222222222222222222222222222,sub")?;
    git(root, &["switch", "main"])?;
    repo.commit_file("destination", "destination change\n")?;
    let expected = git_result_tree(root)?;
    if root.join("sub").is_dir() {
        fs::remove_dir(root.join("sub"))?;
    }
    pick_success(root, "source")?;
    assert_eq!(git(root, &["rev-parse", "HEAD^{tree}"])?, expected, "Git tree parity");
    assert_eq!(git(root, &["write-tree"])?, expected, "index carries the new pin");
    assert!(!root.join("sub").exists(), "the submodule stays absent");
    Ok(())
}

#[test]
fn preserves_raw_author_message_and_trailers() -> Result<()> {
    let repo = TestRepo::new("cherry-pick-metadata")?;
    let root = repo.path();
    repo.commit_file("selected", "base\n")?;
    git(root, &["switch", "-c", "source"])?;
    fs::write(root.join("selected"), b"picked\n")?;
    git(root, &["add", "selected"])?;
    let message = "Subject

Body with spacing preserved

Change-Id: I0123456789
Co-Authored-By: Other Person <other@example.org>
Session-Id: 0123
Session-Ref: abc

";
    git(
        root,
        &[
            "commit",
            "--cleanup=verbatim",
            "--author=Source Author <source@example.org>",
            "--date=2001-02-03T04:05:06-07:30",
            "-m",
            message,
        ],
    )?;
    git(root, &["switch", "main"])?;
    pick_success(root, "source")?;
    let source = git(root, &["cat-file", "commit", "source"])?;
    let picked = git(root, &["cat-file", "commit", "HEAD"])?;
    let preserved = |commit: &[u8]| {
        let mut in_header = true;
        commit
            .split_inclusive(|byte| *byte == b'\n')
            .filter(|line| {
                if *line == b"\n" {
                    in_header = false;
                }
                !in_header || !(line.starts_with(b"committer ") || line.starts_with(b"parent "))
            })
            .flatten()
            .copied()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        preserved(&picked),
        preserved(&source),
        "source headers and message bytes survive"
    );
    assert!(
        picked
            .split(|byte| *byte == b'\n')
            .any(|line| line.starts_with(b"committer Current Committer <current@example.org> ")),
        "the new commit uses the current committer"
    );
    Ok(())
}

fn live(root: &Path) -> Result<Vec<Vec<u8>>> {
    Ok(vec![
        git(root, &["rev-parse", "HEAD"])?,
        fs::read(root.join(".git/HEAD"))?,
        fs::read(root.join(".git/index"))?,
        fs::read(root.join("selected"))?,
        fs::read(root.join("neighbor"))?,
        fs::read(root.join("destination"))?,
    ])
}

#[test]
fn conflicts_name_each_path_and_leave_live_state_unchanged() -> Result<()> {
    let repo = fixture()?;
    let root = repo.path();
    git(root, &["switch", "source"])?;
    fs::write(root.join("neighbor"), b"picked neighbor\n")?;
    git(root, &["add", "neighbor"])?;
    git(root, &["commit", "--amend", "--no-edit"])?;
    git(root, &["switch", "main"])?;
    repo.commit_file("selected", "conflicting destination\n")?;
    repo.commit_file("neighbor", "conflicting neighbor\n")?;
    let before = live(root)?;
    let output = pick(root, "source")?;
    assert!(!output.status.success(), "conflicts refuse the pick");
    for path in ["selected", "neighbor"] {
        assert!(output.stderr.contains(path), "conflict names {path}: {}", output.stderr);
    }
    assert_eq!(live(root)?, before, "the refusal preserves HEAD, index and checkout");
    assert!(!root.join(".git/CHERRY_PICK_HEAD").exists(), "no continuation state");
    Ok(())
}

#[test]
fn unrelated_dirt_survives_and_overlapping_dirt_refuses() -> Result<()> {
    let repo = fixture()?;
    let root = repo.path();
    fs::write(root.join("neighbor"), b"staged neighbor\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged neighbor\n")?;
    for stage in [false, true] {
        fs::write(root.join("selected"), b"dirty selected\n")?;
        if stage {
            git(root, &["add", "selected"])?;
        }
        let before = live(root)?;
        let output = pick(root, "source")?;
        assert!(!output.status.success(), "overlapping dirt refuses the pick");
        assert!(
            output.stderr.contains("selected"),
            "refusal names the path: {}",
            output.stderr
        );
        assert_eq!(live(root)?, before, "the refusal preserves the live state");
    }
    git(
        root,
        &["restore", "--source=HEAD", "--staged", "--worktree", "selected"],
    )?;
    pick_success(root, "source")?;
    assert_eq!(
        git(root, &["show", "HEAD:neighbor"])?,
        b"base neighbor\n",
        "unrelated staging stays out of the commit"
    );
    assert_eq!(
        git(root, &["show", ":neighbor"])?,
        b"staged neighbor\n",
        "unrelated staging survives"
    );
    assert_eq!(
        fs::read(root.join("neighbor"))?,
        b"unstaged neighbor\n",
        "unrelated checkout edit survives"
    );
    Ok(())
}

#[test]
fn pending_recovery_and_ref_lock_refuse_before_publication() -> Result<()> {
    let repo = fixture()?;
    let root = repo.path();
    let before = live(root)?;
    let pending = root.join(".git/brit-rebase");
    fs::create_dir(&pending)?;
    let output = pick(root, "source")?;
    assert!(
        !output.status.success() && output.stderr.contains("pending replay"),
        "{}",
        output.stderr
    );
    assert_eq!(live(root)?, before, "pending recovery blocks the writer");
    fs::remove_dir(pending)?;

    fs::write(root.join(".git/refs/heads/main.lock"), b"held by another writer")?;
    let output = pick(root, "source")?;
    assert!(!output.status.success(), "a locked branch blocks publication");
    assert_eq!(live(root)?, before, "the branch lock is acquired before live writes");
    assert_eq!(
        fs::read(root.join(".git/refs/heads/main.lock"))?,
        b"held by another writer",
        "foreign lock survives"
    );
    Ok(())
}
