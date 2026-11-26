//! Native single-commit picks, compared with Git and checked at the public CLI.
#![cfg(target_os = "linux")]

use std::{fs, path::Path};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};
use gix_testtools::TestResult;

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
    pick_args(root, &[revision])
}

fn pick_args(root: &Path, args: &[&str]) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("required binary"))
        .current_dir(root)
        .args(std::iter::once("cherry-pick").chain(args.iter().copied()))
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

fn sequence_fixture(conflict: bool) -> TestResult<TestRepo> {
    let repo = TestRepo::new("cherry-pick-sequence")?;
    let root = repo.path();
    repo.commit_file("selected", "base\n")?;
    repo.commit_file("neighbor", "base neighbor\n")?;
    git(root, &["branch", "base"])?;
    git(root, &["switch", "-c", "source"])?;
    for (path, message) in [
        (
            "first",
            "First

Change-Id: I1111
",
        ),
        (
            "selected",
            "Second

Co-Authored-By: A Person <a@example.org>
",
        ),
        (
            "third",
            "Third

Session-Id: 0123
",
        ),
    ] {
        fs::write(root.join(path), b"source\n")?;
        git(root, &["add", path])?;
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
    }
    git(root, &["switch", "main"])?;
    repo.commit_file("destination", "destination change\n")?;
    if conflict {
        repo.commit_file("selected", "destination\n")?;
    }
    Ok(repo)
}

#[derive(Debug, PartialEq, Eq)]
struct ReplayedCommit {
    tree: Vec<u8>,
    author_and_message: Vec<u8>,
}

fn replayed(root: &Path, count: usize) -> TestResult<Vec<ReplayedCommit>> {
    (0..count)
        .rev()
        .map(|offset| {
            let raw = git(root, &["cat-file", "commit", &format!("HEAD~{offset}")])?;
            let mut parts = raw.splitn(2, |byte| *byte == b'\n');
            let tree = parts.next().expect("commit has a tree header").to_vec();
            let body = parts.next().expect("commit has headers and a message");
            let separator = body
                .windows(2)
                .position(|bytes| bytes == b"\n\n")
                .expect("message separator");
            let author = body[..separator]
                .split(|byte| *byte == b'\n')
                .find(|line| line.starts_with(b"author "))
                .expect("commit has an author");
            Ok(ReplayedCommit {
                tree,
                author_and_message: [author, &body[separator..]].concat(),
            })
        })
        .collect()
}

#[test]
fn range_replays_oldest_first_with_git_metadata_and_tree_parity() -> TestResult {
    let repo = sequence_fixture(false)?;
    let root = repo.path();
    git(root, &["switch", "-c", "git-result"])?;
    git(root, &["cherry-pick", "base..source"])?;
    let expected = replayed(root, 3)?;
    git(root, &["switch", "main"])?;
    pick_success(root, "base..source")?;
    assert_eq!(
        replayed(root, 3)?,
        expected,
        "each pick matches Git's tree, author and message"
    );
    Ok(())
}

#[test]
fn explicit_list_replays_in_argument_order() -> TestResult {
    let repo = sequence_fixture(false)?;
    let root = repo.path();
    git(root, &["switch", "-c", "git-result"])?;
    git(root, &["cherry-pick", "source", "source~2"])?;
    let expected = replayed(root, 2)?;
    git(root, &["switch", "main"])?;
    let output = pick_args(root, &["source", "source~2"])?;
    assert!(output.status.success(), "{}", output.stderr);
    assert_eq!(replayed(root, 2)?, expected, "the list is applied in argument order");
    Ok(())
}

fn expect_sequence_conflict(root: &Path) -> TestResult {
    let before = git(root, &["rev-parse", "HEAD"])?;
    let commit = git(root, &["rev-parse", "source~1"])?;
    let output = pick(root, "base..source")?;
    assert!(!output.status.success(), "the second pick conflicts");
    assert!(
        output.stderr.contains("selected"),
        "conflicting path: {}",
        output.stderr
    );
    assert!(
        output.stderr.contains(&String::from_utf8_lossy(&commit)[..7]),
        "conflicting commit: {}",
        output.stderr
    );
    assert_eq!(
        git(root, &["rev-parse", "HEAD^"])?,
        before,
        "the first pick advanced HEAD"
    );
    assert_eq!(
        git(root, &["ls-files", "-u", "selected"])?
            .split(|b| *b == b'\n')
            .count(),
        4,
        "the index has all three conflict stages"
    );
    assert_eq!(
        git(root, &["symbolic-ref", "HEAD"])?,
        b"refs/heads/main\n",
        "the destination branch stays checked out during the conflict"
    );
    assert!(!root.join("third").exists(), "the third pick is still pending");
    let status = pick(root, "--status")?;
    assert!(
        status.status.success() && status.stdout.contains("cherry-pick"),
        "stdout: {}; stderr: {}",
        status.stdout,
        status.stderr
    );
    Ok(())
}

fn stage_resolution(root: &Path) -> TestResult {
    let add = BritInvocation::new(brit_bin().expect("required binary"))
        .current_dir(root)
        .args(["add", "selected"])
        .run()?;
    assert!(add.status.success(), "{}", add.stderr);
    Ok(())
}

#[test]
fn sequence_conflict_continue_matches_git() -> TestResult {
    let repo = sequence_fixture(true)?;
    let root = repo.path();
    git(root, &["switch", "-c", "git-result"])?;
    let conflict = gix_testtools::git_command(root)
        .args(["cherry-pick", "base..source"])
        .output()?;
    assert!(!conflict.status.success(), "Git stops on the second pick");
    fs::write(root.join("selected"), b"resolved\n")?;
    git(root, &["add", "selected"])?;
    git(root, &["-c", "core.editor=true", "cherry-pick", "--continue"])?;
    let expected = replayed(root, 3)?;
    git(root, &["switch", "main"])?;
    expect_sequence_conflict(root)?;
    fs::write(root.join("selected"), b"resolved\n")?;
    stage_resolution(root)?;
    pick_success(root, "--continue")?;
    assert_eq!(replayed(root, 3)?, expected, "continued picks match Git");
    assert_eq!(
        git(root, &["status", "--porcelain"])?,
        b"",
        "checkout is clean after continuing"
    );
    Ok(())
}

#[test]
fn sequence_conflict_abort_restores_original_bytes() -> TestResult {
    let repo = sequence_fixture(true)?;
    let root = repo.path();
    git(root, &["pack-refs", "--all"])?;
    fs::write(root.join("neighbor"), b"staged neighbor\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged neighbor\n")?;
    let before = live(root)?;
    let log_before = fs::read(root.join(".git/logs/refs/heads/main"))?;
    assert!(
        !root.join(".git/refs/heads/main").exists(),
        "the starting branch is packed"
    );
    expect_sequence_conflict(root)?;
    fs::write(root.join("selected"), b"resolved\n")?;
    stage_resolution(root)?;
    pick_success(root, "--abort")?;
    assert_eq!(live(root)?, before, "abort restores HEAD, index and tracked file bytes");
    assert_eq!(
        fs::read(root.join(".git/logs/refs/heads/main"))?,
        log_before,
        "branch log is restored"
    );
    assert!(
        !root.join(".git/refs/heads/main").exists(),
        "abort restores the packed branch"
    );
    assert!(!root.join("first").exists(), "abort removes the first pick's file");
    assert!(!root.join("third").exists(), "the pending pick stays unapplied");
    Ok(())
}

#[test]
fn empty_range_names_the_range_and_changes_nothing() -> TestResult {
    let repo = sequence_fixture(false)?;
    let root = repo.path();
    let before = live(root)?;
    let range = "source..base";
    let output = pick(root, range)?;
    assert!(!output.status.success(), "the empty range is refused");
    assert!(
        output.stderr.contains(range) && output.stderr.contains("empty"),
        "{}",
        output.stderr
    );
    assert_eq!(live(root)?, before, "an empty range changes nothing");
    Ok(())
}

#[test]
fn range_refuses_a_merge_before_applying_any_input() -> TestResult {
    let repo = sequence_fixture(false)?;
    let root = repo.path();
    git(root, &["switch", "-c", "side", "base"])?;
    repo.commit_file("side", "side change\n")?;
    git(root, &["switch", "source"])?;
    git(root, &["merge", "--no-edit", "side"])?;
    repo.commit_file("after-merge", "later change\n")?;
    git(root, &["switch", "main"])?;
    let before = live(root)?;
    let merge = git(root, &["rev-parse", "source^"])?;
    let output = pick(root, "base..source")?;
    assert!(!output.status.success(), "a selected merge requires a mainline");
    assert!(
        output.stderr.contains("mainline") && output.stderr.contains(&String::from_utf8_lossy(&merge)[..7]),
        "{}",
        output.stderr
    );
    assert_eq!(live(root)?, before, "the merge refusal leaves the original bytes");
    assert!(!root.join("first").exists(), "earlier inputs were never applied");
    Ok(())
}

#[test]
fn sequence_conflict_skip_applies_remaining_picks() -> TestResult {
    let repo = sequence_fixture(true)?;
    let root = repo.path();
    git(root, &["switch", "-c", "git-result"])?;
    let conflict = gix_testtools::git_command(root)
        .args(["cherry-pick", "base..source"])
        .output()?;
    assert!(!conflict.status.success(), "Git stops on the second pick");
    git(root, &["cherry-pick", "--skip"])?;
    let expected = replayed(root, 2)?;
    git(root, &["switch", "main"])?;
    expect_sequence_conflict(root)?;
    pick_success(root, "--skip")?;
    assert_eq!(replayed(root, 2)?, expected, "skipping a pick matches Git");
    Ok(())
}

#[test]
fn range_with_unrelated_exclusion_includes_the_root() -> TestResult {
    let repo = TestRepo::new("cherry-pick-unrelated")?;
    let root = repo.path();
    repo.commit_file("destination", "destination\n")?;
    git(root, &["switch", "--orphan", "source"])?;
    repo.commit_file("first", "first source commit\n")?;
    repo.commit_file("second", "second source commit\n")?;
    git(root, &["switch", "main"])?;
    git(root, &["switch", "-c", "git-result"])?;
    git(root, &["cherry-pick", "main..source"])?;
    let expected = replayed(root, 2)?;
    git(root, &["switch", "main"])?;
    pick_success(root, "main..source")?;
    assert_eq!(replayed(root, 2)?, expected, "the unrelated range includes its root");
    Ok(())
}
