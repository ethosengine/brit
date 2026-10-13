//! Real persistence-boundary errors in dedicated fault builds, not disk exhaustion.
#![cfg(all(target_os = "linux", feature = "switch-test-faults"))]
use anyhow::{Result, ensure};
use gitoxide_core::repository::replay::{Advance, Session};
use gix::bstr::ByteSlice;
use std::{fs, path::Path};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "git fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}
fn fixture() -> Result<tempfile::TempDir> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.name", "IO"],
        vec!["config", "user.email", "io@example.invalid"],
    ] {
        git(root, &args)?;
    }
    fs::write(root.join("selected"), b"base\n")?;
    fs::write(root.join("neighbor"), b"clean\n")?;
    git(root, &["add", "."])?;
    git(root, &["commit", "-qm", "base"])?;
    git(root, &["switch", "-qc", "upstream"])?;
    fs::write(root.join("upstream"), b"upstream\n")?;
    git(root, &["add", "."])?;
    git(root, &["commit", "-qm", "upstream"])?;
    git(root, &["switch", "-q", "main"])?;
    fs::write(root.join("selected"), b"topic\n")?;
    git(root, &["add", "selected"])?;
    git(root, &["commit", "-qm", "topic"])?;
    fs::write(root.join("neighbor"), b"staged\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged\n")?;
    Ok(temp)
}
fn ready(repo: &gix::Repository) -> Result<Session> {
    let mut session = Session::prepare(repo, b"upstream".as_bstr())?.expect("divergence");
    assert_eq!(session.advance(repo)?, Advance::Started);
    assert!(matches!(session.advance(repo)?, Advance::Step { .. }));
    assert_eq!(session.advance(repo)?, Advance::ReadyFinal);
    Ok(session)
}
fn preserved(root: &Path) -> Result<()> {
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged\n");
    assert_eq!(git(root, &["show", ":neighbor"])?, b"staged\n");
    Ok(())
}
#[test]
fn preparation_enospc_and_file_directory_sync_errors_are_unpublished() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for (operation, errno, suffix) in [
        ("write", 28, "/state.pending"),
        ("file-sync", 5, "/state.pending"),
        ("directory-sync", 5, "/brit-rebase"),
    ] {
        let temp = fixture()?;
        let root = temp.path();
        let repo = gix::open(root)?;
        let index = fs::read(repo.index_path())?;
        let head = fs::read(repo.git_dir().join("HEAD"))?;
        let branch = git(root, &["rev-parse", "main"])?;
        let _fault = gix_testtools::Env::new().set(
            "BRIT_REPLAY_IO_FAULT",
            format!(
                "{operation}|{errno}|{}{}",
                repo.git_dir().join("brit-rebase").display(),
                if suffix == "/brit-rebase" { "" } else { suffix }
            ),
        );
        assert!(
            Session::prepare(&repo, b"upstream".as_bstr()).is_err(),
            "{operation} must interrupt actual preparation"
        );
        assert_eq!(fs::read(repo.index_path())?, index);
        assert_eq!(fs::read(repo.git_dir().join("HEAD"))?, head);
        assert_eq!(git(root, &["rev-parse", "main"])?, branch);
        preserved(root)?;
    }
    Ok(())
}
#[test]
fn final_publication_sync_failures_distinguish_undecided_from_irrevocable_publication() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let mut decided = 0;
    for (resource, occurrence) in [
        ("metadata-lock-0.json", 1),
        ("publication", 1),
        ("publication", 2),
        ("publication", 3),
    ] {
        let temp = fixture()?;
        let root = temp.path();
        let repo = gix::open(root)?;
        let mut session = ready(&repo)?;
        let tip = session.tip()?;
        let fault =
            gix_testtools::Env::new().set("BRIT_REPLAY_IO_FAULT", format!("file-sync|5|/{resource}|{occurrence}"));
        assert!(
            session.finish(&repo).is_err(),
            "{resource} occurrence{occurrence} must hit a real sync"
        );
        drop(fault);
        let mut pending_final = false;
        for entry in fs::read_dir(session.directory())? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("decision-")
                && entry.path().extension().is_some_and(|s| s == "json")
            {
                let env: serde_json::Value = serde_json::from_slice(&fs::read(entry.path())?)?;
                let bytes: Vec<u8> = serde_json::from_value(env["payload"].clone())?;
                let value: serde_json::Value = serde_json::from_slice(&bytes)?;
                pending_final |= value["transition"]["kind"] == "Final";
            }
        }
        if pending_final {
            decided += 1;
            assert!(
                Session::open(&repo)?.abort(&repo).is_err(),
                "durable Final cannot become abort"
            );
        }
        let mut recovered = if pending_final {
            Session::recover_decision(&repo)?
        } else {
            Session::open(&repo)?
        };
        recovered.finish(&repo)?;
        assert_eq!(repo.find_reference("main")?.id().detach(), tip);
        preserved(root)?;
        recovered.cleanup(&repo)?;
    }
    assert!(decided > 0, "at least one injected failure follows durable Final");
    Ok(())
}
#[test]
fn retained_copy_enospc_and_sync_errors_keep_live_authorship_until_complete_copy() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for (operation, errno) in [("write", 28), ("file-sync", 5), ("directory-sync", 5)] {
        let temp = fixture()?;
        let root = temp.path();
        let repo = gix::open(root)?;
        let mut session = ready(&repo)?;
        let head = fs::read(repo.git_dir().join("HEAD"))?;
        let index = fs::read(repo.index_path())?;
        let fault = gix_testtools::Env::new().set(
            "BRIT_REPLAY_IO_FAULT",
            format!(
                "{operation}|{errno}|{}",
                repo.git_dir().join("brit-rebase-retained").display()
            ),
        );
        assert!(session.abort(&repo).is_err());
        drop(fault);
        assert_eq!(fs::read(repo.git_dir().join("HEAD"))?, head);
        assert_eq!(fs::read(repo.index_path())?, index);
        preserved(root)?;
        let mut recovered = Session::open(&repo)?;
        match recovered.abort(&repo) {
            Ok(receipt) => {
                gitoxide_core::repository::replay::retained::inspect(&receipt.path)?;
                preserved(root)?;
            }
            Err(_) => {
                assert_eq!(fs::read(repo.git_dir().join("HEAD"))?, head);
                assert_eq!(fs::read(repo.index_path())?, index);
                preserved(root)?;
            }
        }
    }
    Ok(())
}
#[test]
fn cleanup_sync_failure_reports_committed_state_and_keeps_independent_copy() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let temp = fixture()?;
    let root = temp.path();
    let repo = gix::open(root)?;
    let mut session = ready(&repo)?;
    let tip = session.tip()?;
    session.finish(&repo)?;
    let fault = gix_testtools::Env::new().set(
        "BRIT_REPLAY_IO_FAULT",
        format!(
            "directory-sync|5|{}",
            repo.git_dir().join("brit-rebase-retained").display()
        ),
    );
    assert!(session.cleanup(&repo).is_err());
    drop(fault);
    assert_eq!(repo.find_reference("main")?.id().detach(), tip);
    assert!(Session::open(&repo)?.abort(&repo).is_err());
    preserved(root)?;
    let mut recovered = Session::open(&repo)?;
    let receipt = recovered.cleanup(&repo)?;
    gitoxide_core::repository::replay::retained::inspect(&receipt.path)?;
    Ok(())
}

#[test]
fn lost_hook_exit_sync_is_unknown_never_rerun_and_respects_publication_boundary() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    for post in [false, true] {
        let temp = fixture()?;
        let root = temp.path();
        let repo = gix::open(root)?;
        let name = if post { "post-rewrite" } else { "pre-rebase" };
        let hook = repo.git_dir().join("hooks").join(name);
        fs::write(&hook, b"#!/bin/sh\nprintf 'once\\n' >> .git/hook-count\n")?;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
        let session = if post {
            ready(&repo)?
        } else {
            Session::prepare(&repo, b"upstream".as_bstr())?.expect("divergence")
        };
        let original = git(root, &["rev-parse", "main"])?;
        let mut child = std::process::Command::new(std::env::current_exe()?);
        gix_testtools::configure_git_environment(&mut child, root);
        let output = child
            .args(["--exact", "hook_sync_failure_child_helper", "--nocapture"])
            .env("BRIT_IO_HOOK_ROOT", root)
            .env("BRIT_IO_HOOK_POST", if post { "1" } else { "0" })
            .env(
                "BRIT_REPLAY_IO_FAULT",
                format!(
                    "file-sync|5|{}",
                    session.directory().join(name).join("exit.json.pending").display()
                ),
            )
            .output()?;
        assert!(
            !output.status.success(),
            "hook exit sync must fail in the owning process"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("injected file-sync failure"));
        assert_eq!(fs::read(repo.git_dir().join("hook-count"))?, b"once\n");
        let mut recovered = Session::open(&repo)?;
        if post {
            assert_ne!(git(root, &["rev-parse", "main"])?, original);
            assert!(recovered.abort(&repo).is_err());
            recovered.acknowledge_post_hook(&repo)?;
            recovered.cleanup(&repo)?;
        } else {
            assert_eq!(git(root, &["rev-parse", "main"])?, original);
            recovered.recover_pre_hook(&repo)?;
            assert_eq!(recovered.phase(), "pre-unknown");
            assert!(recovered.advance(&repo).is_err());
            recovered.abort(&repo)?;
            recovered.cleanup(&repo)?;
        }
        assert_eq!(
            fs::read(repo.git_dir().join("hook-count"))?,
            b"once\n",
            "unknown hook never reruns"
        );
        preserved(root)?;
    }
    Ok(())
}

#[test]
fn hook_sync_failure_child_helper() -> Result<()> {
    let Some(root) = std::env::var_os("BRIT_IO_HOOK_ROOT") else {
        return Ok(());
    };
    let repo = gix::open(root)?;
    let mut session = Session::open(&repo)?;
    if std::env::var("BRIT_IO_HOOK_POST").as_deref() == Ok("1") {
        session.finish(&repo)
    } else {
        session.advance(&repo).map(|_| ())
    }
}
