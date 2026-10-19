//! Explicit leases compare the receive-pack advertisement and retain its old OID.
//! Each oracle runs against its own disposable repository and bare remote.
use std::{fs, path::Path};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation},
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

fn push(repo: &Path, stock: bool, args: &[&str], success: bool) -> Result<String> {
    let (ok, stderr) = if stock {
        let output = gix_testtools::git_command(repo).arg("push").args(args).output()?;
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    } else {
        let output = BritInvocation::new(brit_bin().expect("required binary"))
            .arg("push")
            .args(args.iter().copied())
            .current_dir(repo)
            .run()?;
        (output.status.success(), output.stderr)
    };
    ensure!(
        ok == success,
        "stock={stock}, args={args:?}, expected success={success}: {stderr}"
    );
    Ok(stderr)
}

fn fixture() -> Result<(TestRepo, tempfile::TempDir, String, String)> {
    let repo = TestRepo::new("explicit-lease")?;
    let remote = tempfile::tempdir()?;
    git(remote.path(), &["init", "--bare", "-q"])?;
    git(
        repo.path(),
        &["remote", "add", "origin", remote.path().to_str().expect("temp path")],
    )?;
    let base = repo.head_id()?;
    let old = repo.commit_file("remote", "remote work")?;
    git(repo.path(), &["push", "origin", "main:main"])?;
    git(repo.path(), &["reset", "--hard", &base])?;
    repo.commit_file("local", "replacement work")?;
    Ok((repo, remote, old, base))
}

fn hook(repo: &Path, body: &str) -> Result<()> {
    let path = repo.join(".git/hooks/pre-push");
    fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn lease(oid: &str) -> String {
    format!("--force-with-lease=refs/heads/main:{oid}")
}
fn refs(remote: &Path) -> Result<String> {
    git(remote, &["for-each-ref", "--format=%(refname) %(objectname)"])
}

#[test]
fn matching_lease_replaces_non_fast_forward_like_git() -> Result<()> {
    for stock in [true, false] {
        let (repo, remote, old, _) = fixture()?;
        push(
            repo.path(),
            stock,
            &[&lease(&old), "origin", "main:refs/heads/main"],
            true,
        )?;
        assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    }
    Ok(())
}

#[test]
fn stale_lease_refuses_before_hook_even_with_force_or_no_verify() -> Result<()> {
    for stock in [true, false] {
        for option in ["--force", "--no-verify", "--dry-run"] {
            let (repo, remote, _, base) = fixture()?;
            let before = refs(remote.path())?;
            hook(repo.path(), "echo invoked >hook-ran; cat >/dev/null")?;
            // Stock Git lets --force override a stale lease. Brit deliberately
            // treats an explicit expectation as binding even with --force.
            let stock_force = stock && option == "--force";
            push(
                repo.path(),
                stock,
                &[option, &lease(&base), "origin", "main:refs/heads/main"],
                stock_force,
            )?;
            if !stock {
                ensure!(!repo.path().join("hook-ran").exists(), "stale lease must precede hooks");
            }
            if stock_force {
                assert_eq!(refs(remote.path())?, format!("refs/heads/main {}", repo.head_id()?));
            } else {
                assert_eq!(refs(remote.path())?, before);
            }
        }
    }
    Ok(())
}

#[test]
fn advertised_expected_object_need_not_exist_locally() -> Result<()> {
    for stock in [true, false] {
        let (repo, remote, _, _) = fixture()?;
        repo.commit_file("local-second", "second unsent commit")?;
        repo.commit_file("local-third", "third unsent commit")?;
        let competitor = TestRepo::new("lease-absent-object")?;
        let expected = competitor.commit_file("absent", "only remote knows this")?;
        git(
            competitor.path(),
            &[
                "push",
                "--force",
                remote.path().to_str().expect("temp path"),
                "main:refs/heads/main",
            ],
        )?;
        let absent = gix_testtools::git_command(repo.path())
            .args(["cat-file", "-e", &expected])
            .output()?;
        ensure!(!absent.status.success(), "fixture must not contain expected object");
        push(
            repo.path(),
            stock,
            &[&lease(&expected), "origin", "main:refs/heads/main"],
            true,
        )?;
        assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
        git(remote.path(), &["fsck", "--full"])?;
        assert_eq!(
            git(remote.path(), &["show", "main:local-second"])?,
            "second unsent commit"
        );
    }
    Ok(())
}

#[test]
fn matching_dry_run_preserves_remote_and_runs_hook() -> Result<()> {
    for stock in [true, false] {
        let (repo, remote, old, _) = fixture()?;
        let before = refs(remote.path())?;
        hook(repo.path(), "cat >hook-input")?;
        push(
            repo.path(),
            stock,
            &["--dry-run", &lease(&old), "origin", "main:refs/heads/main"],
            true,
        )?;
        assert_eq!(refs(remote.path())?, before);
        assert_eq!(
            fs::read_to_string(repo.path().join("hook-input"))?,
            format!("refs/heads/main {} refs/heads/main {old}\n", repo.head_id()?)
        );
    }
    Ok(())
}

#[test]
fn matching_noop_runs_hook_with_empty_input() -> Result<()> {
    for stock in [true, false] {
        let (repo, remote, old, _) = fixture()?;
        git(repo.path(), &["reset", "--hard", &old])?;
        let before = refs(remote.path())?;
        hook(repo.path(), "cat >hook-input")?;
        push(
            repo.path(),
            stock,
            &[&lease(&old), "origin", "main:refs/heads/main"],
            true,
        )?;
        assert_eq!(fs::read(repo.path().join("hook-input"))?, b"");
        assert_eq!(refs(remote.path())?, before);
    }
    Ok(())
}

#[test]
fn stale_noop_refuses_before_hook_including_dry_run() -> Result<()> {
    // Git permits a stale lease when no update is needed. Brit deliberately validates
    // the explicit expectation even in that case, before hook side effects.
    for option in ["--dry-run", "--no-verify", "--force"] {
        let (repo, remote, old, base) = fixture()?;
        git(repo.path(), &["reset", "--hard", &old])?;
        let before = refs(remote.path())?;
        hook(repo.path(), "echo invoked >hook-ran")?;
        push(
            repo.path(),
            true,
            &[option, &lease(&base), "origin", "main:refs/heads/main"],
            true,
        )?;
        if repo.path().join("hook-ran").exists() {
            fs::remove_file(repo.path().join("hook-ran"))?;
        }
        push(
            repo.path(),
            false,
            &[option, &lease(&base), "origin", "main:refs/heads/main"],
            false,
        )?;
        ensure!(!repo.path().join("hook-ran").exists());
        assert_eq!(refs(remote.path())?, before);
    }
    Ok(())
}

#[test]
fn hook_rejection_preserves_remote_and_no_verify_only_bypasses_hook() -> Result<()> {
    for stock in [true, false] {
        let (repo, remote, old, _) = fixture()?;
        let before = refs(remote.path())?;
        hook(repo.path(), "cat >hook-input; echo invoked >hook-ran; exit 1")?;
        push(
            repo.path(),
            stock,
            &[&lease(&old), "origin", "main:refs/heads/main"],
            false,
        )?;
        assert_eq!(refs(remote.path())?, before);
        ensure!(repo.path().join("hook-ran").exists());
        fs::remove_file(repo.path().join("hook-ran"))?;
        push(
            repo.path(),
            stock,
            &["--no-verify", &lease(&old), "origin", "main:refs/heads/main"],
            true,
        )?;
        ensure!(!repo.path().join("hook-ran").exists());
        assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    }
    Ok(())
}

#[test]
fn competing_writer_during_hook_is_rejected_by_transport() -> Result<()> {
    for stock in [true, false] {
        let (repo, remote, old, base) = fixture()?;
        // base is already in the remote object store. A competing writer advances
        // the ref after advertisement, in the hook, without a second push process.
        hook(
            repo.path(),
            &format!("cat >hook-input; git --git-dir=\"$2\" update-ref refs/heads/main {base} {old}"),
        )?;
        let stderr = push(
            repo.path(),
            stock,
            &[&lease(&old), "origin", "main:refs/heads/main"],
            false,
        )?;
        ensure!(
            stderr.contains("cannot lock ref")
                || stderr.contains("incorrect old value provided")
                || stderr.contains("failed to update ref")
                || (stderr.contains("is at") && stderr.contains("expected")),
            "race must fail at server CAS: {stderr}"
        );
        assert_eq!(git(remote.path(), &["rev-parse", "main"])?, base);
        ensure!(fs::read_to_string(repo.path().join("hook-input"))?.contains(&old));
        assert_eq!(refs(remote.path())?, format!("refs/heads/main {base}"));
    }
    Ok(())
}

#[test]
fn destination_mismatch_and_missing_remote_branch_refuse_before_hook() -> Result<()> {
    let (repo, remote, old, _) = fixture()?;
    let before = refs(remote.path())?;
    hook(repo.path(), "echo invoked >hook-ran")?;
    for (option, destination) in [
        (lease(&old), "main:refs/heads/other"),
        (
            format!("--force-with-lease=refs/heads/absent:{old}"),
            "main:refs/heads/absent",
        ),
    ] {
        push(repo.path(), false, &[&option, "origin", destination], false)?;
        ensure!(!repo.path().join("hook-ran").exists());
        assert_eq!(refs(remote.path())?, before);
    }
    Ok(())
}

#[test]
fn unsupported_lease_forms_refuse_without_remote_or_hook_changes() -> Result<()> {
    let (repo, remote, old, _) = fixture()?;
    let before = refs(remote.path())?;
    hook(repo.path(), "echo invoked >hook-ran")?;
    let forms = [
        "--force-with-lease".to_owned(),
        "--force-with-lease=refs/heads/main".to_owned(),
        "--force-with-lease=refs/heads/main:".to_owned(),
        format!("--force-with-lease=main:{old}"),
        format!("--force-with-lease=refs/tags/main:{old}"),
        format!("--force-with-lease=refs/heads/main:{}", &old[..12]),
        "--force-with-lease=refs/heads/main:zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_owned(),
        "--force-with-lease=refs/heads/main:HEAD".to_owned(),
    ];
    for form in &forms {
        push(repo.path(), false, &[form, "origin", "main:refs/heads/main"], false)?;
    }
    let valid = lease(&old);
    for args in [
        vec![valid.as_str(), "origin", ":refs/heads/main"],
        vec![valid.as_str(), "origin", "main:main"],
        vec![
            valid.as_str(),
            "origin",
            "main:refs/heads/main",
            "main:refs/heads/other",
        ],
        vec![valid.as_str(), "--force-if-includes", "origin", "main:refs/heads/main"],
        vec![valid.as_str(), "origin"],
        vec![valid.as_str(), "--all", "origin"],
        vec![
            valid.as_str(),
            "--recurse-submodules=only",
            "origin",
            "main:refs/heads/main",
        ],
    ] {
        push(repo.path(), false, &args, false)?;
    }
    ensure!(!repo.path().join("hook-ran").exists());
    assert_eq!(refs(remote.path())?, before);
    Ok(())
}
