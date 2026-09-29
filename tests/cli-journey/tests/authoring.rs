//! Station 4 prerequisite: the index and repository hooks govern authoring.
use std::{fs, path::Path};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = gix_testtools::git_command(repo).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn brit(repo: &Path, args: &[&str]) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("required binary"))
        .args(args.iter().copied())
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "Author")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_NAME", "Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@example.com")
        .run()
}

fn hook(repo: &Path, name: &str, body: &str) -> Result<()> {
    let dir = repo.join("hooks with spaces");
    fs::create_dir_all(&dir)?;
    git(repo, &["config", "core.hooksPath", "hooks with spaces"])?;
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[test]
fn commit_uses_the_index_preserving_unstaged_bytes_and_gitlinks() -> Result<()> {
    let repo = TestRepo::new("staged-authoring")?;
    repo.commit_file("deleted", "old")?;
    fs::remove_file(repo.path().join("deleted"))?;
    fs::create_dir(repo.path().join("nested"))?;
    fs::write(repo.path().join("nested/file"), "staged")?;
    git(repo.path(), &["add", "deleted", "nested/file"])?;
    let head = repo.head_id()?;
    git(
        repo.path(),
        &["update-index", "--add", "--cacheinfo", "160000", &head, "submodule"],
    )?;
    let expected = git(repo.path(), &["write-tree"])?;
    fs::write(repo.path().join("nested/file"), "unstaged")?;
    let index_before = fs::read(repo.path().join(".git/index"))?;
    let output = brit(repo.path(), &["commit", "-m", "staged only"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD^{tree}"])?, expected);
    assert_eq!(fs::read(repo.path().join(".git/index"))?, index_before);
    assert_eq!(fs::read_to_string(repo.path().join("nested/file"))?, "unstaged");
    git(repo.path(), &["fsck", "--no-dangling"])?;
    Ok(())
}

#[test]
fn allow_empty_does_not_discard_staged_content() -> Result<()> {
    let repo = TestRepo::new("allow-empty-index")?;
    fs::write(repo.path().join("staged"), "keep me")?;
    git(repo.path(), &["add", "staged"])?;
    let expected = git(repo.path(), &["write-tree"])?;
    let output = brit(repo.path(), &["commit", "--allow-empty", "-m", "keep staged"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD^{tree}"])?, expected);
    Ok(())
}

#[test]
fn initial_commit_and_empty_rejection() -> Result<()> {
    let repo = tempfile::tempdir()?;
    git(repo.path(), &["init", "-q", "--initial-branch=main"])?;
    fs::write(repo.path().join("first"), "first")?;
    git(repo.path(), &["add", "first"])?;
    let output = brit(repo.path(), &["commit", "-m", "initial"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    let head = git(repo.path(), &["rev-parse", "HEAD"])?;
    let rejected = brit(repo.path(), &["commit", "-m", "empty"])?;
    ensure!(!rejected.status.success(), "empty commit must refuse");
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD"])?, head);
    Ok(())
}

#[test]
fn failing_commit_hooks_preserve_head() -> Result<()> {
    for name in ["pre-commit", "prepare-commit-msg", "commit-msg"] {
        let repo = TestRepo::new(name)?;
        hook(repo.path(), name, "echo rejected-by-hook >&2; exit 1")?;
        let head = repo.head_id()?;
        let output = brit(repo.path(), &["commit", "--allow-empty", "-m", "rejected"])?;
        ensure!(!output.status.success(), "{name} must reject");
        ensure!(output.stderr.contains("rejected-by-hook"), "{}", output.stderr);
        assert_eq!(repo.head_id()?, head);
    }
    Ok(())
}

#[test]
fn hooks_edit_message_and_run_in_order() -> Result<()> {
    let repo = TestRepo::new("hook-order")?;
    hook(
        repo.path(),
        "pre-commit",
        "echo pre >> order; test -n \"$GIT_INDEX_FILE\"",
    )?;
    hook(
        repo.path(),
        "prepare-commit-msg",
        "echo prepare >> order; test \"$2\" = message || exit 1; echo prepared >> \"$1\"",
    )?;
    hook(
        repo.path(),
        "commit-msg",
        "echo message >> order; echo checked >> \"$1\"",
    )?;
    hook(repo.path(), "post-commit", "echo post >> order")?;
    let output = brit(repo.path(), &["commit", "--allow-empty", "-m", "original"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(
        fs::read_to_string(repo.path().join("order"))?,
        "pre\nprepare\nmessage\npost\n"
    );
    assert_eq!(
        git(repo.path(), &["log", "-1", "--format=%B"])?,
        "original\nprepared\nchecked"
    );
    Ok(())
}

#[test]
fn push_refuses_to_silently_skip_a_gate() -> Result<()> {
    let repo = TestRepo::new("push-gate")?;
    let remote = tempfile::tempdir()?;
    git(remote.path(), &["init", "--bare", "-q"])?;
    git(
        repo.path(),
        &["remote", "add", "origin", remote.path().to_str().expect("temp path")],
    )?;
    hook(repo.path(), "pre-push", "echo should-not-be-skipped >&2; exit 1")?;
    let output = brit(repo.path(), &["push", "origin", "main:main"])?;
    ensure!(!output.status.success(), "must refuse when a pre-push gate exists");
    ensure!(
        output.stderr.contains("pre-push") && output.stderr.contains("git push"),
        "{}",
        output.stderr
    );
    assert_eq!(git(remote.path(), &["for-each-ref"])?, "");
    Ok(())
}

#[test]
fn no_verify_skips_only_the_two_verification_hooks() -> Result<()> {
    let repo = TestRepo::new("hook-bypass")?;
    hook(repo.path(), "pre-commit", "exit 1")?;
    hook(repo.path(), "commit-msg", "exit 1")?;
    hook(repo.path(), "prepare-commit-msg", "echo prepared >> \"$1\"")?;
    let output = brit(
        repo.path(),
        &["commit", "--no-verify", "--allow-empty", "-m", "explicit bypass"],
    )?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(
        git(repo.path(), &["log", "-1", "--format=%B"])?,
        "explicit bypass\nprepared"
    );
    hook(repo.path(), "prepare-commit-msg", "exit 1")?;
    let head = repo.head_id()?;
    ensure!(!brit(
        repo.path(),
        &["commit", "--no-verify", "--allow-empty", "-m", "rejected"]
    )?
    .status
    .success());
    assert_eq!(repo.head_id()?, head);
    Ok(())
}

#[test]
fn linked_worktree_commit_uses_its_own_index_and_shared_hooks() -> Result<()> {
    let repo = TestRepo::new("linked-authoring")?;
    let linked = tempfile::tempdir()?;
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.path().to_str().expect("temp path"),
        ],
    )?;
    fs::write(repo.path().join("unrelated"), "main staged")?;
    git(repo.path(), &["add", "unrelated"])?;
    let main_head = repo.head_id()?;
    let main_index = fs::read(repo.path().join(".git/index"))?;
    fs::write(linked.path().join("selected"), "linked staged")?;
    git(linked.path(), &["add", "selected"])?;
    let expected = git(linked.path(), &["write-tree"])?;
    hook(repo.path(), "pre-commit", "git diff --cached --name-only > observed")?;
    // Default hooks live in the shared common directory, not .git/worktrees/<name>.
    git(repo.path(), &["config", "--unset", "core.hooksPath"])?;
    fs::copy(
        repo.path().join("hooks with spaces/pre-commit"),
        repo.path().join(".git/hooks/pre-commit"),
    )?;
    fs::create_dir(linked.path().join("nested"))?;
    let output = brit(&linked.path().join("nested"), &["commit", "-m", "linked"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(linked.path(), &["rev-parse", "HEAD^{tree}"])?, expected);
    assert_eq!(fs::read_to_string(linked.path().join("observed"))?, "selected\n");
    assert_eq!(repo.head_id()?, main_head);
    assert_eq!(fs::read(repo.path().join(".git/index"))?, main_index);
    Ok(())
}

#[test]
fn pre_commit_can_stage_changes_and_post_commit_failure_is_a_warning() -> Result<()> {
    let repo = TestRepo::new("hook-staging")?;
    hook(
        repo.path(),
        "pre-commit",
        "echo generated > generated; git add generated",
    )?;
    hook(repo.path(), "post-commit", "echo post-failed >&2; exit 1")?;
    let output = brit(repo.path(), &["commit", "-m", "generated by gate"])?;
    ensure!(
        output.status.success(),
        "commit has already succeeded: {}",
        output.stderr
    );
    assert_eq!(git(repo.path(), &["show", "HEAD:generated"])?, "generated");
    ensure!(
        output.stderr.contains("warning:") && output.stderr.contains("post-failed"),
        "{}",
        output.stderr
    );
    Ok(())
}

#[test]
fn unmerged_index_and_in_progress_merge_refuse_without_advancing_head() -> Result<()> {
    let repo = TestRepo::new("conflicted-authoring")?;
    repo.commit_file("file", "base")?;
    git(repo.path(), &["switch", "-c", "other"])?;
    repo.commit_file("file", "other")?;
    git(repo.path(), &["switch", "main"])?;
    repo.commit_file("file", "main")?;
    let head = repo.head_id()?;
    let merged = gix_testtools::git_command(repo.path())
        .args(["merge", "other"])
        .output()?;
    ensure!(!merged.status.success());
    ensure!(!brit(repo.path(), &["commit", "--allow-empty", "-m", "conflict"])?
        .status
        .success());
    assert_eq!(repo.head_id()?, head);
    fs::remove_file(repo.path().join(".git/MERGE_HEAD"))?;
    let output = brit(repo.path(), &["commit", "--allow-empty", "-m", "still unmerged"])?;
    ensure!(
        !output.status.success() && output.stderr.contains("unmerged"),
        "{}",
        output.stderr
    );
    assert_eq!(repo.head_id()?, head);
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinks_and_executable_modes_match_git() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let repo = TestRepo::new("modes-authoring")?;
    fs::write(repo.path().join("executable"), "#!/bin/sh\nexit 0\n")?;
    fs::set_permissions(repo.path().join("executable"), fs::Permissions::from_mode(0o755))?;
    symlink("missing-target", repo.path().join("link"))?;
    git(repo.path(), &["add", "."])?;
    let expected = git(repo.path(), &["write-tree"])?;
    let output = brit(repo.path(), &["commit", "-m", "modes"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD^{tree}"])?, expected);
    Ok(())
}

#[test]
fn byte_paths_come_from_the_index_without_requiring_host_filename_support() -> Result<()> {
    use std::{io::Write, process::Stdio};
    let repo = TestRepo::new("byte-index-authoring")?;
    fs::write(repo.path().join("binary-fixture"), b"\0binary\xff")?;
    let blob = git(repo.path(), &["hash-object", "-w", "binary-fixture"])?;
    // A byte name belongs to Git's index/object model. APFS need not be able
    // to materialize it for this commit test to qualify the bridge semantics.
    let mut command = gix_testtools::git_command(repo.path());
    let mut child = command
        .args(["update-index", "-z", "--index-info"])
        .stdin(Stdio::piped())
        .spawn()?;
    let mut entry = format!("100644 {blob}\tbyte-").into_bytes();
    entry.extend_from_slice(b"\xff\0");
    child.stdin.take().expect("piped stdin").write_all(&entry)?;
    ensure!(child.wait()?.success());
    let expected = git(repo.path(), &["write-tree"])?;
    let output = brit(repo.path(), &["commit", "-m", "byte index"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD^{tree}"])?, expected);
    Ok(())
}

#[test]
fn hook_moving_head_refuses_our_commit() -> Result<()> {
    let repo = TestRepo::new("hook-head-race")?;
    hook(repo.path(), "pre-commit", "git switch -c moved")?;
    let head = repo.head_id()?;
    let output = brit(
        repo.path(),
        &["commit", "--allow-empty", "-m", "must not follow moved HEAD"],
    )?;
    ensure!(
        !output.status.success() && output.stderr.contains("HEAD changed"),
        "{}",
        output.stderr
    );
    assert_eq!(repo.head_id()?, head);
    Ok(())
}

#[test]
fn prepare_hook_can_supply_an_initially_empty_message_without_a_shebang() -> Result<()> {
    let repo = TestRepo::new("hook-message")?;
    hook(repo.path(), "prepare-commit-msg", "")?;
    fs::write(
        repo.path().join("hooks with spaces/prepare-commit-msg"),
        "echo supplied > \"$1\"\n",
    )?;
    let output = brit(repo.path(), &["commit", "--allow-empty", "-m", ""])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(repo.path(), &["log", "-1", "--format=%B"])?, "supplied");
    hook(repo.path(), "commit-msg", ": > \"$1\"")?;
    let head = repo.head_id()?;
    ensure!(!brit(repo.path(), &["commit", "--allow-empty", "-m", "cleared"])?
        .status
        .success());
    assert_eq!(repo.head_id()?, head);
    Ok(())
}

#[test]
fn alternate_index_is_shared_with_hooks_without_touching_the_default_index() -> Result<()> {
    let repo = TestRepo::new("alternate-index")?;
    repo.commit_file("file", "original")?;
    let index_before = fs::read(repo.path().join(".git/index"))?;
    let alternate = repo.path().join("alternate-index");
    fs::write(&alternate, &index_before)?;
    fs::write(repo.path().join("file"), "alternate")?;
    let staged = gix_testtools::git_command(repo.path())
        .env("GIT_INDEX_FILE", &alternate)
        .args(["add", "file"])
        .output()?;
    ensure!(staged.status.success());
    hook(repo.path(), "pre-commit", "git show :file > observed")?;
    let output = BritInvocation::new(brit_bin().expect("binary"))
        .args(["commit", "-m", "alternate"])
        .env("GIT_INDEX_FILE", alternate.as_os_str())
        .current_dir(repo.path())
        .env("GIT_AUTHOR_NAME", "Author")
        .env("GIT_AUTHOR_EMAIL", "a@example.com")
        .env("GIT_COMMITTER_NAME", "Author")
        .env("GIT_COMMITTER_EMAIL", "a@example.com")
        .run()?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(fs::read_to_string(repo.path().join("observed"))?, "alternate");
    assert_eq!(git(repo.path(), &["show", "HEAD:file"])?, "alternate");
    assert_eq!(fs::read(repo.path().join(".git/index"))?, index_before);
    Ok(())
}

#[test]
fn intent_to_add_is_not_committed() -> Result<()> {
    let repo = TestRepo::new("intent-to-add")?;
    fs::write(repo.path().join("intent"), "unstaged")?;
    git(repo.path(), &["add", "-N", "intent"])?;
    let tree = git(repo.path(), &["rev-parse", "HEAD^{tree}"])?;
    let output = brit(repo.path(), &["commit", "--allow-empty", "-m", "no staged content"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD^{tree}"])?, tree);
    assert!(git(repo.path(), &["status", "--porcelain"])?.contains("intent"));
    Ok(())
}

#[test]
fn unsupported_path_scoped_commit_preserves_unrelated_staged_work() -> Result<()> {
    let repo = TestRepo::new("scoped-refusal")?;
    fs::write(repo.path().join("selected"), "this task")?;
    fs::write(repo.path().join("unrelated"), "another task")?;
    git(repo.path(), &["add", "selected", "unrelated"])?;
    let head = repo.head_id()?;
    let index = fs::read(repo.path().join(".git/index"))?;
    let output = brit(repo.path(), &["commit", "-m", "only selected", "--", "selected"])?;
    ensure!(
        !output.status.success(),
        "unsupported scope must never commit the whole index"
    );
    assert_eq!(repo.head_id()?, head);
    assert_eq!(fs::read(repo.path().join(".git/index"))?, index);
    Ok(())
}

#[test]
fn push_bypass_is_explicit_and_hook_path_can_be_absolute() -> Result<()> {
    let repo = TestRepo::new("push-explicit-bypass")?;
    let remote = tempfile::tempdir()?;
    git(remote.path(), &["init", "--bare", "-q"])?;
    git(
        repo.path(),
        &["remote", "add", "origin", remote.path().to_str().expect("temp path")],
    )?;
    hook(repo.path(), "pre-push", "exit 1")?;
    let absolute = repo.path().join("hooks with spaces");
    git(
        repo.path(),
        &["config", "core.hooksPath", absolute.to_str().expect("temp path")],
    )?;
    ensure!(!brit(repo.path(), &["push", "origin", "main:main"])?.status.success());
    let output = brit(repo.path(), &["push", "--no-verify", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/main"])?, repo.head_id()?);
    Ok(())
}

#[cfg(unix)]
#[test]
fn non_executable_hooks_are_ignored_and_null_hook_path_disables_hooks() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let repo = TestRepo::new("disabled-hooks")?;
    hook(repo.path(), "pre-commit", "exit 1")?;
    fs::set_permissions(
        repo.path().join("hooks with spaces/pre-commit"),
        fs::Permissions::from_mode(0o644),
    )?;
    let output = brit(repo.path(), &["commit", "--allow-empty", "-m", "non-executable"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    hook(repo.path(), "pre-commit", "exit 1")?;
    git(repo.path(), &["config", "core.hooksPath", "/dev/null"])?;
    let output = brit(repo.path(), &["commit", "--allow-empty", "-m", "disabled"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    Ok(())
}
