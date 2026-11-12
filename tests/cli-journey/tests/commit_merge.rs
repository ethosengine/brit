//! Resolved merge authoring: exact Git objects and preservation at every refusal.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const IDENTITY: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "Merge Author"),
    ("GIT_AUTHOR_EMAIL", "author@example.com"),
    ("GIT_AUTHOR_DATE", "2025-01-02 03:04:05 +0000"),
    ("GIT_COMMITTER_NAME", "Merge Committer"),
    ("GIT_COMMITTER_EMAIL", "committer@example.com"),
    ("GIT_COMMITTER_DATE", "2026-01-02 03:04:05 +0000"),
];

fn command(root: &Path, native: bool) -> Command {
    let mut cmd = if native {
        let mut cmd = Command::new(brit_bin().expect("required Brit binary"));
        gix_testtools::configure_git_environment(&mut cmd, root);
        cmd.current_dir(root);
        cmd
    } else {
        gix_testtools::git_command(root)
    };
    cmd.envs(IDENTITY).env("GIT_CONFIG_GLOBAL", root.join("absent-global"));
    cmd
}

fn run(root: &Path, native: bool, args: &[&str]) -> Result<Output> {
    Ok(command(root, native).args(args).output()?)
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = run(root, false, args)?;
    ensure!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}

fn git_path(root: &Path, name: &str) -> Result<PathBuf> {
    Ok(root.join(String::from_utf8(git(root, &["rev-parse", "--git-path", name])?)?.trim()))
}

fn fixture(shape: &str) -> Result<(TestRepo, PathBuf)> {
    let repo = TestRepo::new("resolved-merge")?;
    let root = repo.path();
    repo.commit_file("selected", "base\n")?;
    git(root, &["checkout", "-b", "side"])?;
    repo.commit_file("selected", "side\n")?;
    git(root, &["checkout", "main"])?;
    if shape == "conflict" {
        repo.commit_file("selected", "main\n")?;
    } else {
        repo.commit_file("main-only", "main\n")?;
    }
    let root = if shape == "linked" {
        git(root, &["worktree", "add", "-b", "linked", "linked"])?;
        root.join("linked")
    } else {
        if shape == "detached" {
            git(root, &["checkout", "--detach"])?;
        }
        root.to_path_buf()
    };
    let out = run(&root, false, &["merge", "--no-commit", "--no-ff", "side"])?;
    assert_eq!(out.status.success(), shape != "conflict", "merge fixture: {out:?}");
    if shape == "conflict" {
        fs::write(root.join("selected"), "resolved\n")?;
        git(&root, &["add", "selected"])?;
    }
    if shape == "unchanged" {
        git(&root, &["read-tree", "HEAD"])?;
    }
    Ok((repo, root))
}

fn snapshot(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut paths = vec![root.join("selected")];
    for name in [
        "HEAD",
        "refs/heads/main",
        "logs/HEAD",
        "logs/refs/heads/main",
        "index",
        "ORIG_HEAD",
        "MERGE_HEAD",
        "MERGE_MSG",
        "MERGE_MODE",
        "AUTO_MERGE",
    ] {
        let path = git_path(root, name)?;
        if path.exists() {
            paths.push(path);
        }
    }
    paths
        .into_iter()
        .map(|path| Ok((path.clone(), fs::read(path)?)))
        .collect()
}

fn unchanged(before: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    for (path, bytes) in before {
        assert_eq!(&fs::read(path)?, bytes, "preserve {}", path.display());
    }
    Ok(())
}

#[test]
fn exact_git_objects_for_resolved_conflicted_unchanged_detached_and_linked_merges() -> Result<()> {
    for shape in ["normal", "conflict", "unchanged", "detached", "linked"] {
        for file_message in [false, true] {
            let (_actual, root) = fixture(shape)?;
            let (_oracle, oracle) = fixture(shape)?;
            let message = "  Merge subject  \n\nbody\t \n";
            fs::write(root.join("message"), message)?;
            fs::write(oracle.join("message"), message)?;
            fs::write(root.join("selected"), "unstaged remains\n")?;
            fs::write(oracle.join("selected"), "unstaged remains\n")?;
            let before = snapshot(&root)?;
            let head = git(&root, &["rev-parse", "HEAD"])?;
            let merge_head = fs::read(git_path(&root, "MERGE_HEAD")?)?;
            let args = if file_message {
                vec!["commit", "-F", "message"]
            } else {
                vec!["commit", "-m", message]
            };
            git(&oracle, &args)?;
            let out = run(&root, true, &args)?;
            ensure!(
                out.status.success(),
                "{shape}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(
                git(&root, &["cat-file", "commit", "HEAD"])?,
                git(&oracle, &["cat-file", "commit", "HEAD"])?,
                "{shape}"
            );
            assert_eq!(git(&root, &["rev-parse", "HEAD^1"])?, head);
            assert_eq!(git(&root, &["rev-parse", "HEAD^2"])?, merge_head);
            for name in ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE", "AUTO_MERGE"] {
                assert!(!git_path(&root, name)?.exists(), "cleaned {name}");
            }
            for (path, bytes) in before {
                if path.ends_with("index") || path.ends_with("ORIG_HEAD") || path.ends_with("selected") {
                    assert_eq!(fs::read(path)?, bytes);
                }
            }
            if shape == "linked" {
                assert!(git_path(&root, "MERGE_HEAD")?.to_string_lossy().contains("worktrees"));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn hook(root: &Path, name: &str, body: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let path = git_path(root, &format!("hooks/{name}"))?;
    fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn hooks_edit_message_stage_content_and_post_commit_observes_clean_state() -> Result<()> {
    let (_repo, root) = fixture("normal")?;
    let (_oracle, oracle) = fixture("normal")?;
    for root in [&root, &oracle] {
        hook(
            root,
            "pre-commit",
            "printf 'hook-staged\\n' > selected\ngit add selected\nprintf 'unstaged\\n' > selected",
        )?;
        hook(
            root,
            "prepare-commit-msg",
            "test \"$2\" = message\nprintf '\\nCo-Authored-By: Hook <hook@example.com>\\n' >> \"$1\"",
        )?;
        hook(root, "commit-msg", "printf '\\nvalidated\\n' >> \"$1\"")?;
        hook(
            root,
            "post-commit",
            "test ! -e \"$(git rev-parse --git-path MERGE_HEAD)\"\nprintf done > post-ran\nexit 1",
        )?;
    }
    git(&oracle, &["commit", "-m", "merge"])?;
    let out = run(&root, true, &["commit", "-m", "merge"])?;
    ensure!(out.status.success(), "published merge: {out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("post-commit"));
    assert_eq!(
        git(&root, &["cat-file", "commit", "HEAD"])?,
        git(&oracle, &["cat-file", "commit", "HEAD"])?
    );
    assert_eq!(git(&root, &["show", "HEAD:selected"])?, b"hook-staged\n");
    assert_eq!(fs::read(root.join("selected"))?, b"unstaged\n");
    assert_eq!(fs::read(root.join("post-ran"))?, b"done");
    Ok(())
}

#[test]
fn malformed_missing_noncommit_multiple_and_incorporated_heads_preserve_state() -> Result<()> {
    for kind in [
        "malformed",
        "missing",
        "noncommit",
        "multiple",
        "absent-object",
        "incorporated",
        "symlink",
    ] {
        let (_repo, root) = fixture("normal")?;
        let path = git_path(&root, "MERGE_HEAD")?;
        match kind {
            "malformed" => fs::write(&path, "not-an-id\n")?,
            "missing" => fs::remove_file(&path)?,
            "noncommit" => fs::write(&path, git(&root, &["rev-parse", "HEAD^{tree}"])?)?,
            "multiple" => {
                let head = fs::read(&path)?;
                fs::write(&path, [head.clone(), head].concat())?;
            }
            "absent-object" => fs::write(&path, format!("{}\n", "1".repeat(40)))?,
            "incorporated" => fs::write(&path, git(&root, &["rev-parse", "HEAD"])?)?,
            "symlink" => {
                #[cfg(unix)]
                {
                    fs::rename(&path, root.join("foreign-head"))?;
                    std::os::unix::fs::symlink(root.join("foreign-head"), &path)?;
                }
                #[cfg(not(unix))]
                {
                    continue;
                }
            }
            _ => unreachable!(),
        }
        let before = snapshot(&root)?;
        let out = run(&root, true, &["commit", "-m", "refuse"])?;
        ensure!(!out.status.success(), "{kind} must refuse");
        unchanged(&before)?;
    }
    Ok(())
}

#[test]
fn unsupported_states_options_and_foreign_locks_preserve_bytes() -> Result<()> {
    for name in [
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-apply",
        "rebase-merge",
        "sequencer",
        "BISECT_LOG",
        "MERGE_AUTOSTASH",
        "MERGE_RR",
        "REBASE_HEAD",
        "SQUASH_MSG",
        "HEAD.lock",
        "index.lock",
        "refs/heads/main.lock",
        "MERGE_HEAD.lock",
        "MERGE_MSG.lock",
        "AUTO_MERGE.lock",
        "MERGE_MODE.lock",
    ] {
        let (_repo, root) = fixture("normal")?;
        let path = git_path(&root, name)?;
        fs::write(&path, "foreign state\n")?;
        let before = snapshot(&root)?;
        let out = run(&root, true, &["commit", "-m", "refuse"])?;
        ensure!(!out.status.success(), "{name} must refuse: {out:?}");
        unchanged(&before)?;
        assert_eq!(fs::read(&path)?, b"foreign state\n");
    }
    for args in [
        vec!["commit"],
        vec!["commit", "--amend", "-m", "refuse"],
        vec!["commit", "--amend", "--no-edit"],
    ] {
        let (_repo, root) = fixture("normal")?;
        let before = snapshot(&root)?;
        ensure!(!run(&root, true, &args)?.status.success(), "{args:?}");
        unchanged(&before)?;
    }
    let (_repo, root) = fixture("normal")?;
    git(&root, &["config", "commit.gpgSign", "true"])?;
    let before = snapshot(&root)?;
    ensure!(!run(&root, true, &["commit", "-m", "signed"])?.status.success());
    unchanged(&before)?;
    Ok(())
}

#[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
mod faults {
    use super::*;
    use std::{
        process::{Child, Stdio},
        time::{Duration, Instant},
    };

    struct Paused(Option<Child>);

    impl Drop for Paused {
        fn drop(&mut self) {
            if let Some(child) = &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    impl Paused {
        fn start(root: &Path, checkpoint: &str) -> Result<Self> {
            let mut child = Self(Some(
                command(root, true)
                    .env("BRIT_SWITCH_CHECKPOINT", checkpoint)
                    .env("BRIT_SWITCH_CHECKPOINT_DIR", root)
                    .args(["commit", "-m", "interrupted merge"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?,
            ));
            let deadline = Instant::now() + Duration::from_secs(10);
            while !root.join(format!("{checkpoint}.reached")).exists() {
                ensure!(Instant::now() < deadline, "checkpoint {checkpoint} timed out");
                if let Some(status) = child.0.as_mut().expect("child").try_wait()? {
                    let out = child.output()?;
                    anyhow::bail!(
                        "checkpoint {checkpoint} exited {status}: {}",
                        String::from_utf8_lossy(&out.stderr)
                    );
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(child)
        }

        fn signal(&self, name: &str) -> Result<()> {
            let status = Command::new("kill")
                .args([name, &self.0.as_ref().expect("child").id().to_string()])
                .status()?;
            ensure!(status.success(), "signal isolated child");
            Ok(())
        }

        fn output(mut self) -> Result<Output> {
            Ok(self.0.take().expect("child").wait_with_output()?)
        }
    }

    #[test]
    fn publication_rechecks_pinned_head_attachment_and_merge_metadata() -> Result<()> {
        for change in ["head", "attachment", "detached", "metadata", "cancel"] {
            let (_repo, root) = fixture("normal")?;
            let child = Paused::start(&root, "merge-before-publication")?;
            match change {
                "head" => {
                    git(&root, &["update-ref", "HEAD", "side"])?;
                }
                "attachment" => {
                    git(&root, &["branch", "other", "HEAD"])?;
                    git(&root, &["symbolic-ref", "HEAD", "refs/heads/other"])?;
                }
                "detached" => {
                    git(&root, &["update-ref", "--no-deref", "HEAD", "HEAD"])?;
                }
                "metadata" => fs::write(git_path(&root, "MERGE_MSG")?, "foreign metadata\n")?,
                "cancel" => child.signal("-INT")?,
                _ => unreachable!(),
            }
            let before = snapshot(&root)?;
            let refs = git(&root, &["show-ref"])?;
            fs::write(root.join("merge-before-publication.release"), "continue")?;
            let out = child.output()?;
            ensure!(!out.status.success(), "must refuse {change}: {out:?}");
            unchanged(&before)?;
            assert_eq!(git(&root, &["show-ref"])?, refs);
        }
        Ok(())
    }

    #[test]
    fn interrupted_or_failed_cleanup_preserves_published_commit_and_refuses_duplicate() -> Result<()> {
        for change in ["interrupt", "kill", "changed", "unremovable"] {
            let (_repo, root) = fixture("normal")?;
            hook(&root, "post-commit", "touch post-ran")?;
            let old = git(&root, &["rev-parse", "HEAD"])?;
            let original_index = fs::read(git_path(&root, "index")?)?;
            let original_head = fs::read(git_path(&root, "ORIG_HEAD")?)?;
            let merge_head = fs::read(git_path(&root, "MERGE_HEAD")?)?;
            let child = Paused::start(&root, "merge-cleanup")?;
            let published = git(&root, &["rev-parse", "HEAD"])?;
            assert_ne!(published, old);
            assert!(!git_path(&root, "MERGE_MSG")?.exists());
            assert_eq!(fs::read(git_path(&root, "MERGE_HEAD")?)?, merge_head);
            match change {
                "interrupt" => child.signal("-INT")?,
                "kill" => child.signal("-KILL")?,
                "changed" => fs::write(git_path(&root, "MERGE_MODE")?, "foreign mode\n")?,
                "unremovable" => {
                    fs::remove_file(git_path(&root, "MERGE_MODE")?)?;
                    fs::create_dir(git_path(&root, "MERGE_MODE")?)?;
                }
                _ => unreachable!(),
            }
            fs::write(root.join("merge-cleanup.release"), "continue")?;
            let out = child.output()?;
            ensure!(!out.status.success(), "interrupted cleanup refuses: {out:?}");
            if change != "kill" {
                let diagnostic = String::from_utf8_lossy(&out.stderr);
                assert!(diagnostic.contains(String::from_utf8_lossy(&published).trim()));
                assert!(diagnostic.contains("was published") && diagnostic.contains("MERGE_HEAD"));
            }
            assert_eq!(git(&root, &["rev-parse", "HEAD"])?, published);
            assert_eq!(fs::read(git_path(&root, "index")?)?, original_index);
            assert_eq!(fs::read(git_path(&root, "ORIG_HEAD")?)?, original_head);
            assert_eq!(fs::read(git_path(&root, "MERGE_HEAD")?)?, merge_head);
            assert!(!root.join("post-ran").exists());
            if change == "unremovable" {
                // Keep the obstructing directory: refusal must preserve it too.
                assert!(git_path(&root, "MERGE_MODE")?.is_dir());
            }
            let retry = run(&root, true, &["commit", "-m", "duplicate"])?;
            ensure!(!retry.status.success(), "never duplicate an incorporated merge");
            if change != "unremovable" {
                assert!(String::from_utf8_lossy(&retry.stderr).contains("already incorporated"));
            }
            assert_eq!(git(&root, &["rev-parse", "HEAD"])?, published);
            if change == "changed" {
                assert_eq!(fs::read(git_path(&root, "MERGE_MODE")?)?, b"foreign mode\n");
            }
        }
        Ok(())
    }
}

#[test]
fn unresolved_index_and_unborn_head_refuse() -> Result<()> {
    let (_repo, root) = fixture("conflict")?;
    git(&root, &["merge", "--abort"])?;
    ensure!(!run(&root, false, &["merge", "--no-commit", "side"])?.status.success());
    let before = snapshot(&root)?;
    ensure!(!run(&root, true, &["commit", "-m", "unresolved"])?.status.success());
    unchanged(&before)?;
    let (_repo, root) = fixture("normal")?;
    git(&root, &["symbolic-ref", "HEAD", "refs/heads/unborn"])?;
    let before = snapshot(&root)?;
    ensure!(!run(&root, true, &["commit", "-m", "unborn"])?.status.success());
    unchanged(&before)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn rejected_hooks_and_prepublication_cancellation_preserve_merge() -> Result<()> {
    for name in ["pre-commit", "prepare-commit-msg", "commit-msg"] {
        for body in ["exit 1", "kill -INT \"$PPID\"\nsleep 0.1"] {
            let (_repo, root) = fixture("normal")?;
            hook(&root, name, body)?;
            hook(&root, "post-commit", "touch post-ran")?;
            let before = snapshot(&root)?;
            ensure!(!run(&root, true, &["commit", "-m", "refuse"])?.status.success());
            unchanged(&before)?;
            assert!(!root.join("post-ran").exists());
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn hooks_cannot_replace_head_attachment_or_merge_metadata() -> Result<()> {
    for body in [
        "git update-ref HEAD refs/heads/side",
        "git branch other HEAD\ngit symbolic-ref HEAD refs/heads/other",
        "git update-ref --no-deref HEAD HEAD",
        "printf changed > \"$(git rev-parse --git-path MERGE_MSG)\"",
        "printf changed > \"$(git rev-parse --git-path MERGE_HEAD)\"",
        "printf changed > \"$(git rev-parse --git-path MERGE_MODE)\"",
        "printf changed > \"$(git rev-parse --git-path AUTO_MERGE)\"",
        "printf pending > \"$(git rev-parse --git-path MERGE_AUTOSTASH)\"",
    ] {
        let (_repo, root) = fixture("normal")?;
        hook(
            &root,
            "commit-msg",
            &format!("{body}\ngit rev-parse HEAD > hook-head\ncp \"$(git rev-parse --git-path HEAD)\" hook-attachment"),
        )?;
        let index = fs::read(git_path(&root, "index")?)?;
        let out = run(&root, true, &["commit", "-m", "refuse"])?;
        ensure!(!out.status.success(), "hook changes must refuse: {out:?}");
        assert_eq!(git(&root, &["rev-parse", "HEAD"])?, fs::read(root.join("hook-head"))?);
        assert_eq!(
            fs::read(git_path(&root, "HEAD")?)?,
            fs::read(root.join("hook-attachment"))?
        );
        assert_eq!(fs::read(git_path(&root, "index")?)?, index);
        assert!(git_path(&root, "MERGE_HEAD")?.exists());
    }
    Ok(())
}
