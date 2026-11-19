//! Public rebase journeys against the exact BRIT_BIN and retained helper artifacts.
#![cfg(target_os = "linux")]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

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

fn invoke(root: &Path, args: &[&str]) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("required public artifact"))
        .current_dir(root)
        .args(args.iter().copied())
        .env("GIT_EDITOR", message_editor(root))
        .env("GIT_SEQUENCE_EDITOR", ":")
        .run()
}

fn message_editor(root: &Path) -> std::ffi::OsString {
    let editor = root.join(".git/fixed-message-editor");
    if editor.is_file() {
        editor.into_os_string()
    } else {
        ":".into()
    }
}

fn success(root: &Path, args: &[&str]) -> Result<Capture> {
    let result = invoke(root, args)?;
    ensure!(
        result.status.success(),
        "{args:?}: {}\n{}",
        result.stdout,
        result.stderr
    );
    Ok(result)
}

fn refused(root: &Path, args: &[&str]) -> Result<Capture> {
    let result = invoke(root, args)?;
    ensure!(
        !result.status.success(),
        "unexpected success {args:?}: {}",
        result.stdout
    );
    Ok(result)
}

fn fixture(conflict: bool) -> Result<TestRepo> {
    let repo = TestRepo::new("public-rebase")?;
    // Both raw SHA-1 collision fixtures are legitimate, distinct Git blobs.
    for name in ["messageA", "messageB"] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../gix-hash/tests/fixtures/shambles")
                .join(name),
            repo.path().join(name),
        )?;
    }
    git(repo.path(), &["add", "messageA", "messageB"])?;
    repo.commit_file("selected", "base\n")?;
    repo.commit_file("neighbor", "base neighbor\n")?;
    git(repo.path(), &["switch", "-c", "upstream"])?;
    repo.commit_file(if conflict { "selected" } else { "upstream" }, "upstream\n")?;
    git(repo.path(), &["switch", "main"])?;
    repo.commit_file("selected", "topic\n")?;
    repo.commit_file("second", "second\n")?;
    fs::write(repo.path().join("neighbor"), b"staged neighbor\n")?;
    git(repo.path(), &["add", "neighbor"])?;
    fs::write(repo.path().join("neighbor"), b"unstaged neighbor\n")?;
    Ok(repo)
}

fn preserved(root: &Path) -> Result<()> {
    for name in ["messageA", "messageB"] {
        let expected = fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../gix-hash/tests/fixtures/shambles")
                .join(name),
        )?;
        assert_eq!(fs::read(root.join(name))?, expected);
        assert_eq!(git(root, &["show", &format!("HEAD:{name}")])?, expected);
    }
    assert_eq!(git(root, &["show", ":neighbor"])?, b"staged neighbor\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged neighbor\n");
    assert_eq!(git(root, &["show", "HEAD:neighbor"])?, b"base neighbor\n");
    Ok(())
}

fn script(path: &Path, body: &str) -> Result<()> {
    fs::write(path, format!("#!/bin/sh\nset -eu\n{body}\n"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn interactive(root: &Path, todo: &str) -> Result<Capture> {
    let editor = root.join(".git/sequence-editor");
    script(&editor, &format!("printf '%s' '{todo}' > \"$1\""))?;
    BritInvocation::new(brit_bin().expect("required artifact"))
        .current_dir(root)
        .args(["rebase", "-i", "upstream"])
        .env("GIT_SEQUENCE_EDITOR", editor.into_os_string())
        .env("GIT_EDITOR", message_editor(root))
        .run()
}

fn ids(root: &Path) -> Result<Vec<String>> {
    Ok(
        String::from_utf8(git(root, &["rev-list", "--reverse", "upstream..main"])?)?
            .lines()
            .map(str::to_owned)
            .collect(),
    )
}

fn retained_leaf(root: &Path) -> Result<PathBuf> {
    let mut leaves = Vec::new();
    for operation in fs::read_dir(root.join(".git/brit-rebase-retained"))? {
        for copy in fs::read_dir(operation?.path())? {
            let path = copy?.path();
            if path.join("COPY_COMPLETE").is_file() {
                leaves.push(path);
            }
        }
    }
    leaves.sort();
    leaves
        .pop()
        .ok_or_else(|| anyhow::anyhow!("no completed retained copy"))
}

fn repository_bytes(root: &Path) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>> {
    fn walk(root: &Path, path: &Path, files: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(root, &path, files)?;
            } else {
                files.insert(path.strip_prefix(root)?.to_path_buf(), fs::read(path)?);
            }
        }
        Ok(())
    }
    let mut files = std::collections::BTreeMap::new();
    walk(root, root, &mut files)?;
    Ok(files)
}

fn linked_fixture(conflict: bool) -> Result<(TestRepo, tempfile::TempDir, PathBuf)> {
    let main = fixture(conflict)?;
    let temp = tempfile::Builder::new().prefix("rebase with spaces ").tempdir()?;
    let root = temp.path().join("linked worktree");
    git(
        main.path(),
        &[
            "worktree",
            "add",
            "-b",
            "topic/nested",
            root.to_str().expect("temporary path"),
            "main",
        ],
    )?;
    fs::write(root.join("neighbor"), b"staged neighbor\n")?;
    git(&root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged neighbor\n")?;
    fs::write(root.join("private-neighbor"), b"untracked neighbor\n")?;
    let git_dir = git(&root, &["rev-parse", "--absolute-git-dir"])?;
    let git_dir = PathBuf::from(String::from_utf8(git_dir)?.trim());
    assert_eq!(fs::read(git_dir.join("commondir"))?, b"../..\n");
    Ok((main, temp, root))
}

fn main_private_bytes(root: &Path) -> Result<Vec<Vec<u8>>> {
    [
        "selected",
        "second",
        "neighbor",
        ".git/HEAD",
        ".git/index",
        ".git/logs/HEAD",
        ".git/refs/heads/main",
        ".git/logs/refs/heads/main",
    ]
    .iter()
    .map(|name| fs::read(root.join(name)).map_err(Into::into))
    .collect()
}

#[test]
fn linked_worktree_rebase_publishes_nested_branch_and_preserves_both_worktrees() -> Result<()> {
    let (main, _temp, root) = linked_fixture(false)?;
    let before = main_private_bytes(main.path())?;
    let original = git(&root, &["rev-parse", "HEAD"])?;
    let head_log = git(&root, &["reflog", "show", "--format=%H", "HEAD"])?;
    let branch_log = git(&root, &["reflog", "show", "--format=%H", "topic/nested"])?;
    // An optional predecessor artifact proves byte-compatible recovery of the
    // journal that originally exposed this bug; ordinary CI exercises start.
    if let Some(baseline) = std::env::var_os("BRIT_REBASE_BASELINE_BIN") {
        let result = BritInvocation::new(PathBuf::from(baseline))
            .current_dir(&root)
            .args(["rebase", "upstream"])
            .env("GIT_EDITOR", message_editor(&root))
            .env("GIT_SEQUENCE_EDITOR", ":")
            .run()?;
        ensure!(!result.status.success() && result.stderr.contains("unsafe metadata ancestor"));
        let git_dir = PathBuf::from(String::from_utf8(git(&root, &["rev-parse", "--absolute-git-dir"])?)?.trim());
        let state = fs::read(git_dir.join("brit-rebase/state.json"))?;
        let envelope: serde_json::Value = serde_json::from_slice(&state)?;
        let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let value: serde_json::Value = serde_json::from_slice(&payload)?;
        assert_eq!(value["version"], 1);
        assert_eq!(value["phase"], "ready-final");
        assert!(success(&root, &["rebase", "--status"])?.stdout.contains("ready-final"));
        assert_eq!(
            fs::read(git_dir.join("brit-rebase/state.json"))?,
            state,
            "inspection leaves old bytes intact"
        );
        success(&root, &["rebase", "--continue"])?;
    } else {
        success(&root, &["rebase", "upstream"])?;
    }
    assert_eq!(git(&root, &["symbolic-ref", "HEAD"])?, b"refs/heads/topic/nested\n");
    assert_eq!(
        git(&root, &["rev-parse", "HEAD~2"])?,
        git(&root, &["rev-parse", "upstream"])?
    );
    assert_eq!(git(&root, &["rev-parse", "ORIG_HEAD"])?, original);
    assert_eq!(
        git(&root, &["diff", "--name-only", "upstream", "HEAD", "--"])?,
        b"second\nselected\n"
    );
    assert_eq!(fs::read(root.join("selected"))?, b"topic\n");
    assert_eq!(fs::read(root.join("second"))?, b"second\n");
    assert_eq!(fs::read(root.join("upstream"))?, b"upstream\n");
    for (reference, old) in [("HEAD", head_log), ("topic/nested", branch_log)] {
        let log = git(&root, &["reflog", "show", "--format=%H", reference])?;
        assert!(log.starts_with(&git(&root, &["rev-parse", "HEAD"])?));
        assert!(log.ends_with(&old), "existing reflog entries survive");
        assert!(log.len() > old.len(), "publication appends its reflog");
    }
    preserved(&root)?;
    assert_eq!(fs::read(root.join("private-neighbor"))?, b"untracked neighbor\n");
    assert_eq!(main_private_bytes(main.path())?, before);
    assert!(success(&root, &["rebase", "--status"])?
        .stdout
        .contains("No active rebase"));
    Ok(())
}

#[test]
fn linked_worktree_conflict_continue_abort_and_detached_head_preserve_neighbors() -> Result<()> {
    for mode in ["continue", "abort", "detached"] {
        let (main, _temp, root) = linked_fixture(mode != "detached")?;
        let before = main_private_bytes(main.path())?;
        let original = git(&root, &["rev-parse", "HEAD"])?;
        if mode == "detached" {
            git(&root, &["switch", "--detach"])?;
            success(&root, &["rebase", "upstream"])?;
            assert!(git(&root, &["rev-parse", "--abbrev-ref", "HEAD"])? == b"HEAD\n");
            assert_eq!(git(&root, &["rev-parse", "topic/nested"])?, original);
        } else {
            refused(&root, &["rebase", "upstream"])?;
            assert!(
                !git(&root, &["ls-files", "-u"])?.is_empty(),
                "fixture stops on conflict"
            );
            if mode == "abort" {
                success(&root, &["rebase", "--abort"])?;
                assert_eq!(git(&root, &["rev-parse", "HEAD"])?, original);
            } else {
                fs::write(root.join("selected"), b"resolved\n")?;
                success(&root, &["rebase", "--stage", "--", "selected"])?;
                success(&root, &["rebase", "--continue"])?;
                assert_eq!(git(&root, &["show", "HEAD:selected"])?, b"resolved\n");
            }
            assert_eq!(git(&root, &["symbolic-ref", "HEAD"])?, b"refs/heads/topic/nested\n");
        }
        if mode != "abort" {
            assert_eq!(
                git(&root, &["rev-parse", "HEAD~2"])?,
                git(&root, &["rev-parse", "upstream"])?
            );
        }
        preserved(&root)?;
        assert_eq!(fs::read(root.join("private-neighbor"))?, b"untracked neighbor\n");
        assert_eq!(main_private_bytes(main.path())?, before);
        assert!(success(&root, &["rebase", "--status"])?
            .stdout
            .contains("No active rebase"));
    }
    Ok(())
}

fn rev(root: &Path, revision: &str) -> Result<String> {
    Ok(String::from_utf8(git(root, &["rev-parse", revision])?)?
        .trim()
        .to_owned())
}

/// Rebase still refuses a shallow repository, even when the selection itself lies above the
/// boundary: its retained copy is proven complete by walking released commits to the roots
/// from the copy alone. The refusal happens before any state, hook or ref change.
#[test]
fn shallow_repository_rebase_refuses_before_any_state() -> Result<()> {
    let repo = fixture(false)?;
    let root = repo.path();
    let base = String::from_utf8(git(root, &["merge-base", "main", "upstream"])?)?;
    fs::write(root.join(".git/shallow"), &base)?;
    let head = rev(root, "HEAD")?;
    let result = refused(root, &["rebase", "upstream"])?;
    ensure!(
        result.stderr.contains("shallow"),
        "diagnostic names the shallow refusal: {}",
        result.stderr
    );
    assert_eq!(rev(root, "HEAD")?, head);
    assert_eq!(fs::read_to_string(root.join(".git/shallow"))?, base);
    assert!(!root.join(".git/brit-rebase").exists());
    preserved(root)
}

/// A divergent repository whose index and history carry a gitlink at `sub`
/// whose submodule directory was never checked out.
fn uninitialised_gitlink_fixture() -> Result<TestRepo> {
    let repo = TestRepo::new("gitlink-rebase")?;
    let root = repo.path();
    repo.commit_file("base", "base\n")?;
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000,1111111111111111111111111111111111111111,sub",
        ],
    )?;
    git(root, &["commit", "-q", "-m", "gitlink"])?;
    git(root, &["switch", "-q", "-c", "upstream"])?;
    repo.commit_file("upstream", "upstream\n")?;
    git(root, &["switch", "-q", "main"])?;
    repo.commit_file("topic", "topic\n")?;
    ensure!(!root.join("sub").exists(), "submodule directory is absent");
    Ok(repo)
}

/// The gitlink's commit is not a local object, so preparation must never look
/// it up. Brit rebase still does not support gitlinks: one without a checked-out
/// submodule directory is refused by name before any operation state exists,
/// where it used to fail after creating its journal ("object could not be found").
#[test]
fn uninitialised_gitlink_refuses_by_name_before_any_state() -> Result<()> {
    let repo = uninitialised_gitlink_fixture()?;
    let root = repo.path();
    let head = rev(root, "HEAD")?;
    let index = fs::read(root.join(".git/index"))?;
    let result = refused(root, &["rebase", "upstream"])?;
    ensure!(
        result
            .stderr
            .contains("gitlink sub has no checked-out submodule directory")
            && !result.stderr.contains("could not be found"),
        "named early refusal: {}",
        result.stderr
    );
    assert!(!root.join(".git/brit-rebase").exists());
    assert!(git(root, &["for-each-ref", "refs/brit-rebase-retain"])?.is_empty());
    assert_eq!(rev(root, "HEAD")?, head);
    assert_eq!(fs::read(root.join(".git/index"))?, index);
    success(root, &["branch", "create", "after-refusal"])?;
    Ok(())
}

/// Case D, unchanged by this lane: a gitlink whose submodule directory is
/// checked out is refused before any state, with the message it had before.
#[test]
fn checked_out_gitlink_keeps_its_refusal_before_any_state() -> Result<()> {
    let repo = uninitialised_gitlink_fixture()?;
    let root = repo.path();
    fs::create_dir(root.join("sub"))?;
    let result = refused(root, &["rebase", "upstream"])?;
    ensure!(
        result.stderr.contains("Directory staging is not supported"),
        "{}",
        result.stderr
    );
    assert!(!root.join(".git/brit-rebase").exists());
    Ok(())
}

#[test]
fn public_linear_replay_finishes_and_helper_exports_after_cleanup_without_runtime_git() -> Result<()> {
    let repo = fixture(false)?;
    let root = repo.path();
    fs::write(root.join("private-neighbor"), b"private neighbor\0bytes\n")?;
    let originals = ["messageA", "messageB", "neighbor", "private-neighbor"]
        .into_iter()
        .map(|name| Ok((name, fs::read(root.join(name))?)))
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    let bin = brit_bin().expect("required artifact");
    let helper = std::env::var_os("BRIT_REBASE_RETAINED_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| bin.with_file_name("brit-rebase-retained"));
    ensure!(
        helper.is_file(),
        "required retained helper artifact missing: {helper:?}"
    );
    let sentinels = tempfile::tempdir()?;
    let marker = root.join(".git/git-was-invoked");
    script(
        &sentinels.path().join("git"),
        "printf called > \"$BRIT_GIT_SENTINEL\"; exit 97",
    )?;
    let result = BritInvocation::new(bin)
        .current_dir(root)
        .args(["rebase", "upstream"])
        .env("PATH", sentinels.path().as_os_str())
        .env("BRIT_GIT_SENTINEL", marker.as_os_str())
        .run()?;
    ensure!(result.status.success(), "{} {}", result.stdout, result.stderr);
    assert!(!marker.exists(), "replay must not spawn Git");
    assert!(!root.join(".git/brit-rebase").exists());
    preserved(root)?;
    assert!(success(root, &["rebase", "--status"])?
        .stdout
        .contains("No active rebase"));
    let copy = retained_leaf(root)?;
    let source_before = repository_bytes(root)?;
    let inspected = BritInvocation::new(&helper)
        .args(["inspect", "--copy"])
        .arg(copy.as_os_str())
        .run()?;
    ensure!(inspected.status.success(), "{}", inspected.stderr);
    let manifest: serde_json::Value = serde_json::from_str(&inspected.stdout)?;
    assert!(manifest["manifest"]["entries"]
        .as_array()
        .is_some_and(|entries| !entries.is_empty()));
    let destination = tempfile::tempdir()?;
    let extracted = BritInvocation::new(&helper)
        .args(["extract", "--copy"])
        .arg(copy.as_os_str())
        .arg("--to")
        .arg(destination.path().as_os_str())
        .env("PATH", sentinels.path().as_os_str())
        .run()?;
    ensure!(extracted.status.success(), "{}", extracted.stderr);
    let exported: serde_json::Value = serde_json::from_slice(&fs::read(destination.path().join("manifest.json"))?)?;
    assert_eq!(
        exported["manifest"], manifest["manifest"],
        "export retains every inspected entry"
    );
    let entries = exported["manifest"]["entries"].as_array().expect("entries");
    let payloads = exported["payloads"].as_array().expect("payloads");
    assert_eq!(entries.len(), payloads.len());
    let mut seen = std::collections::BTreeSet::new();
    for (ordinal, (_entry, payload)) in entries.iter().zip(payloads).enumerate() {
        let bytes = fs::read(destination.path().join(payload.as_str().expect("payload path")))?;
        assert_eq!(
            bytes,
            fs::read(copy.join(format!("{ordinal:08}")))?,
            "exported member {ordinal} matches captured bytes"
        );
        for (name, original) in &originals {
            if &bytes == original {
                seen.insert((*name).to_owned());
            }
        }
    }
    // The copy captures replay footprint/object custody, not private neighbors.
    assert!(
        seen.contains("messageA") && seen.contains("messageB"),
        "collision fixture bytes are independently exported: {seen:?}"
    );
    for (name, bytes) in &originals {
        assert_eq!(&fs::read(root.join(name))?, bytes);
    }
    assert_eq!(
        repository_bytes(root)?,
        source_before,
        "inspect and extract leave every source file unchanged"
    );
    assert!(
        copy.join("COPY_COMPLETE").is_file(),
        "helper never expires retained copies"
    );
    let again = BritInvocation::new(&helper)
        .args(["extract", "--copy"])
        .arg(copy.as_os_str())
        .arg("--to")
        .arg(destination.path().as_os_str())
        .run()?;
    assert!(!again.status.success(), "nonempty destination refuses");
    Ok(())
}

#[test]
fn public_conflict_status_patch_resolution_and_exact_abort() -> Result<()> {
    for abort in [false, true] {
        let repo = fixture(true)?;
        let root = repo.path();
        let head = git(root, &["rev-parse", "HEAD"])?;
        let index = fs::read(root.join(".git/index"))?;
        refused(root, &["rebase", "upstream"])?;
        let status = success(root, &["rebase", "--status"])?;
        assert!(status.stdout.contains("Conflict stop"), "{}", status.stdout);
        assert!(success(root, &["rebase", "--show-current-patch"])?
            .stdout
            .contains("topic"));
        refused(root, &["rebase", "--amend", "--no-edit"])?;
        refused(root, &["add", "."])?;
        refused(root, &["commit", "-m", "ordinary"])?;
        fs::write(root.join("selected"), b"resolved\n")?;
        success(root, &["rebase", "--stage", "--", "selected"])?;
        if abort {
            success(root, &["rebase", "--abort"])?;
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
            assert_eq!(fs::read(root.join(".git/index"))?, index);
            assert_eq!(fs::read(root.join("selected"))?, b"topic\n");
        } else {
            success(root, &["rebase", "--continue"])?;
            assert_eq!(git(root, &["show", "HEAD:selected"])?, b"resolved\n");
        }
        preserved(root)?;
    }
    Ok(())
}

#[test]
fn public_edit_authoring_forms_scope_expansion_deletion_and_split_messages() -> Result<()> {
    for split in [false, true] {
        let repo = fixture(false)?;
        let root = repo.path();
        if split {
            // Editor configuration is pinned for the entire operation. Keep the
            // executable choice fixed and supply its text through a private file.
            script(
                &root.join(".git/fixed-message-editor"),
                "cat .git/editor-message-input > \"$1\"",
            )?;
            fs::write(root.join(".git/editor-message-input"), b"editor split message\n")?;
        }
        let candidates = ids(root)?;
        let result = interactive(
            root,
            &format!("edit {}\nbreak\npick {}\n", candidates[0], candidates[1]),
        )?;
        ensure!(result.status.success(), "{} {}", result.stdout, result.stderr);
        assert!(success(root, &["rebase", "--status"])?.stdout.contains("Edit stop"));
        let message = root.join(".git/message-input");
        fs::write(&message, b"from message file\n")?;
        if split {
            success(root, &["rebase", "--begin-split"])?;
            success(root, &["rebase", "--stage", "--", "selected"])?;
            success(root, &["rebase", "--commit", "-m", "split first"])?;
            fs::write(root.join("expanded"), b"split second\n")?;
            success(root, &["rebase", "--stage", "--expand-scope", "--", "expanded"])?;
            success(
                root,
                &["rebase", "--commit", "-F", message.to_str().expect("fixture path")],
            )?;
            fs::write(root.join("selected"), b"editor split\n")?;
            success(root, &["rebase", "--stage", "--", "selected"])?;
            success(root, &["rebase", "--commit"])?;
            assert_eq!(git(root, &["log", "-1", "--format=%s"])?, b"editor split message\n");
        } else {
            fs::write(root.join("selected"), b"amended\n")?;
            success(root, &["rebase", "--stage", "--", "selected"])?;
            success(root, &["rebase", "--amend", "-m", "amend explicit"])?;
            success(
                root,
                &["rebase", "--amend", "-F", message.to_str().expect("fixture path")],
            )?;
            success(root, &["rebase", "--amend", "--no-edit"])?;
            success(root, &["rebase", "--amend"])?;
            assert_eq!(git(root, &["log", "-1", "--format=%s"])?, b"from message file\n");
            fs::remove_file(root.join("selected"))?;
            success(root, &["rebase", "--stage", "--", "selected"])?;
            success(root, &["rebase", "--amend", "--no-edit"])?;
        }
        success(root, &["rebase", "--continue"])?;
        assert!(success(root, &["rebase", "--status"])?.stdout.contains("break"));
        refused(root, &["rebase", "--stage", "--expand-scope", "--", "neighbor"])?;
        success(root, &["rebase", "--edit-todo"])?;
        success(root, &["rebase", "--skip"])?;
        preserved(root)?;
        assert!(!root.join(".git/brit-rebase").exists());
    }
    Ok(())
}

#[test]
fn failed_todo_at_edit_refuses_invalid_saved_bytes_before_accepting_authored_group() -> Result<()> {
    let repo = fixture(false)?;
    let root = repo.path();
    let candidates = ids(root)?;
    let result = interactive(root, &format!("edit {}\npick {}\n", candidates[0], candidates[1]))?;
    ensure!(result.status.success(), "{}", result.stderr);
    let before = git(root, &["rev-parse", "HEAD"])?;
    let bad = root.join(".git/bad-editor");
    script(&bad, "printf 'unknown not-a-commit\\n' > \"$1\"; exit 19")?;
    let edited = BritInvocation::new(brit_bin().expect("artifact"))
        .current_dir(root)
        .args(["rebase", "--edit-todo"])
        .env("GIT_SEQUENCE_EDITOR", bad.into_os_string())
        .env("GIT_EDITOR", message_editor(root))
        .run()?;
    assert!(!edited.status.success());
    assert!(edited.stderr.contains("editor did not succeed"), "{}", edited.stderr);
    assert_eq!(
        fs::read(root.join(".git/brit-rebase/editor-1-output"))?,
        b"unknown not-a-commit\n"
    );
    refused(root, &["rebase", "--continue"])?;
    assert_eq!(git(root, &["rev-parse", "HEAD"])?, before);
    assert!(success(root, &["rebase", "--status"])?.stdout.contains("Edit stop"));
    success(root, &["rebase", "--abort"])?;
    preserved(root)?;
    Ok(())
}

#[cfg(feature = "switch-test-faults")]
mod recovery {
    use super::*;
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };

    struct Running(Child);
    impl Drop for Running {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    struct HookCleanup(PathBuf);
    impl Drop for HookCleanup {
        fn drop(&mut self) {
            // Test failure must not leak the deliberately endless fixture hook.
            // Only signal a group whose leader still has its recorded identity.
            let cleanup = || -> Result<()> {
                let envelope: serde_json::Value = serde_json::from_slice(&fs::read(self.0.join("launch.json"))?)?;
                let bytes: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
                let identity: serde_json::Value = serde_json::from_slice(&bytes)?;
                let pid = identity["pid"].as_u64().ok_or_else(|| anyhow::anyhow!("missing PID"))?;
                let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
                let fields = stat
                    .rsplit_once(')')
                    .ok_or_else(|| anyhow::anyhow!("invalid process stat"))?
                    .1;
                if fields.split_whitespace().nth(19) == identity["start"].as_str() {
                    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).status();
                }
                Ok(())
            };
            let _ = cleanup();
        }
    }

    fn launch(root: &Path, args: &[&str], variables: &[(&str, &std::ffi::OsStr)]) -> Result<Running> {
        let mut command = Command::new(brit_bin().expect("fault-enabled artifact"));
        gix_testtools::configure_git_environment(&mut command, root);
        command
            .current_dir(root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", root.join(".git/missing-global"))
            .env("GIT_EDITOR", ":")
            .env("GIT_SEQUENCE_EDITOR", ":")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        for (name, value) in variables {
            command.env(name, value);
        }
        Ok(Running(command.spawn()?))
    }

    fn wait_file(child: &mut Running, path: &Path) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !path.exists() {
            ensure!(child.0.try_wait()?.is_none(), "child exited before {path:?}");
            ensure!(Instant::now() < deadline, "checkpoint timeout {path:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn interrupt(child: &mut Running, signal: &str) -> Result<()> {
        ensure!(Command::new("kill")
            .args([signal, &child.0.id().to_string()])
            .status()?
            .success());
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.0.try_wait()? {
                ensure!(!status.success());
                return Ok(());
            }
            ensure!(Instant::now() < deadline, "signal did not stop command");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_invocation_exit(directory: &Path) -> Result<()> {
        let envelope: serde_json::Value = serde_json::from_slice(&fs::read(directory.join("launch.json"))?)?;
        let bytes: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let identity: serde_json::Value = serde_json::from_slice(&bytes)?;
        let pid = identity["pid"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("launch PID missing"))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match fs::read_to_string(format!("/proc/{pid}/stat")) {
                Ok(stat)
                    if stat
                        .rsplit_once(')')
                        .is_some_and(|(_, fields)| matches!(fields.split_whitespace().next(), Some("Z" | "X"))) =>
                {
                    return Ok(());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error.into()),
                _ => {}
            }
            ensure!(Instant::now() < deadline, "invocation child remains live");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn public_unknown_operation_hooks_never_rerun_and_final_ack_preserves_publication() -> Result<()> {
        for name in ["pre-rebase", "post-rewrite"] {
            for signal in ["-INT", "-KILL"] {
                let repo = fixture(false)?;
                let root = repo.path();
                let original = git(root, &["rev-parse", "refs/heads/main"])?;
                script(
                    &root.join(".git/hooks").join(name),
                    "printf 'called\\n' >> .git/hook-calls",
                )?;
                let mut child = launch(
                    root,
                    &["rebase", "upstream"],
                    &[("BRIT_AUTHORING_INVOCATION_PAUSE", "after-release".as_ref())],
                )?;
                wait_file(&mut child, &root.join(".git/brit-rebase").join(name).join("checkpoint"))?;
                wait_file(&mut child, &root.join(".git/hook-calls"))?;
                wait_invocation_exit(&root.join(".git/brit-rebase").join(name))?;
                interrupt(&mut child, signal)?;
                assert_eq!(fs::read(root.join(".git/hook-calls"))?, b"called\n");
                let status = success(root, &["rebase", "--status"])?;
                assert!(status.stdout.contains("outcome") || status.stdout.contains("published"));
                if name == "pre-rebase" {
                    refused(root, &["rebase", "--continue"])?;
                    success(root, &["rebase", "--abort"])?;
                    assert_eq!(git(root, &["rev-parse", "refs/heads/main"])?, original);
                } else {
                    let published = git(root, &["rev-parse", "refs/heads/main"])?;
                    assert_ne!(published, original);
                    refused(root, &["rebase", "--abort"])?;
                    success(root, &["rebase", "--continue"])?;
                    assert_eq!(git(root, &["rev-parse", "refs/heads/main"])?, published);
                }
                assert_eq!(fs::read(root.join(".git/hook-calls"))?, b"called\n");
                preserved(root)?;
            }
        }
        Ok(())
    }

    #[test]
    fn public_sigint_cancels_live_operation_hook_without_relaunch_or_indefinite_wait() -> Result<()> {
        for name in ["pre-rebase", "post-rewrite"] {
            let repo = fixture(false)?;
            let root = repo.path();
            let original = git(root, &["rev-parse", "refs/heads/main"])?;
            script(
                &root.join(".git/hooks").join(name),
                "printf 'called\\n' >> .git/live-hook-calls\nwhile :; do sleep 0.05; done",
            )?;
            let mut child = launch(root, &["rebase", "upstream"], &[])?;
            let _hook_cleanup = HookCleanup(root.join(".git/brit-rebase").join(name));
            wait_file(&mut child, &root.join(".git/live-hook-calls"))?;
            // This is an executing hook, not a paused instrumentation checkpoint.
            // The supervisor must react despite owning a separate process group.
            interrupt(&mut child, "-INT")?;
            let status = success(root, &["rebase", "--status"])?;
            ensure!(
                status.stdout.contains("outcome") || status.stdout.contains("published"),
                "cancellation diagnostics missing: {}",
                status.stdout
            );
            if name == "pre-rebase" {
                refused(root, &["rebase", "--continue"])?;
                success(root, &["rebase", "--abort"])?;
                assert_eq!(git(root, &["rev-parse", "refs/heads/main"])?, original);
            } else {
                let published = git(root, &["rev-parse", "refs/heads/main"])?;
                assert_ne!(published, original);
                refused(root, &["rebase", "--abort"])?;
                success(root, &["rebase", "--continue"])?;
                assert_eq!(git(root, &["rev-parse", "refs/heads/main"])?, published);
            }
            // Successful guarded abort/ack also proves the child group is quiescent.
            assert_eq!(fs::read(root.join(".git/live-hook-calls"))?, b"called\n");
            preserved(root)?;
        }
        Ok(())
    }

    #[test]
    fn public_selection_interrupt_leaves_no_journal_or_repository_changes() -> Result<()> {
        for point in [
            "replay-selection-object",
            "replay-object-verified",
            "replay-selected-commit",
            "replay-candidate-chain",
            "replay-selection-tree",
            "replay-footprint-candidate",
            "replay-footprint-path",
            "replay-index-before",
            "replay-index-after",
            "replay-changed-path",
            "replay-preservation-entry",
            "replay-worktree-witness-entry",
            "replay-before-journal",
            "replay-journal-ready",
        ] {
            let repo = fixture(false)?;
            let root = repo.path();
            let head = git(root, &["rev-parse", "HEAD"])?;
            let index = fs::read(root.join(".git/index"))?;
            let markers = tempfile::tempdir()?;
            let mut child = launch(
                root,
                &["rebase", "upstream"],
                &[
                    ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                    ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
                ],
            )?;
            wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
            let started = Instant::now();
            if matches!(
                point,
                "replay-selected-commit"
                    | "replay-candidate-chain"
                    | "replay-selection-tree"
                    | "replay-footprint-candidate"
                    | "replay-footprint-path"
                    | "replay-index-before"
                    | "replay-index-after"
                    | "replay-changed-path"
            ) {
                ensure!(Command::new("kill")
                    .args(["-INT", &child.0.id().to_string()])
                    .status()?
                    .success());
                // The rendezvous cannot refuse on its own: release to the real poll.
                fs::write(markers.path().join(format!("{point}.release")), b"release")?;
                loop {
                    if let Some(status) = child.0.try_wait()? {
                        ensure!(!status.success(), "cancelled rebase must refuse");
                        break;
                    }
                    ensure!(
                        started.elapsed() < Duration::from_secs(1),
                        "{point}: cancellation exceeded one second"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
                ensure!(started.elapsed() < Duration::from_secs(1));
                eprintln!(
                    "public cancellation point={point} elapsed_ns={} outcome=refused executable={}",
                    started.elapsed().as_nanos(),
                    brit_bin().expect("public artifact").display()
                );
            } else {
                interrupt(&mut child, "-INT")?;
            }
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
            assert_eq!(fs::read(root.join(".git/index"))?, index);
            assert!(
                !root.join(".git/brit-rebase").exists(),
                "selection never creates a journal"
            );
            assert!(git(root, &["for-each-ref", "refs/brit-rebase-retain"])?.is_empty());
            preserved(root)?;
            success(root, &["rebase", "upstream"])?;
            preserved(root)?;
        }
        Ok(())
    }

    #[test]
    fn public_preparation_cancel_preserves_recoverable_journal_and_stops_retention() -> Result<()> {
        for (point, expected_refs) in [
            ("replay-retention-verify", 0),
            ("replay-retention-reference", 0),
            ("replay-retention-reference-published", 1),
        ] {
            let repo = fixture(false)?;
            let root = repo.path();
            let head = git(root, &["rev-parse", "HEAD"])?;
            let index = fs::read(root.join(".git/index"))?;
            let markers = tempfile::tempdir()?;
            let mut child = launch(
                root,
                &["rebase", "upstream"],
                &[
                    ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                    ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
                ],
            )?;
            wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
            interrupt(&mut child, "-INT")?;
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
            assert_eq!(fs::read(root.join(".git/index"))?, index);
            let journal = fs::read(root.join(".git/brit-rebase/state.json"))?;
            let envelope: serde_json::Value = serde_json::from_slice(&journal)?;
            let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
            let state: serde_json::Value = serde_json::from_slice(&payload)?;
            assert_eq!(state["phase"], "preparing");
            assert_eq!(
                git(root, &["for-each-ref", "refs/brit-rebase-retain"])?
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                    .count(),
                expected_refs
            );
            preserved(root)?;
            // `--abort` cancels without rolling preparation forward, so it adds no
            // ref; interrupted before its cancellation record is published, it
            // has changed nothing, and the next `--abort` finishes.
            let markers = tempfile::tempdir()?;
            let point = "replay-state-file-synced";
            let mut child = launch(
                root,
                &["rebase", "--abort"],
                &[
                    ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                    ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
                ],
            )?;
            wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
            interrupt(&mut child, "-INT")?;
            assert_eq!(
                git(root, &["for-each-ref", "refs/brit-rebase-retain"])?
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                    .count(),
                expected_refs
            );
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
            assert_eq!(fs::read(root.join(".git/index"))?, index);
            assert_eq!(fs::read(root.join(".git/brit-rebase/state.json"))?, journal);
            success(root, &["rebase", "--abort"])?;
            assert!(!root.join(".git/brit-rebase").exists());
            assert!(git(root, &["for-each-ref", "refs/brit-rebase-retain"])?.is_empty());
            preserved(root)?;
        }
        Ok(())
    }

    /// Brit's own leftovers in the Git directory: journal, tombstones, cleanup
    /// receipts, retained copies, scratch directories and retention refs.
    fn assert_no_rebase_leftovers(root: &Path, scratch: &Path) -> Result<()> {
        let mut leftovers = Vec::new();
        for entry in fs::read_dir(root.join(".git"))? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if name.starts_with("brit-rebase") || name.starts_with(".brit-checkout-") {
                leftovers.push(name);
            }
        }
        ensure!(leftovers.is_empty(), "rebase leftovers in .git: {leftovers:?}");
        ensure!(
            git(root, &["for-each-ref", "refs/brit-rebase-retain"])?.is_empty(),
            "retention refs remain"
        );
        let spools = fs::read_dir(scratch)?
            .map(|entry| Ok(entry?.file_name()))
            .collect::<Result<Vec<_>>>()?;
        ensure!(spools.is_empty(), "scratch leftovers in TMPDIR: {spools:?}");
        Ok(())
    }

    fn remove_killed_payload_spool(root: &Path) -> Result<()> {
        let mut spools = Vec::new();
        for entry in fs::read_dir(root.join(".git"))? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with(".brit-checkout-") {
                spools.push(entry.path());
            }
        }
        ensure!(spools.len() == 1, "exactly the killed spool directory: {spools:?}");
        for member in fs::read_dir(&spools[0])? {
            let member = member?;
            ensure!(
                member.file_name().to_string_lossy().starts_with("blob-") && member.path().is_file(),
                "only spool files in {:?}",
                spools[0]
            );
            fs::remove_file(member.path())?;
        }
        fs::remove_dir(&spools[0])?;
        Ok(())
    }

    fn abort_with_scratch(root: &Path, scratch: &Path) -> Result<Capture> {
        BritInvocation::new(brit_bin().expect("fault-enabled artifact"))
            .current_dir(root)
            .args(["rebase", "--abort"])
            .env("TMPDIR", scratch)
            .run()
    }

    /// A preparation that failed at any checkpoint, by interruption or by
    /// process death, is cancelled by `--abort` without being rolled forward:
    /// the abort exits 0, leaves nothing of the operation behind, and the next
    /// rebase works. Before the fix, `--abort` re-ran the failed verification.
    #[test]
    fn public_failed_preparation_always_aborts_to_a_usable_repository() -> Result<()> {
        for point in [
            "replay-before-journal",
            "replay-journal-ready",
            "replay-state-file-synced",
            "replay-state-directory-synced",
            "replay-prepared",
            "replay-payload-intent",
            "replay-payload-persisted",
            "replay-retention-verify",
            "replay-retention-reference",
            "replay-retention-reference-published",
            "replay-retained",
        ] {
            for signal in ["-INT", "-KILL"] {
                let repo = fixture(false)?;
                let root = repo.path();
                let head = git(root, &["rev-parse", "HEAD"])?;
                let index = fs::read(root.join(".git/index"))?;
                let markers = tempfile::tempdir()?;
                let scratch = tempfile::tempdir()?;
                let mut child = launch(
                    root,
                    &["rebase", "upstream"],
                    &[
                        ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                        ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
                        ("TMPDIR", scratch.path().as_os_str()),
                    ],
                )?;
                wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
                interrupt(&mut child, signal)?;
                assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
                assert_eq!(fs::read(root.join(".git/index"))?, index);
                // Before the journal exists there is nothing to abort.
                let journal = root.join(".git/brit-rebase").exists();
                let aborted = abort_with_scratch(root, scratch.path())?;
                ensure!(
                    aborted.status.success() || !journal,
                    "{point} {signal}: --abort must cancel a failed preparation: {}",
                    aborted.stderr
                );
                ensure!(
                    !root.join(".git/brit-rebase").exists(),
                    "{point} {signal}: journal remains"
                );
                if signal == "-KILL" && point.starts_with("replay-payload-") {
                    // Known residual: the spool directory of a process killed while
                    // persisting a payload is named by no journal, so cancellation
                    // cannot claim it. Only that spool may remain; it blocks nothing.
                    remove_killed_payload_spool(root)?;
                }
                assert_no_rebase_leftovers(root, scratch.path())
                    .map_err(|error| error.context(format!("{point} {signal}")))?;
                assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
                assert_eq!(fs::read(root.join(".git/index"))?, index);
                preserved(root)?;
                success(root, &["rebase", "upstream"])?;
                preserved(root)?;
            }
        }
        Ok(())
    }

    /// Cancellation never reads objects: a prepared journal whose pinned
    /// history lost an object still aborts, while `--continue` refuses.
    #[test]
    fn public_preparation_abort_never_reads_the_object_database() -> Result<()> {
        let repo = fixture(false)?;
        let root = repo.path();
        let head = git(root, &["rev-parse", "HEAD"])?;
        let markers = tempfile::tempdir()?;
        let point = "replay-retention-verify";
        let mut child = launch(
            root,
            &["rebase", "upstream"],
            &[
                ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
            ],
        )?;
        wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
        interrupt(&mut child, "-KILL")?;
        // The upstream-only commit is pinned by the journal and reachable from
        // nothing the user changed; its loose object is the one to lose.
        let lost = String::from_utf8(git(root, &["rev-parse", "upstream"])?)?;
        let lost = lost.trim();
        let object = root.join(".git/objects").join(&lost[..2]).join(&lost[2..]);
        ensure!(object.is_file(), "fixture keeps loose objects");
        fs::remove_file(&object)?;
        refused(root, &["rebase", "--continue"])?;
        let aborted = invoke(root, &["rebase", "--abort"])?;
        ensure!(aborted.status.success(), "abort read objects: {}", aborted.stderr);
        assert!(!root.join(".git/brit-rebase").exists());
        assert!(git(root, &["for-each-ref", "refs/brit-rebase-retain"])?.is_empty());
        assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
        preserved(root)
    }

    /// A divergent repository whose shared history holds one `big`-byte blob.
    fn big_blob_fixture(big: usize) -> Result<TestRepo> {
        let repo = TestRepo::new("bounded-rebase")?;
        let root = repo.path();
        fs::write(root.join("big"), vec![0u8; big])?;
        git(root, &["add", "big"])?;
        repo.commit_file("base", "base\n")?;
        git(root, &["switch", "-q", "-c", "upstream"])?;
        repo.commit_file("upstream", "upstream\n")?;
        git(root, &["switch", "-q", "main"])?;
        repo.commit_file("topic", "topic\n")?;
        Ok(repo)
    }

    /// Run a fault-enabled brit command to completion with a lowered scratch
    /// quota, a private TMPDIR, and a rendezvous armed at the first object
    /// verification, which only proves no verification happened if it is unused.
    fn bounded(root: &Path, args: &[&str], limit: &str, markers: &Path, scratch: &Path) -> Result<bool> {
        let mut child = launch(
            root,
            args,
            &[
                ("BRIT_TEST_SCRATCH_LIMIT", limit.as_ref()),
                ("BRIT_SWITCH_CHECKPOINT", "replay-selection-object".as_ref()),
                ("BRIT_SWITCH_CHECKPOINT_DIR", markers.as_os_str()),
                ("TMPDIR", scratch.as_os_str()),
            ],
        )?;
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(status) = child.0.try_wait()? {
                return Ok(status.success());
            }
            ensure!(
                !markers.join("replay-selection-object.reached").exists(),
                "{args:?} verified an object before refusing"
            );
            ensure!(Instant::now() < deadline, "{args:?} did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A rebase whose retained copy cannot fit the scratch quota is refused before
    /// any journal or spool exists, instead of publishing and then being unable to
    /// finish its cleanup or be aborted. The quota is lowered for the test only.
    #[test]
    fn public_rebase_whose_retained_copy_cannot_fit_refuses_before_any_state() -> Result<()> {
        let repo = big_blob_fixture(2_000_000)?;
        let root = repo.path();
        let head = git(root, &["rev-parse", "HEAD"])?;
        let index = fs::read(root.join(".git/index"))?;
        let markers = tempfile::tempdir()?;
        let scratch = tempfile::tempdir()?;
        let mut command = Command::new(brit_bin().expect("fault-enabled artifact"));
        gix_testtools::configure_git_environment(&mut command, root);
        let output = command
            .current_dir(root)
            .args(["rebase", "upstream"])
            .env("BRIT_TEST_SCRATCH_LIMIT", "1000000")
            .env("TMPDIR", scratch.path())
            .stdin(Stdio::null())
            .output()?;
        let message = String::from_utf8_lossy(&output.stderr);
        ensure!(!output.status.success(), "oversized rebase must refuse");
        ensure!(
            message.contains("can need up to")
                && message.contains("the scratch limit configured for testing is too small for this rebase")
                && !message.contains("not available on repositories of this size yet")
                && message.contains("over the 1000000-byte scratch limit")
                && message.contains("nothing was changed")
                && message.contains("git rebase is the fallback"),
            "named refusal: {message}"
        );
        assert_no_rebase_leftovers(root, scratch.path())?;
        // The refusal precedes the first object verification, and so every spool.
        ensure!(!bounded(
            root,
            &["rebase", "upstream"],
            "1000000",
            markers.path(),
            scratch.path()
        )?);
        assert_no_rebase_leftovers(root, scratch.path())?;
        assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
        assert_eq!(fs::read(root.join(".git/index"))?, index);
        success(root, &["branch", "create", "after-refusal"])?;
        // Under the ordinary quota the same rebase proceeds and finishes.
        success(root, &["rebase", "upstream"])?;
        assert!(!root.join(".git/brit-rebase").exists());
        Ok(())
    }

    /// The review's band, journal and mid-copy reproductions (`exp_c_band.sh`,
    /// `exp_c_journal.sh`): a file of S bytes changed by the replayed commit needs
    /// several copies of S in the journal and the retained copy. Under a
    /// 1,000,000-byte quota each such rebase used to pass the preflight and then
    /// strand, at cleanup or inside its first checkout. Each is now refused before
    /// anything changed, with no journal left, and the repository stays usable.
    #[test]
    fn public_rebases_the_quota_cannot_finish_are_refused_before_anything_changes() -> Result<()> {
        for size in [120_000usize, 170_000, 450_000] {
            let repo = TestRepo::new("bounded-band")?;
            let root = repo.path();
            fs::write(root.join("big"), noise(size, 1))?;
            git(root, &["add", "big"])?;
            repo.commit_file("base", "base\n")?;
            git(root, &["switch", "-q", "-c", "upstream"])?;
            repo.commit_file("up", "up\n")?;
            git(root, &["switch", "-q", "main"])?;
            fs::write(root.join("big"), noise(size, 2))?;
            git(root, &["add", "big"])?;
            git(root, &["commit", "-q", "-m", "topic"])?;
            let head = git(root, &["rev-parse", "HEAD"])?;
            let index = fs::read(root.join(".git/index"))?;
            let output = quota(root, &["rebase", "upstream"], "1000000")?;
            let message = String::from_utf8_lossy(&output.stderr);
            ensure!(
                !output.status.success()
                    && message.contains("refused before")
                    && message.contains("nothing was changed"),
                "{size}: clean refusal: {message}"
            );
            ensure!(!root.join(".git/brit-rebase").exists(), "{size}: no journal remains");
            ensure!(
                git(root, &["for-each-ref", "refs/brit-rebase-retain"])?.is_empty(),
                "{size}: no retention ref remains"
            );
            assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
            assert_eq!(fs::read(root.join(".git/index"))?, index);
            assert_eq!(git(root, &["status", "--porcelain"])?, b"");
            success(root, &["branch", "create", "usable"])?;
            success(root, &["rebase", "upstream"])?;
        }
        Ok(())
    }

    /// A divergent repository for the boundary walk: a 400 KB file and 150 small
    /// files in shared history, with `shape` naming what the replayed commit does,
    /// and the user's own unstaged edit of `notes`, which no shape touches.
    fn boundary_fixture(shape: &str) -> Result<TestRepo> {
        let repo = TestRepo::new("bounded-walk")?;
        let root = repo.path();
        fs::write(root.join("big"), noise(400_000, 7))?;
        fs::create_dir(root.join("d"))?;
        for i in 0..150 {
            fs::write(root.join("d").join(format!("f{i}")), format!("file {i}\n"))?;
        }
        fs::write(root.join("a"), b"base\n")?;
        fs::write(root.join("notes"), b"notes\n")?;
        git(root, &["add", "-A"])?;
        git(root, &["commit", "-q", "-m", "base"])?;
        git(root, &["switch", "-q", "-c", "upstream"])?;
        let conflicting = matches!(shape, "conflict" | "resolution" | "skip");
        let upstream_path = if conflicting { "a" } else { "u" };
        fs::write(root.join(upstream_path), b"upstream\n")?;
        git(root, &["add", "-A"])?;
        git(root, &["commit", "-q", "-m", "upstream"])?;
        git(root, &["switch", "-q", "main"])?;
        match shape {
            "large" => fs::write(root.join("big"), noise(400_000, 8))?,
            "many" => {
                for i in 0..150 {
                    fs::write(root.join("d").join(format!("f{i}")), format!("changed {i}\n"))?;
                }
            }
            "steps" => {
                for i in 0..4 {
                    fs::write(root.join(format!("s{i}")), format!("step {i}\n"))?;
                    git(root, &["add", "-A"])?;
                    git(root, &["commit", "-q", "-m", &format!("step {i}")])?;
                }
                fs::write(root.join("t"), b"t\n")?;
            }
            "conflict" | "resolution" | "skip" => fs::write(root.join("a"), b"topic\n")?,
            "rename" => {
                git(root, &["mv", "big", "big2"])?;
            }
            "deletion" => {
                git(root, &["rm", "-q", "big"])?;
            }
            "mode" => fs::set_permissions(root.join("a"), fs::Permissions::from_mode(0o755))?,
            "hook" | "edit" => fs::write(root.join("t"), b"t\n")?,
            _ => anyhow::bail!("unknown shape {shape}"),
        }
        git(root, &["add", "-A"])?;
        git(root, &["commit", "-q", "-m", "topic"])?;
        if shape == "hook" {
            script(&root.join(".git/hooks/pre-rebase"), "exit 0")?;
        }
        if shape == "edit" {
            let topic = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
            script(
                &root.join(".git/sequence-editor"),
                &format!("printf 'edit {}\\n' > \"$1\"", topic.trim()),
            )?;
        }
        fs::write(root.join("notes"), b"notes\nthe user's own unstaged edit\n")?;
        Ok(repo)
    }

    /// Start `shape`'s rebase under `limit` the way that shape stops: a pre-rebase
    /// hook killed after its durable start, an interactive edit, or a plain run.
    fn start_shape(root: &Path, shape: &str, limit: &str) -> Result<std::process::Output> {
        match shape {
            "hook" => {
                let mut child = launch(
                    root,
                    &["rebase", "upstream"],
                    &[
                        ("BRIT_TEST_SCRATCH_LIMIT", limit.as_ref()),
                        ("BRIT_AUTHORING_INVOCATION_PAUSE", "after-launch".as_ref()),
                    ],
                )?;
                let checkpoint = root.join(".git/brit-rebase/pre-rebase/checkpoint");
                let deadline = Instant::now() + Duration::from_secs(60);
                loop {
                    if let Some(status) = child.0.try_wait()? {
                        // Refused at admission, before the hook was ever prepared.
                        return Ok(std::process::Output {
                            status,
                            stdout: Vec::new(),
                            stderr: Vec::new(),
                        });
                    }
                    if checkpoint.exists() {
                        let _cleanup = HookCleanup(root.join(".git/brit-rebase/pre-rebase"));
                        interrupt(&mut child, "-KILL")?;
                        return Ok(std::process::Output {
                            status: child.0.wait()?,
                            stdout: Vec::new(),
                            stderr: Vec::new(),
                        });
                    }
                    ensure!(Instant::now() < deadline, "hook shape did not stop");
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            "edit" => quota_env(
                root,
                &["rebase", "-i", "upstream"],
                limit,
                &[(
                    "GIT_SEQUENCE_EDITOR",
                    root.join(".git/sequence-editor").into_os_string(),
                )],
            ),
            _ => quota(root, &["rebase", "upstream"], limit),
        }
    }

    /// What a user does next at `shape`'s stop, under the same limit; each may be
    /// refused, which must leave `--abort` able to restore everything.
    fn follow_shape(root: &Path, shape: &str, limit: &str) -> Result<()> {
        match shape {
            "resolution" => {
                fs::write(root.join("a"), noise(300_000, 9))?;
                quota(root, &["rebase", "--stage", "--", "a"], limit)?;
                quota(root, &["rebase", "--continue"], limit)?;
            }
            "skip" => {
                quota(root, &["rebase", "--skip"], limit)?;
            }
            "conflict" => {
                quota(root, &["rebase", "--continue"], limit)?;
            }
            "edit" => {
                quota(root, &["rebase", "--continue"], limit)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Walk the lowered limit from well under to well over what each shape needs.
    /// At every limit the outcome is one of three, and no other: refused with
    /// nothing changed; finished, keeping the user's own edit; or stopped, and then
    /// (after the shape's own next step under the same limit, which may itself be
    /// refused) `--abort` under that limit restores HEAD, the index, every branch
    /// and every worktree file exactly and leaves no authoring session.
    #[test]
    fn public_boundary_walk_never_admits_a_rebase_that_cannot_finish_or_abort() -> Result<()> {
        for shape in [
            "large",
            "many",
            "steps",
            "conflict",
            "rename",
            "deletion",
            "mode",
            "resolution",
            "hook",
            "skip",
            "edit",
        ] {
            let mut outcomes = Vec::new();
            let mut limit = 400_000u64;
            while limit <= 40_000_000 {
                let repo = boundary_fixture(shape)?;
                let root = repo.path();
                let before = live(root)?;
                let limit_text = limit.to_string();
                let output = start_shape(root, shape, &limit_text)?;
                let journal = root.join(".git/brit-rebase").exists();
                let outcome = if !journal && output.status.success() {
                    ensure!(
                        fs::read(root.join("notes"))? == b"notes\nthe user's own unstaged edit\n",
                        "{shape} at {limit}: a finished rebase kept the user's edit"
                    );
                    "finished"
                } else if !journal {
                    ensure!(
                        live(root)? == before,
                        "{shape} at {limit}: a refusal changed nothing: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    "refused"
                } else {
                    follow_shape(root, shape, &limit_text)?;
                    if root.join(".git/brit-rebase").exists() {
                        abort_restores(root, &limit_text, &before, &format!("{shape} at {limit}"))?;
                        "aborted"
                    } else {
                        "finished"
                    }
                };
                outcomes.push(outcome);
                limit *= 2;
            }
            // The walk crossed the boundary: some limits refuse, some admit.
            ensure!(
                outcomes.first() == Some(&"refused") && outcomes.last() != Some(&"refused"),
                "{shape}: the walk spans the admission boundary: {outcomes:?}"
            );
        }
        Ok(())
    }

    /// The live state an abort must restore exactly: HEAD, the index's bytes,
    /// every branch, and every worktree file.
    fn live(root: &Path) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>> {
        let mut state: std::collections::BTreeMap<PathBuf, Vec<u8>> = repository_bytes(root)?
            .into_iter()
            .filter(|(path, _)| !path.starts_with(".git"))
            .collect();
        state.insert(PathBuf::from(".git/HEAD"), fs::read(root.join(".git/HEAD"))?);
        state.insert(PathBuf::from(".git/index"), fs::read(root.join(".git/index"))?);
        state.insert(PathBuf::from("branches"), git(root, &["for-each-ref", "refs/heads"])?);
        Ok(state)
    }

    /// `--abort` under `limit` succeeds, restores `before` exactly, and leaves no
    /// rebase journal and no authoring session.
    fn abort_restores(
        root: &Path,
        limit: &str,
        before: &std::collections::BTreeMap<PathBuf, Vec<u8>>,
        what: &str,
    ) -> Result<()> {
        let aborted = quota(root, &["rebase", "--abort"], limit)?;
        ensure!(
            aborted.status.success(),
            "{what}: --abort refused: {}",
            String::from_utf8_lossy(&aborted.stderr)
        );
        ensure!(
            !root.join(".git/brit-rebase").exists() && !root.join(".git/brit-authoring").exists(),
            "{what}: no rebase journal or authoring session remains"
        );
        let after = live(root)?;
        let differing: Vec<_> = before
            .keys()
            .chain(after.keys())
            .filter(|path| before.get(*path) != after.get(*path))
            .collect();
        ensure!(
            differing.is_empty(),
            "{what}: --abort restored exactly; differs: {differing:?}"
        );
        Ok(())
    }

    /// Every kind of Brit mutation works: no journal, session or lock is left over.
    fn mutations_work(root: &Path) -> Result<()> {
        let status = success(root, &["rebase", "--status"])?;
        ensure!(status.stdout.contains("No active rebase"), "{}", status.stdout);
        success(root, &["branch", "create", "after-recovery"])?;
        fs::write(root.join("after-recovery"), b"after\n")?;
        success(root, &["add", "after-recovery"])?;
        success(root, &["commit", "-m", "after recovery"])?;
        success(root, &["switch", "after-recovery"])?;
        success(root, &["switch", "main"])?;
        Ok(())
    }

    /// Deterministic incompressible bytes.
    fn noise(size: usize, seed: u64) -> Vec<u8> {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        (0..size)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect()
    }

    /// R3: a completed journal's cleanup always finishes on `--continue`, whatever
    /// the limit. The cleanup copy is killed as it began and again part-way through
    /// its members; under a 1,000-byte limit, far below the copy, `--continue`
    /// finishes it and leaves one complete copy.
    #[test]
    fn public_cleanup_always_finishes_whatever_the_quota() -> Result<()> {
        for point in ["replay-retained-manifest", "replay-retained-member"] {
            let repo = big_blob_fixture(400_000)?;
            let root = repo.path();
            let original = git(root, &["rev-parse", "main"])?;
            kill_rebase_at(root, point)?;
            let published = git(root, &["rev-parse", "main"])?;
            ensure!(
                published != original,
                "{point}: the rebase published before its cleanup"
            );
            let continued = quota(root, &["rebase", "--continue"], "1000")?;
            ensure!(
                continued.status.success(),
                "{point}: cleanup finished under the quota: {}",
                String::from_utf8_lossy(&continued.stderr)
            );
            assert!(!root.join(".git/brit-rebase").exists());
            retained_leaf(root)?;
            assert_eq!(git(root, &["rev-parse", "main"])?, published);
            mutations_work(root)?;
        }
        Ok(())
    }

    /// The review's `exp_a_moved.sh`, with its truthful outcome: a preparation
    /// journal in a moved repository does not open as this repository's own. No
    /// command touches it: `--status` says what failed and what to do by hand,
    /// every recovery command and mutation refuses, and the journal and its refs
    /// stay byte for byte. Following the printed manual step frees the repository.
    #[test]
    fn public_journal_of_a_moved_repository_is_reported_and_never_touched() -> Result<()> {
        let repo = fixture(false)?;
        kill_rebase_at(repo.path(), "replay-retained")?;
        let parent = tempfile::tempdir()?;
        let moved = parent.path().join("moved");
        fs::rename(repo.path(), &moved)?;
        let journal = repository_bytes(&moved.join(".git/brit-rebase"))?;
        let refs = git(&moved, &["for-each-ref", "refs/brit-rebase-retain"])?;
        let status = success(&moved, &["rebase", "--status"])?;
        for expected in [
            "does not open as this repository's own",
            "identity mismatch",
            "No brit command will change, rename or remove it",
            "ANOTHER",
            "refs/brit-rebase-retain/",
            "git update-ref -d",
        ] {
            ensure!(
                status.stdout.contains(expected),
                "status names {expected}: {}",
                status.stdout
            );
        }
        ensure!(!status.stdout.contains("--retire"), "no false cure: {}", status.stdout);
        for args in [
            &["rebase", "--abort"][..],
            &["rebase", "--continue"],
            &["rebase", "--retire"],
            &["branch", "create", "blocked"],
        ] {
            refused(&moved, args)?;
        }
        assert_eq!(repository_bytes(&moved.join(".git/brit-rebase"))?, journal);
        assert_eq!(git(&moved, &["for-each-ref", "refs/brit-rebase-retain"])?, refs);
        // The manual step the status names.
        fs::rename(
            moved.join(".git/brit-rebase"),
            moved.join(".git/brit-rebase-kept-manual"),
        )?;
        for line in String::from_utf8(refs)?.lines() {
            let name = line.rsplit('\t').next().unwrap_or_default();
            git(&moved, &["update-ref", "-d", name])?;
        }
        success(&moved, &["branch", "create", "unblocked"])?;
        success(&moved, &["rebase", "upstream"])?;
        Ok(())
    }

    /// S2: a repository moved while its rebase was part-way through a checkout.
    /// `--status` names the location the journal records and says that moving it
    /// back and running `--abort` there restores it, which it does, exactly; the
    /// manual steps are offered as the alternative that leaves the checkout
    /// part-way.
    #[test]
    fn public_a_moved_part_way_rebase_names_its_location_and_aborts_once_moved_back() -> Result<()> {
        let repo = fixture(false)?;
        let root = repo.path().to_owned();
        let before = live(&root)?;
        kill_rebase_at(&root, "replay-decision-applied")?;
        let parent = tempfile::tempdir()?;
        let moved = parent.path().join("moved");
        fs::rename(&root, &moved)?;
        let status = success(&moved, &["rebase", "--status"])?;
        for expected in [
            "does not open as this repository's own",
            root.to_str().expect("utf-8 test path"),
            "Moving it back",
            "brit rebase --abort there restores",
            "To set it aside by hand instead",
            "stay part-way",
        ] {
            ensure!(
                status.stdout.contains(expected),
                "status names {expected}: {}",
                status.stdout
            );
        }
        refused(&moved, &["rebase", "--abort"])?;
        fs::rename(&moved, &root)?;
        abort_restores(&root, REAL_LIMIT, &before, "moved back")?;
        usable(&root)?;
        preserved(&root)
    }

    /// The review's `exp2_retire_foreign.sh` (M2): a repository copied while its
    /// rebase was part-way through a checkout, Git locks held. In the copy every
    /// command refuses or only reports, and nothing changes in EITHER repository,
    /// byte for byte, lock files included. The original still aborts exactly.
    #[test]
    fn public_a_copied_part_way_journal_is_touched_by_no_command_in_either_repository() -> Result<()> {
        let repo = big_blob_fixture(400_000)?;
        let root = repo.path();
        user_work(root)?;
        let before = live(root)?;
        kill_rebase_at(root, "replay-worktree-publication")?;
        ensure!(
            root.join(".git/index.lock").exists(),
            "the part-way checkout holds the index lock"
        );
        let parent = tempfile::tempdir()?;
        let copy = parent.path().join("copy");
        ensure!(Command::new("cp").arg("-a").arg(root).arg(&copy).status()?.success());
        let unguarded = |root: &Path| -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>> {
            Ok(repository_bytes(root)?
                .into_iter()
                .filter(|(path, _)| {
                    !path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("brit-mutation."))
                })
                .collect())
        };
        let original_bytes = unguarded(root)?;
        let copy_bytes = unguarded(&copy)?;
        let status = success(&copy, &["rebase", "--status"])?;
        for expected in [
            "does not open as this repository's own",
            "ANOTHER",
            "index.lock",
            "refs/brit-rebase-retain/",
        ] {
            ensure!(
                status.stdout.contains(expected),
                "status names {expected}: {}",
                status.stdout
            );
        }
        for args in [
            &["rebase", "--retire"][..],
            &["rebase", "--abort"],
            &["rebase", "--continue"],
            &["rebase", "--skip"],
            &["branch", "create", "blocked"],
        ] {
            let output = refused(&copy, args)?;
            ensure!(
                !output.stderr.contains("Removed"),
                "{args:?} removed nothing: {}",
                output.stderr
            );
        }
        ensure!(unguarded(&copy)? == copy_bytes, "nothing in the copy changed");
        ensure!(unguarded(root)? == original_bytes, "nothing in the original changed");
        abort_restores(root, "1000", &before, "the original")
    }

    /// The review's `exp2_foreign_misc.sh`: this repository's own part-way journal
    /// with `state.json` at mode 0644, and with a corrupt checksum. `--status`
    /// exits 0 and says exactly what failed, names no command as the cure, and every
    /// command refuses without touching the journal. With the mode restored by hand,
    /// `--abort` restores everything exactly.
    #[test]
    fn public_a_journal_with_a_wrong_mode_or_checksum_is_reported_truthfully() -> Result<()> {
        for case in ["mode", "checksum"] {
            let repo = big_blob_fixture(400_000)?;
            let root = repo.path();
            user_work(root)?;
            let before = live(root)?;
            kill_rebase_at(root, "replay-worktree-publication")?;
            let state = root.join(".git/brit-rebase/state.json");
            if case == "mode" {
                fs::set_permissions(&state, fs::Permissions::from_mode(0o644))?;
            } else {
                let mut envelope: serde_json::Value = serde_json::from_slice(&fs::read(&state)?)?;
                let checksum = envelope["checksum"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("checksum"))?
                    .to_owned();
                let flipped = if checksum.starts_with('0') { "1" } else { "0" };
                envelope["checksum"] = serde_json::Value::from(format!("{flipped}{}", &checksum[1..]));
                fs::write(&state, serde_json::to_vec(&envelope)?)?;
            }
            let journal = repository_bytes(&root.join(".git/brit-rebase"))?;
            let status = success(root, &["rebase", "--status"])?;
            let failure = if case == "mode" {
                "unsafe replay state"
            } else {
                "checksum mismatch"
            };
            for expected in [
                "does not open as this repository's own",
                failure,
                "No brit command will change, rename or remove it",
            ] {
                ensure!(
                    status.stdout.contains(expected),
                    "{case}: status names {expected}: {}",
                    status.stdout
                );
            }
            ensure!(
                !status.stdout.contains("--retire"),
                "{case}: no false cure: {}",
                status.stdout
            );
            for args in [
                &["rebase", "--retire"][..],
                &["rebase", "--abort"],
                &["rebase", "--continue"],
                &["branch", "create", "blocked"],
            ] {
                refused(root, args)?;
            }
            assert_eq!(repository_bytes(&root.join(".git/brit-rebase"))?, journal, "{case}");
            if case == "mode" {
                fs::set_permissions(&state, fs::Permissions::from_mode(0o600))?;
                abort_restores(root, "1000", &before, case)?;
                mutations_work(root)?;
            }
        }
        Ok(())
    }

    /// The user's own uncommitted work beside a `big_blob_fixture` rebase: an
    /// unstaged edit and a staged change, neither touched by the replay.
    fn user_work(root: &Path) -> Result<()> {
        fs::write(root.join("base"), b"base\nthe user's unstaged edit\n")?;
        fs::write(root.join("staged"), b"the user's staged file\n")?;
        git(root, &["add", "staged"])?;
        Ok(())
    }

    /// An older Brit captured gitlinks into a preparation it could not finish. The
    /// journal is produced here by recording one in a real preparation journal (its
    /// checksum recomputed as Brit computes it): `--continue` refuses by name and
    /// `--abort` cancels it.
    #[test]
    fn public_older_preparation_recording_a_gitlink_refuses_continue_and_cancels() -> Result<()> {
        let repo = fixture(false)?;
        let root = repo.path();
        kill_rebase_at(root, "replay-retention-verify")?;
        let path = root.join(".git/brit-rebase/state.json");
        let envelope: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
        let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
        let mut state: serde_json::Value = serde_json::from_slice(&payload)?;
        state["preservation"]["original"]
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("preservation entries"))?
            .push(serde_json::json!({
                "path": b"sub".to_vec(),
                "id": "1111111111111111111111111111111111111111",
                "mode": 0o160000,
                "flags": 0
            }));
        let payload = serde_json::to_vec(&state)?;
        let scratch = tempfile::tempdir()?;
        fs::write(scratch.path().join("payload"), &payload)?;
        let sum = Command::new("sha1sum").arg(scratch.path().join("payload")).output()?;
        let checksum = String::from_utf8(sum.stdout)?
            .split(' ')
            .next()
            .unwrap_or_default()
            .to_owned();
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({ "payload": payload, "checksum": checksum }))?,
        )?;
        let continued = refused(root, &["rebase", "--continue"])?;
        ensure!(
            continued.stderr.contains("gitlink sub is unsupported") && continued.stderr.contains("--abort"),
            "{}",
            continued.stderr
        );
        let status = success(root, &["rebase", "--status"])?;
        ensure!(
            !status.stdout.contains("--continue retries"),
            "status names only --abort: {}",
            status.stdout
        );
        success(root, &["rebase", "--abort"])?;
        assert!(!root.join(".git/brit-rebase").exists());
        preserved(root)
    }

    /// Kill `brit rebase upstream` once it reaches a switch checkpoint.
    fn kill_rebase_at(root: &Path, point: &str) -> Result<()> {
        let markers = tempfile::tempdir()?;
        let mut child = launch(
            root,
            &["rebase", "upstream"],
            &[
                ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
            ],
        )?;
        wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
        interrupt(&mut child, "-KILL")
    }

    fn quota(root: &Path, args: &[&str], limit: &str) -> Result<std::process::Output> {
        quota_env(root, args, limit, &[])
    }

    fn quota_env(
        root: &Path,
        args: &[&str],
        limit: &str,
        variables: &[(&str, std::ffi::OsString)],
    ) -> Result<std::process::Output> {
        let mut command = Command::new(brit_bin().expect("fault-enabled artifact"));
        gix_testtools::configure_git_environment(&mut command, root);
        Ok(command
            .current_dir(root)
            .args(args)
            .env("BRIT_TEST_SCRATCH_LIMIT", limit)
            .env("GIT_EDITOR", ":")
            .envs(variables.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .output()?)
    }

    /// Every file under `.git` except the guard's own files, by path.
    fn git_files(root: &Path) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>> {
        Ok(repository_bytes(&root.join(".git"))?
            .into_iter()
            .filter(|(path, _)| !path.to_string_lossy().starts_with("brit-mutation."))
            .collect())
    }

    /// After `--retire`, every file that existed is still there: the
    /// journal's under `brit-rebase-kept-<nonce>`, everything else unchanged.
    fn assert_all_kept(
        before: &std::collections::BTreeMap<PathBuf, Vec<u8>>,
        after: &std::collections::BTreeMap<PathBuf, Vec<u8>>,
        kept: &Path,
    ) -> Result<()> {
        for (path, bytes) in before {
            let moved = match path.strip_prefix("brit-rebase") {
                Ok(rest) => kept.join(rest),
                Err(_) => path.clone(),
            };
            ensure!(after.get(&moved) == Some(bytes), "{path:?} was not kept as {moved:?}");
        }
        Ok(())
    }

    fn kept_directory(root: &Path) -> Result<PathBuf> {
        let kept = fs::read_dir(root.join(".git"))?
            .map(|entry| Ok(entry?.file_name()))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|name| name.to_string_lossy().starts_with("brit-rebase-kept-"))
            .collect::<Vec<_>>();
        ensure!(kept.len() == 1, "one kept journal: {kept:?}");
        Ok(PathBuf::from(&kept[0]))
    }

    /// `--retire` sets aside a published rebase whose cleanup copy is all that
    /// remains (here killed as that copy began; on a large repository it can take
    /// long). It refuses while another process holds the guard, deletes nothing,
    /// changes nothing live, removes no lock and gives no checkout advice, and is
    /// idempotent; afterwards a commit, a switch and a fresh rebase all succeed.
    #[test]
    fn public_retire_sets_aside_a_completed_rebase_keeping_everything() -> Result<()> {
        let repo = big_blob_fixture(2_000_000)?;
        let root = repo.path();
        let original = git(root, &["rev-parse", "main"])?;
        kill_rebase_at(root, "replay-retained-manifest")?;
        let published = git(root, &["rev-parse", "main"])?;
        assert_ne!(published, original, "the rebase published");
        refused(root, &["rebase", "--abort"])?;
        refused(root, &["branch", "create", "blocked"])?;
        let refs = git(root, &["for-each-ref", "refs/brit-rebase-retain"])?;
        let before = git_files(root)?;
        let worktree = fs::read(root.join("topic"))?;

        // While another process holds the guard, nothing moves.
        let guard = String::from_utf8(git(root, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?)?;
        let guard = PathBuf::from(guard.trim()).join("brit-mutation.guard");
        let mut holder = Command::new("flock")
            .arg(&guard)
            .args(["sh", "-c", "printf ready; cat"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut ready = [0; 5];
        std::io::Read::read_exact(&mut holder.stdout.take().expect("piped"), &mut ready)?;
        let held = invoke(root, &["rebase", "--retire"])?;
        drop(holder.stdin.take());
        holder.wait()?;
        ensure!(!held.status.success(), "retire waits for nobody: {}", held.stdout);
        assert!(root.join(".git/brit-rebase").exists());

        let retired = success(root, &["rebase", "--retire"])?;
        for expected in [
            "nothing was deleted",
            "published at",
            "Kept journal",
            "rm -rf",
            "git update-ref -d refs/brit-rebase-retain/",
        ] {
            ensure!(
                retired.stdout.contains(expected),
                "report names {expected}: {}",
                retired.stdout
            );
        }
        for absent in ["checkout", "lock"] {
            ensure!(
                !retired.stdout.contains(absent),
                "no {absent} advice: {}",
                retired.stdout
            );
        }
        let kept = kept_directory(root)?;
        assert_all_kept(&before, &git_files(root)?, &kept)?;
        assert_eq!(git(root, &["for-each-ref", "refs/brit-rebase-retain"])?, refs);
        assert_eq!(git(root, &["rev-parse", "main"])?, published);
        assert_eq!(fs::read(root.join("topic"))?, worktree);
        let again = success(root, &["rebase", "--retire"])?;
        ensure!(again.stdout.contains("No rebase to retire"), "{}", again.stdout);
        let status = success(root, &["rebase", "--status"])?;
        ensure!(
            status.stdout.contains("No active rebase") && status.stdout.matches("--retire").count() == 1,
            "status mentions the kept material once: {}",
            status.stdout
        );

        // Every mutation works again.
        fs::write(root.join("after"), b"after\n")?;
        success(root, &["add", "after"])?;
        success(root, &["commit", "-m", "after retire"])?;
        success(root, &["switch", "upstream"])?;
        success(root, &["switch", "main"])?;
        git(root, &["switch", "-q", "-c", "small", "upstream"])?;
        fs::write(root.join("small"), b"small\n")?;
        git(root, &["add", "small"])?;
        git(root, &["commit", "-q", "-m", "small"])?;
        success(root, &["rebase", "main"])?;
        assert!(!root.join(".git/brit-rebase").exists());
        // The later commands changed HEAD and the index; the kept material is intact.
        let operation = before
            .into_iter()
            .filter(|(path, _)| path.starts_with("brit-rebase") || path.starts_with("brit-rebase-retained"))
            .collect();
        assert_all_kept(&operation, &git_files(root)?, &kept)?;
        assert!(git(root, &["for-each-ref", "refs/brit-rebase-retain"])?.starts_with(&refs));
        Ok(())
    }

    /// An abort that stops while retaining, before the rebase's first checkout
    /// decision (here after a pre-rebase hook's durable start), changed nothing
    /// live: `--retire` sets it aside, keeping everything, and the branch and the
    /// user's work are as they were. A conflict stop, where `--abort` has live
    /// state to restore, refuses `--retire` and names `--abort`.
    #[test]
    fn public_retire_takes_an_abort_retaining_before_its_first_decision() -> Result<()> {
        let repo = big_blob_fixture(400_000)?;
        let root = repo.path();
        script(&root.join(".git/hooks/pre-rebase"), "exit 0")?;
        let original = git(root, &["rev-parse", "main"])?;
        let mut child = launch(
            root,
            &["rebase", "upstream"],
            &[("BRIT_AUTHORING_INVOCATION_PAUSE", "after-start".as_ref())],
        )?;
        wait_file(&mut child, &root.join(".git/brit-rebase/pre-rebase/checkpoint"))?;
        interrupt(&mut child, "-KILL")?;
        let markers = tempfile::tempdir()?;
        let mut abort = launch(
            root,
            &["rebase", "--abort"],
            &[
                ("BRIT_SWITCH_CHECKPOINT", "replay-retained-manifest".as_ref()),
                ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
            ],
        )?;
        wait_file(&mut abort, &markers.path().join("replay-retained-manifest.reached"))?;
        interrupt(&mut abort, "-KILL")?;
        let status = success(root, &["rebase", "--status"])?;
        ensure!(status.stdout.contains("retaining-abort"), "{}", status.stdout);
        refused(root, &["branch", "create", "blocked"])?;
        let before = git_files(root)?;
        let retired = success(root, &["rebase", "--retire"])?;
        ensure!(retired.stdout.contains("did not publish"), "{}", retired.stdout);
        assert_all_kept(&before, &git_files(root)?, &kept_directory(root)?)?;
        assert_eq!(git(root, &["rev-parse", "main"])?, original);
        preserved_neighbors_unchanged(root)?;
        success(root, &["branch", "create", "unblocked"])?;

        let repo = fixture(true)?;
        let root = repo.path();
        refused(root, &["rebase", "upstream"])?;
        let conflict = invoke(root, &["rebase", "--retire"])?;
        ensure!(
            !conflict.status.success() && conflict.stderr.contains("--abort"),
            "{}",
            conflict.stderr
        );
        assert!(root.join(".git/brit-rebase").exists());
        success(root, &["rebase", "--abort"])?;
        preserved(root)
    }

    /// The review's `exp2_retire_vs_abort.sh` (M3): wherever `--abort` can still
    /// restore the original state, `--retire` refuses and names it, `--status`
    /// never steers to `--retire`, and nothing of the journal changes, at any
    /// limit; then `--abort`, under the lowest limit, restores HEAD, the index,
    /// every branch and every worktree file, including the user's own uncommitted
    /// work, exactly.
    #[test]
    fn public_retire_refuses_every_rebase_abort_can_restore() -> Result<()> {
        for point in [
            "replay-decision-durable",
            "replay-worktree-publication",
            "replay-decision-applied",
        ] {
            let repo = big_blob_fixture(400_000)?;
            let root = repo.path();
            user_work(root)?;
            let before = live(root)?;
            kill_rebase_at(root, point)?;
            let journal = repository_bytes(&root.join(".git/brit-rebase"))?;
            for limit in ["1000000", "1000"] {
                let status = quota(root, &["rebase", "--status"], limit)?;
                ensure!(
                    status.status.success() && !String::from_utf8_lossy(&status.stdout).contains("--retire"),
                    "{point}: status does not steer to --retire: {}",
                    String::from_utf8_lossy(&status.stdout)
                );
                let retire = quota(root, &["rebase", "--retire"], limit)?;
                ensure!(
                    !retire.status.success() && String::from_utf8_lossy(&retire.stderr).contains("--abort"),
                    "{point} at {limit}: retire refuses, naming --abort: {}",
                    String::from_utf8_lossy(&retire.stderr)
                );
                ensure!(
                    repository_bytes(&root.join(".git/brit-rebase"))? == journal,
                    "{point}: retire changed nothing"
                );
            }
            abort_restores(root, "1000", &before, point)?;
            mutations_work(root)?;
        }
        Ok(())
    }

    /// A rebase stopped at a conflict, in the review's `exp2_conflict_resolution.sh`
    /// fixture: a three-line file both sides change, the user's own unstaged edit
    /// and staged change beside it.
    fn conflict_fixture() -> Result<TestRepo> {
        let repo = TestRepo::new("bounded-conflict")?;
        let root = repo.path();
        fs::write(root.join("c"), b"a\nb\nc\n")?;
        fs::write(root.join("base"), b"base\n")?;
        git(root, &["add", "c", "base"])?;
        git(root, &["commit", "-q", "-m", "base"])?;
        git(root, &["switch", "-q", "-c", "upstream"])?;
        fs::write(root.join("c"), b"a\nUP\nc\n")?;
        git(root, &["commit", "-q", "-am", "upstream"])?;
        git(root, &["switch", "-q", "main"])?;
        fs::write(root.join("c"), b"a\nTOPIC\nc\n")?;
        git(root, &["commit", "-q", "-am", "topic"])?;
        fs::write(root.join("base"), b"base\nthe user's unstaged edit\n")?;
        fs::write(root.join("staged"), b"the user's staged file\n")?;
        git(root, &["add", "staged"])?;
        Ok(repo)
    }

    /// The review's `exp2_conflict_sweep.sh` (M1): under a 1 MB limit, a staged
    /// resolution of 200 B, 100 KB, 200 KB or 700 KB, with or without a
    /// `--continue` first (which may be refused, naming `--abort`), never leaves the
    /// rebase stuck: `--abort` restores everything exactly, no authoring session
    /// remains, and every mutation works. A continue that is admitted finishes.
    #[test]
    fn public_any_staged_resolution_aborts_exactly_under_the_quota() -> Result<()> {
        for size in [200usize, 100_000, 200_000, 700_000] {
            for continued in [false, true] {
                let what = format!("{size} bytes, continued first: {continued}");
                let repo = conflict_fixture()?;
                let root = repo.path();
                let before = live(root)?;
                let stopped = quota(root, &["rebase", "upstream"], "1000000")?;
                ensure!(
                    !stopped.status.success() && root.join(".git/brit-authoring").exists(),
                    "{what}: the rebase stops at its conflict: {}",
                    String::from_utf8_lossy(&stopped.stderr)
                );
                fs::write(root.join("c"), noise(size, 3))?;
                let staged = quota(root, &["rebase", "--stage", "--", "c"], "1000000")?;
                ensure!(
                    staged.status.success() || String::from_utf8_lossy(&staged.stderr).contains("--abort"),
                    "{what}: a refused stage names --abort: {}",
                    String::from_utf8_lossy(&staged.stderr)
                );
                if continued {
                    let output = quota(root, &["rebase", "--continue"], "1000000")?;
                    if output.status.success() {
                        ensure!(!root.join(".git/brit-rebase").exists(), "{what}: finished");
                        ensure!(fs::read(root.join("base"))? == b"base\nthe user's unstaged edit\n");
                        mutations_work(root)?;
                        continue;
                    }
                    ensure!(
                        String::from_utf8_lossy(&output.stderr).contains("--abort"),
                        "{what}: a refused continue names --abort: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                abort_restores(root, "1000000", &before, &what)?;
                mutations_work(root)?;
            }
        }
        Ok(())
    }

    /// R2: admission reserves what the first forward step after a stop needs. At
    /// the smallest limit that admits the conflict fixture (walked up in 5% steps),
    /// a trivial resolution stages and continues to the end.
    #[test]
    fn public_a_trivial_resolution_continues_at_the_smallest_admitted_limit() -> Result<()> {
        let mut limit = 200_000u64;
        loop {
            ensure!(limit < 20_000_000, "the conflict fixture was never admitted");
            let repo = conflict_fixture()?;
            let root = repo.path();
            let text = limit.to_string();
            let started = quota(root, &["rebase", "upstream"], &text)?;
            if !root.join(".git/brit-rebase").exists() {
                ensure!(
                    String::from_utf8_lossy(&started.stderr).contains("refused before"),
                    "{limit}: {}",
                    String::from_utf8_lossy(&started.stderr)
                );
                limit = limit * 21 / 20;
                continue;
            }
            fs::write(root.join("c"), b"a\nresolved\nc\n")?;
            let staged = quota(root, &["rebase", "--stage", "--", "c"], &text)?;
            ensure!(
                staged.status.success(),
                "{limit}: stage: {}",
                String::from_utf8_lossy(&staged.stderr)
            );
            let continued = quota(root, &["rebase", "--continue"], &text)?;
            ensure!(
                continued.status.success() && !root.join(".git/brit-rebase").exists(),
                "{limit}: the first forward step after admission continues: {}",
                String::from_utf8_lossy(&continued.stderr)
            );
            assert_eq!(git(root, &["show", "HEAD:c"])?, b"a\nresolved\nc\n");
            return Ok(());
        }
    }

    /// R3, bounded only by the disk: an abort whose retained copy hits `ENOSPC`
    /// part-way says the disk is full, keeps the journal resumable, and the same
    /// `--abort`, once space is back, restores everything exactly.
    #[test]
    fn public_an_abort_that_runs_out_of_disk_resumes_and_restores_exactly() -> Result<()> {
        let repo = big_blob_fixture(400_000)?;
        let root = repo.path();
        user_work(root)?;
        let before = live(root)?;
        kill_rebase_at(root, "replay-decision-applied")?;
        let full = quota_env(
            root,
            &["rebase", "--abort"],
            "1000",
            &[("BRIT_REPLAY_IO_FAULT", "write|28|brit-rebase-retained|3".into())],
        )?;
        let message = String::from_utf8_lossy(&full.stderr);
        ensure!(
            !full.status.success() && message.contains("disk is full") && message.contains("run the same"),
            "{message}"
        );
        ensure!(root.join(".git/brit-rebase").exists(), "the journal remains to resume");
        abort_restores(root, "1000", &before, "after the disk filled")?;
        mutations_work(root)
    }

    /// The built-in scratch limit: the quota a real run has.
    const REAL_LIMIT: &str = "1073741824";

    /// Run `args` and SIGKILL it the `ordinal`th time it reaches `point`. Returns
    /// `false` when it finished without reaching it that often.
    fn kill_at_ordinal(root: &Path, args: &[&str], point: &str, ordinal: usize) -> Result<bool> {
        let markers = tempfile::tempdir()?;
        let wanted = if ordinal == 1 {
            point.to_owned()
        } else {
            format!("{point}#{ordinal}")
        };
        let mut child = launch(
            root,
            args,
            &[
                ("BRIT_SWITCH_CHECKPOINT", wanted.as_ref()),
                ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
            ],
        )?;
        let marker = markers.path().join(format!("{point}.reached"));
        let deadline = Instant::now() + Duration::from_secs(60);
        while !marker.exists() {
            if child.0.try_wait()?.is_some() {
                return Ok(false);
            }
            ensure!(Instant::now() < deadline, "{args:?}: {wanted} timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        interrupt(&mut child, "-KILL")?;
        Ok(true)
    }

    /// No rebase remains and Brit mutations work (the fixtures' own uncommitted
    /// work may overlap a switch, so this commits on a new branch instead).
    fn usable(root: &Path) -> Result<()> {
        let status = success(root, &["rebase", "--status"])?;
        ensure!(status.stdout.contains("No active rebase"), "{}", status.stdout);
        success(root, &["branch", "create", "after-recovery"])?;
        Ok(())
    }

    /// A rebase about to run one forward command, and the live state from
    /// before the rebase began (its uncommitted work included).
    struct Forward {
        repo: TestRepo,
        before: std::collections::BTreeMap<PathBuf, Vec<u8>>,
        command: &'static [&'static str],
    }

    fn forward(scenario: &str) -> Result<Forward> {
        let edit = scenario.starts_with("edit");
        let repo = if edit { fixture(false)? } else { conflict_fixture()? };
        let root = repo.path();
        let before = live(root)?;
        let command: &'static [&'static str] = match scenario {
            "first run" => &["rebase", "upstream"],
            "conflict --continue" => {
                refused(root, &["rebase", "upstream"])?;
                fs::write(root.join("c"), b"a\nRESOLVED\nc\n")?;
                success(root, &["rebase", "--stage", "--", "c"])?;
                &["rebase", "--continue"]
            }
            "conflict --skip" => {
                refused(root, &["rebase", "upstream"])?;
                &["rebase", "--skip"]
            }
            "edit --continue" | "edit --amend" => {
                let candidates = ids(root)?;
                let started = interactive(root, &format!("edit {}\npick {}\n", candidates[0], candidates[1]))?;
                ensure!(started.status.success(), "{}", started.stderr);
                if scenario == "edit --amend" {
                    &["rebase", "--amend", "-m", "amended at the edit stop"]
                } else {
                    &["rebase", "--continue"]
                }
            }
            other => anyhow::bail!("unknown scenario {other}"),
        };
        Ok(Forward { repo, before, command })
    }

    /// Carry a rebase on to its end with `--continue`, resolving a conflict stop
    /// the way the user would. The user's uncommitted work survives.
    fn finish(root: &Path, what: &str) -> Result<()> {
        for _ in 0..6 {
            if !root.join(".git/brit-rebase").exists() {
                ensure!(!root.join(".git/brit-authoring").exists(), "{what}: no authoring left");
                return usable(root);
            }
            let journal = root.join(".git/brit-rebase");
            if !journal.join("state.json").exists() && !journal.join("state.pending").exists() {
                // Killed before preparation recorded anything: there is nothing to
                // continue; --abort removes it and the rebase starts again.
                let continued = refused(root, &["rebase", "--continue"])?;
                ensure!(
                    continued.stderr.contains("--abort"),
                    "{what}: names --abort: {}",
                    continued.stderr
                );
                success(root, &["rebase", "--abort"])?;
                invoke(root, &["rebase", "upstream"])?;
                continue;
            }
            let status = success(root, &["rebase", "--status"])?;
            if status.stdout.contains("Conflict stop") {
                fs::write(root.join("c"), b"a\nRESOLVED\nc\n")?;
                success(root, &["rebase", "--stage", "--", "c"])?;
            }
            let continued = invoke(root, &["rebase", "--continue"])?;
            ensure!(
                continued.status.success()
                    || (root.join(".git/brit-authoring").exists()
                        && success(root, &["rebase", "--status"])?.stdout.contains(" stop")),
                "{what}: --continue went on: {}\n{}",
                continued.stdout,
                continued.stderr
            );
        }
        anyhow::bail!("{what}: --continue never finished the rebase")
    }

    /// The review's `exp3_pending.sh` (M1), swept: every forward command the
    /// fault build drives is killed at every journal save it makes, before its
    /// successor record is written, after it is written and synchronized but
    /// before its rename, and after the rename. After each crash `--status` names
    /// a cure, `--abort` restores HEAD, the index, the worktree and the user's
    /// uncommitted work exactly, and, from the same crash run again, `--continue`
    /// finishes the rebase.
    fn sweep_forward(scenario: &str) -> Result<()> {
        for point in [
            "replay-state-file-writing",
            "replay-state-file-synced",
            "replay-state-directory-synced",
        ] {
            let mut ordinal = 1;
            loop {
                ensure!(ordinal < 60, "{scenario}: {point} reached more than 60 times");
                let what = format!("{scenario}, killed at {point} #{ordinal}");
                let abort = forward(scenario)?;
                let root = abort.repo.path();
                if !kill_at_ordinal(root, abort.command, point, ordinal)? {
                    break;
                }
                let mut published = false;
                if root.join(".git/brit-rebase").exists() {
                    let status = success(root, &["rebase", "--status"])?;
                    ensure!(
                        status.stdout.contains("--abort") || status.stdout.contains("--continue"),
                        "{what}: --status names a cure: {}",
                        status.stdout
                    );
                    published = status.stdout.contains("cannot be aborted");
                }
                if published {
                    // Its final decision is durable: by design only --continue.
                    refused(root, &["rebase", "--abort"])?;
                    finish(root, &what)?;
                } else {
                    abort_restores(root, REAL_LIMIT, &abort.before, &what)?;
                    usable(root)?;
                }
                let carry = forward(scenario)?;
                let root = carry.repo.path();
                ensure!(
                    kill_at_ordinal(root, carry.command, point, ordinal)?,
                    "{what}: replayed"
                );
                finish(root, &what)?;
                if !scenario.starts_with("edit") {
                    ensure!(
                        fs::read(root.join("base"))? == b"base\nthe user's unstaged edit\n"
                            && git(root, &["show", ":staged"])? == b"the user's staged file\n",
                        "{what}: the user's work survived"
                    );
                }
                ordinal += 1;
            }
        }
        Ok(())
    }

    #[test]
    fn public_a_kill_in_the_first_run_never_strands_the_rebase() -> Result<()> {
        sweep_forward("first run")
    }

    #[test]
    fn public_a_kill_in_a_conflict_continue_never_strands_the_rebase() -> Result<()> {
        sweep_forward("conflict --continue")
    }

    #[test]
    fn public_a_kill_in_a_conflict_skip_never_strands_the_rebase() -> Result<()> {
        sweep_forward("conflict --skip")
    }

    #[test]
    fn public_a_kill_in_edit_stop_commands_never_strands_the_rebase() -> Result<()> {
        sweep_forward("edit --continue")?;
        sweep_forward("edit --amend")
    }

    /// The review's reproduction exactly (M1): `--continue` after a staged
    /// resolution, SIGKILLed once its successor record is synchronized and before
    /// its rename. Every recovery command used to refuse "foreign pending replay
    /// state" and every mutation refused.
    #[test]
    fn public_a_continue_killed_before_its_rename_aborts_or_continues() -> Result<()> {
        for then in ["--abort", "--continue"] {
            let stop = forward("conflict --continue")?;
            let root = stop.repo.path();
            ensure!(kill_at_ordinal(root, stop.command, "replay-state-file-synced", 1)?);
            ensure!(root.join(".git/brit-rebase/state.pending").exists());
            let status = success(root, &["rebase", "--status"])?;
            ensure!(
                status.stdout.contains("never took effect") && status.stdout.contains("discards it"),
                "{}",
                status.stdout
            );
            if then == "--abort" {
                abort_restores(root, REAL_LIMIT, &stop.before, "abort after the kill")?;
                mutations_work(root)?;
            } else {
                success(root, &["rebase", "--continue"])?;
                ensure!(!root.join(".git/brit-rebase").exists());
                assert_eq!(git(root, &["show", "HEAD:c"])?, b"a\nRESOLVED\nc\n");
                mutations_work(root)?;
            }
        }
        Ok(())
    }

    /// The newest copy under `.git/brit-rebase-retained` not yet complete.
    fn incomplete_leaf(root: &Path) -> Result<PathBuf> {
        let mut leaves = Vec::new();
        for operation in fs::read_dir(root.join(".git/brit-rebase-retained"))? {
            for copy in fs::read_dir(operation?.path())? {
                let path = copy?.path();
                if !path.join("COPY_COMPLETE").exists() {
                    leaves.push(path);
                }
            }
        }
        ensure!(leaves.len() == 1, "one incomplete copy: {leaves:?}");
        Ok(leaves.remove(0))
    }

    fn truncate(path: &Path, length: u64) -> Result<()> {
        fs::OpenOptions::new().write(true).open(path)?.set_len(length)?;
        Ok(())
    }

    /// The review's `exp3_abort_kill.sh` (M2): `--abort` at a conflict stop,
    /// SIGKILLed once its retained copy's manifest is written, the manifest then
    /// cut short (as a kill inside a non-atomic write, or a loss of power before
    /// it was synchronized, would leave it), and the marker of a completed copy
    /// likewise. The next `--abort` writes them again and restores exactly.
    #[test]
    fn public_a_torn_retained_manifest_or_marker_never_strands_an_abort() -> Result<()> {
        for (record, point) in [
            ("manifest.json", "replay-retained-manifest"),
            ("COPY_COMPLETE", "replay-copy-complete"),
        ] {
            for cut in ["0", "1", "half", "one short"] {
                let what = format!("{record} cut to {cut}");
                let stop = forward("conflict --skip")?;
                let root = stop.repo.path();
                ensure!(kill_at_ordinal(root, &["rebase", "--abort"], point, 1)?, "{what}");
                let leaf = if record == "COPY_COMPLETE" {
                    retained_leaf(root)?
                } else {
                    incomplete_leaf(root)?
                };
                let length = fs::metadata(leaf.join(record))?.len();
                truncate(
                    &leaf.join(record),
                    match cut {
                        "0" => 0,
                        "1" => 1,
                        "half" => length / 2,
                        _ => length - 1,
                    },
                )?;
                abort_restores(root, REAL_LIMIT, &stop.before, &what)?;
                mutations_work(root)?;
            }
        }
        Ok(())
    }

    /// The same shape for the abort's own records in the journal: its decision
    /// record (the abort's commit point) and its staged proposal, cut short after
    /// they were written, and an abort killed holding its transition's Git locks
    /// before its decision. None was ever in effect; the next `--abort` restores
    /// exactly (an unproved staging directory is kept as evidence, as an absent
    /// owner record always was, and `--retire` then sets the journal aside), and
    /// `--status` never calls a torn decision durable.
    #[test]
    fn public_a_torn_abort_decision_or_proposal_never_strands_an_abort() -> Result<()> {
        for (record, point) in [
            ("decision", "replay-decision-durable"),
            ("plan.json", "replay-directories-prepared"),
            ("owner", "replay-directories-prepared"),
            ("nothing", "lock-intent"),
            ("nothing", "lock-acquired"),
        ] {
            for cut in ["0", "half", "one short"] {
                if record == "nothing" && cut != "0" {
                    continue;
                }
                let what = format!("abort {record} cut to {cut}");
                let stop = forward("conflict --skip")?;
                let root = stop.repo.path();
                ensure!(kill_at_ordinal(root, &["rebase", "--abort"], point, 1)?, "{what}");
                let journal = root.join(".git/brit-rebase");
                let mut candidates = Vec::new();
                for entry in fs::read_dir(&journal)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let modified = entry.metadata()?.modified()?;
                    if record == "decision" && name.starts_with("decision-") {
                        candidates.push((modified, entry.path()));
                    } else if record != "decision" && name.starts_with("transition-") {
                        candidates.push((modified, entry.path().join(record)));
                    }
                }
                candidates.sort();
                if record != "nothing" {
                    let path = candidates.pop().map(|(_, path)| path).expect("the abort's record");
                    if record == "owner" {
                        // An owner record cut short means its plan was never begun.
                        fs::remove_file(path.with_file_name("plan.json"))?;
                    }
                    let length = fs::metadata(&path)?.len();
                    truncate(
                        &path,
                        match cut {
                            "0" => 0,
                            "half" => length / 2,
                            _ => length - 1,
                        },
                    )?;
                }
                let status = success(root, &["rebase", "--status"])?;
                ensure!(
                    !status.stdout.contains("Durable"),
                    "{what}: a torn record is not durable: {}",
                    status.stdout
                );
                if matches!(record, "plan.json" | "owner") {
                    // The abort's own staging is unproved without both records, so
                    // it is kept as evidence, not adopted: the abort restores
                    // exactly, cleanup reports the member, and --retire sets the
                    // journal aside, deleting nothing.
                    quota(root, &["rebase", "--abort"], REAL_LIMIT)?;
                    ensure!(live(root)? == stop.before, "{what}: --abort restored exactly");
                    let status = success(root, &["rebase", "--status"])?;
                    ensure!(status.stdout.contains("--retire"), "{what}: {}", status.stdout);
                    success(root, &["rebase", "--retire"])?;
                    ensure!(!root.join(".git/brit-authoring").exists(), "{what}");
                    usable(root)?;
                    continue;
                }
                abort_restores(root, REAL_LIMIT, &stop.before, &what)?;
                mutations_work(root)?;
            }
        }
        Ok(())
    }

    /// S3: the pending-decision line says what is true of the decision actually
    /// pending. A durable checkout decision is aborted (it is applied, then
    /// undone); the durable final decision is not, and `--continue` publishes it.
    #[test]
    fn public_status_names_the_cure_for_the_decision_actually_pending() -> Result<()> {
        let repo = fixture(false)?;
        let root = repo.path();
        let before = live(root)?;
        ensure!(kill_at_ordinal(
            root,
            &["rebase", "upstream"],
            "replay-decision-durable",
            1
        )?);
        let status = success(root, &["rebase", "--status"])?;
        ensure!(
            status.stdout.contains("--abort applies it") && !status.stdout.contains("cannot be aborted"),
            "{}",
            status.stdout
        );
        abort_restores(root, REAL_LIMIT, &before, "non-final decision")?;
        let mut last = 0;
        for ordinal in 1..40 {
            let repo = fixture(false)?;
            if !kill_at_ordinal(repo.path(), &["rebase", "upstream"], "replay-decision-durable", ordinal)? {
                last = ordinal - 1;
                break;
            }
        }
        ensure!(last > 1, "several decisions");
        let repo = fixture(false)?;
        let root = repo.path();
        ensure!(kill_at_ordinal(
            root,
            &["rebase", "upstream"],
            "replay-decision-durable",
            last
        )?);
        let status = success(root, &["rebase", "--status"])?;
        ensure!(
            status.stdout.contains("final decision pending") && status.stdout.contains("cannot be aborted"),
            "{}",
            status.stdout
        );
        refused(root, &["rebase", "--abort"])?;
        success(root, &["rebase", "--continue"])?;
        ensure!(!root.join(".git/brit-rebase").exists());
        preserved(root)
    }

    fn preserved_neighbors_unchanged(root: &Path) -> Result<()> {
        assert_eq!(fs::read(root.join("topic"))?, b"topic\n");
        assert_eq!(git(root, &["status", "--porcelain"])?, b"");
        Ok(())
    }

    /// Defence in depth beside the mutation guard: while any process holds the
    /// lease of a prepared, never launched pre-rebase invocation, `--abort` will
    /// not cancel it and changes nothing; once the lease is free it cancels.
    #[test]
    fn public_a_held_invocation_lease_blocks_cancelling_the_prepared_hook() -> Result<()> {
        let repo = fixture(false)?;
        let root = repo.path();
        script(&root.join(".git/hooks/pre-rebase"), "exit 0")?;
        kill_rebase_at(root, "replay-operation-invocation-prepared")?;
        let lease = root.join(".git/brit-rebase/pre-rebase/lease");
        ensure!(lease.is_file(), "the prepared invocation has its lease");
        let journal = repository_bytes(&root.join(".git/brit-rebase"))?;
        let mut holder = Command::new("flock")
            .arg(&lease)
            .args(["sh", "-c", "printf ready; cat"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut ready = [0; 5];
        std::io::Read::read_exact(&mut holder.stdout.take().expect("piped"), &mut ready)?;
        let held = invoke(root, &["rebase", "--abort"])?;
        drop(holder.stdin.take());
        holder.wait()?;
        ensure!(
            !held.status.success() && held.stderr.contains("supervisor is live"),
            "a held lease refuses cancellation: {}",
            held.stderr
        );
        assert_eq!(repository_bytes(&root.join(".git/brit-rebase"))?, journal);
        let cancelled = success(root, &["rebase", "--abort"])?;
        ensure!(
            cancelled.stdout.contains("preparation cancelled"),
            "{}",
            cancelled.stdout
        );
        assert!(!root.join(".git/brit-rebase").exists());
        preserved(root)
    }

    /// The cancellation of a `prepared` journal with hook artifacts rests on one
    /// claim: the hook is spawned only after its durable start records
    /// `invoking-pre`. Before that save (paused where the invocation is prepared)
    /// cancellation takes the journal and nothing runs; after it (paused before the
    /// spawn, and after the spawn) cancellation declines and the full abort, which
    /// writes a retained copy, owns it. One unexpected member and it declines too.
    #[test]
    fn public_prepared_hook_artifacts_are_cancelled_only_before_the_durable_start() -> Result<()> {
        for case in [
            "before-durable-start",
            "after-start",
            "after-launch",
            "planted-launch",
            "planted-member",
        ] {
            let repo = fixture(false)?;
            let root = repo.path();
            script(
                &root.join(".git/hooks/pre-rebase"),
                "printf ran > \"$GIT_DIR/hook-ran\"",
            )?;
            let original = git(root, &["rev-parse", "main"])?;
            match case {
                "after-start" | "after-launch" => {
                    let mut child = launch(
                        root,
                        &["rebase", "upstream"],
                        &[("BRIT_AUTHORING_INVOCATION_PAUSE", case.as_ref())],
                    )?;
                    wait_file(&mut child, &root.join(".git/brit-rebase/pre-rebase/checkpoint"))?;
                    interrupt(&mut child, "-KILL")?;
                }
                _ => kill_rebase_at(root, "replay-operation-invocation-prepared")?,
            }
            let journal = root.join(".git/brit-rebase");
            match case {
                "planted-launch" => fs::write(journal.join("pre-rebase/launch.json"), b"{}")?,
                "planted-member" => fs::write(journal.join("unexpected"), b"x")?,
                _ => {}
            }
            let aborted = invoke(root, &["rebase", "--abort"])?;
            let cancelled = aborted.stdout.contains("preparation cancelled");
            let restored = aborted.stdout.contains("Original state restored");
            if case == "before-durable-start" {
                ensure!(
                    aborted.status.success() && cancelled && !journal.exists(),
                    "{case}: cancellation takes the unlaunched prepared journal: {}{}",
                    aborted.stdout,
                    aborted.stderr
                );
                ensure!(
                    !root.join(".git/brit-rebase-retained").exists(),
                    "{case}: cancellation writes no retained copy"
                );
            } else {
                // A planted member also keeps the full abort's cleanup pending,
                // as any unknown journal member does; the restoration happened.
                ensure!(
                    !cancelled && restored && root.join(".git/brit-rebase-retained").exists(),
                    "{case}: the full abort owns this journal: {}{}",
                    aborted.stdout,
                    aborted.stderr
                );
            }
            ensure!(!root.join(".git/hook-ran").exists(), "{case}: the hook never ran");
            assert_eq!(git(root, &["rev-parse", "main"])?, original);
            preserved(root)?;
        }
        Ok(())
    }

    /// An interrupted cancellation finishes on the next `--abort`.
    #[test]
    fn public_interrupted_preparation_cancel_finishes_on_the_next_abort() -> Result<()> {
        for point in [
            "replay-preparation-tombstoned",
            "replay-preparation-last-member-removed",
        ] {
            let repo = fixture(false)?;
            let root = repo.path();
            let markers = tempfile::tempdir()?;
            let scratch = tempfile::tempdir()?;
            let prepare = "replay-retained";
            let mut child = launch(
                root,
                &["rebase", "upstream"],
                &[
                    ("BRIT_SWITCH_CHECKPOINT", prepare.as_ref()),
                    ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
                ],
            )?;
            wait_file(&mut child, &markers.path().join(format!("{prepare}.reached")))?;
            interrupt(&mut child, "-KILL")?;
            let mut child = launch(
                root,
                &["rebase", "--abort"],
                &[
                    ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                    ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
                    ("TMPDIR", scratch.path().as_os_str()),
                ],
            )?;
            wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
            interrupt(&mut child, "-KILL")?;
            let status = success(root, &["rebase", "--status"])?;
            ensure!(
                status.stdout.contains("--abort"),
                "{point}: status names the cure: {}",
                status.stdout
            );
            let aborted = abort_with_scratch(root, scratch.path())?;
            ensure!(aborted.status.success(), "{point}: {}", aborted.stderr);
            assert_no_rebase_leftovers(root, scratch.path())?;
            preserved(root)?;
            success(root, &["rebase", "upstream"])?;
        }
        Ok(())
    }

    #[test]
    fn public_prepared_invocation_gap_never_relaunches_an_operation_hook() -> Result<()> {
        for name in ["pre-rebase", "post-rewrite"] {
            for signal in ["-INT", "-KILL"] {
                let repo = fixture(false)?;
                let root = repo.path();
                script(
                    &root.join(".git/hooks").join(name),
                    "printf called > .git/unexpected-hook",
                )?;
                let markers = tempfile::tempdir()?;
                let point = "replay-operation-invocation-prepared";
                let mut child = launch(
                    root,
                    &["rebase", "upstream"],
                    &[
                        ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                        ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
                    ],
                )?;
                wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
                interrupt(&mut child, signal)?;
                assert!(!root.join(".git/unexpected-hook").exists());
                success(root, &["rebase", "--status"])?;
                if name == "pre-rebase" {
                    refused(root, &["rebase", "--continue"])?;
                    success(root, &["rebase", "--abort"])?;
                } else {
                    refused(root, &["rebase", "--abort"])?;
                    success(root, &["rebase", "--continue"])?;
                }
                assert!(
                    !root.join(".git/unexpected-hook").exists(),
                    "prepared invocation never reruns"
                );
                preserved(root)?;
            }
        }
        Ok(())
    }

    #[test]
    fn public_unknown_sequence_editor_accepts_saved_todo_once() -> Result<()> {
        for signal in ["-INT", "-KILL"] {
            let repo = fixture(false)?;
            let root = repo.path();
            let editor = root.join(".git/editor");
            script(&editor, "printf 'called\\n' >> .git/editor-calls")?;
            let mut child = launch(
                root,
                &["rebase", "-i", "upstream"],
                &[
                    ("GIT_SEQUENCE_EDITOR", editor.as_os_str()),
                    ("BRIT_AUTHORING_INVOCATION_PAUSE", "after-release".as_ref()),
                ],
            )?;
            wait_file(&mut child, &root.join(".git/brit-rebase/editor-0/checkpoint"))?;
            wait_file(&mut child, &root.join(".git/editor-calls"))?;
            wait_invocation_exit(&root.join(".git/brit-rebase/editor-0"))?;
            interrupt(&mut child, signal)?;
            success(root, &["rebase", "--continue"])?;
            assert_eq!(fs::read(root.join(".git/editor-calls"))?, b"called\n");
            assert!(!root.join(".git/brit-rebase").exists());
            preserved(root)?;
        }
        Ok(())
    }

    #[test]
    fn public_child_proposal_recovers_without_reauthoring_then_drains_post_hooks() -> Result<()> {
        for signal in ["-INT", "-KILL"] {
            let repo = fixture(false)?;
            let root = repo.path();
            script(
                &root.join(".git/hooks/post-commit"),
                "printf 'post\\n' >> .git/post-calls",
            )?;
            let candidates = ids(root)?;
            let started = interactive(root, &format!("edit {}\npick {}\n", candidates[0], candidates[1]))?;
            ensure!(started.status.success(), "{}", started.stderr);
            let markers = tempfile::tempdir()?;
            let point = "replay-child-proposal-durable";
            let mut child = launch(
                root,
                &["rebase", "--amend", "-m", "durable amendment"],
                &[
                    ("BRIT_SWITCH_CHECKPOINT", point.as_ref()),
                    ("BRIT_SWITCH_CHECKPOINT_DIR", markers.path().as_os_str()),
                ],
            )?;
            wait_file(&mut child, &markers.path().join(format!("{point}.reached")))?;
            interrupt(&mut child, signal)?;
            assert!(!root.join(".git/post-calls").exists());
            success(root, &["rebase", "--continue"])?;
            assert_eq!(fs::read(root.join(".git/post-calls"))?, b"post\n");
            assert_eq!(
                git(root, &["log", "-1", "--format=%s", "HEAD~1"])?,
                b"durable amendment\n"
            );
            preserved(root)?;
        }
        Ok(())
    }
}

#[test]
fn public_newly_empty_choice_and_conflict_skip_preserve_neighbors() -> Result<()> {
    for skip in [false, true] {
        let repo = fixture(false)?;
        let root = repo.path();
        // The upstream independently contains the first topic's patch.
        let dirty_index = fs::read(root.join(".git/index"))?;
        git(root, &["reset", "--", "neighbor"])?;
        git(root, &["switch", "upstream"])?;
        git(root, &["cherry-pick", "main~1"])?;
        git(root, &["switch", "main"])?;
        fs::write(root.join(".git/index"), dirty_index)?;
        refused(root, &["rebase", "upstream"])?;
        assert!(success(root, &["rebase", "--status"])?.stdout.contains("Empty stop"));
        success(root, &["rebase", if skip { "--skip" } else { "--continue" }])?;
        preserved(root)?;
    }
    let repo = fixture(true)?;
    refused(repo.path(), &["rebase", "upstream"])?;
    success(repo.path(), &["rebase", "--skip"])?;
    assert_eq!(git(repo.path(), &["show", "HEAD:selected"])?, b"upstream\n");
    preserved(repo.path())?;
    Ok(())
}

#[test]
fn public_parser_rejects_deferred_options_and_incompatible_actions_without_state() -> Result<()> {
    let repo = fixture(false)?;
    for args in [
        vec!["rebase"],
        vec!["rebase", "--onto", "upstream", "main"],
        vec!["rebase", "--root"],
        vec!["rebase", "--autosquash", "upstream"],
        vec!["rebase", "--rebase-merges", "upstream"],
        vec!["rebase", "--exec", "false", "upstream"],
        vec!["rebase", "--continue", "--abort"],
        vec!["rebase", "upstream", "--status"],
        vec!["rebase", "-i", "--status"],
        vec!["rebase", "--stage"],
        vec!["rebase", "--stage", "selected"],
        vec!["rebase", "--expand-scope", "--status"],
        vec!["rebase", "--amend", "-m", "a", "--no-edit"],
        vec!["rebase", "--commit", "--no-edit"],
        vec!["rebase", "--amend", "--commit"],
    ] {
        refused(repo.path(), &args)?;
        assert!(
            !repo.path().join(".git/brit-rebase").exists(),
            "{args:?} cannot create state"
        );
    }
    Ok(())
}
