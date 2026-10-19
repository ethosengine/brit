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

#[test]
fn public_linear_replay_finishes_and_helper_exports_after_cleanup_without_runtime_git() -> Result<()> {
    let repo = fixture(false)?;
    let root = repo.path();
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
    assert!(destination.path().join("manifest.json").is_file());
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
