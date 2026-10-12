//! Public pre-push contract: resolved updates, stock-Git input and rejection safety.
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
        .run()
}

fn hook(repo: &Path, body: &str) -> Result<()> {
    let dir = repo.join("hooks with spaces");
    fs::create_dir_all(&dir)?;
    git(repo, &["config", "core.hooksPath", "hooks with spaces"])?;
    let path = dir.join("pre-push");
    fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn remote(repo: &Path) -> Result<tempfile::TempDir> {
    let remote = tempfile::tempdir()?;
    git(remote.path(), &["init", "--bare", "-q"])?;
    git(
        repo,
        &["remote", "add", "origin", remote.path().to_str().expect("temp path")],
    )?;
    Ok(remote)
}

#[test]
fn resolved_create_update_delete_match_git_and_rejection_changes_nothing() -> Result<()> {
    let repo = TestRepo::new("pre-push-updates")?;
    let remote = remote(repo.path())?;
    let old = repo.head_id()?;
    git(repo.path(), &["push", "origin", "main:main", "main:gone"])?;
    repo.commit_file("new", "next commit")?;
    hook(
        repo.path(),
        "printf '%s\\n' \"$1\" \"$2\" >hook-args; cat >hook-input; exit 1",
    )?;
    let specs = ["push", "origin", "main:main", "main:created", ":refs/heads/gone"];
    let stock = gix_testtools::git_command(repo.path()).args(specs).output()?;
    ensure!(!stock.status.success(), "stock hook rejects");
    let expected = fs::read(repo.path().join("hook-input"))?;
    let args = fs::read(repo.path().join("hook-args"))?;
    let output = brit(repo.path(), &specs)?;
    ensure!(!output.status.success(), "Brit hook must reject");
    // Git orders refs differently; the contract is the exact set of resolved updates.
    let lines = |bytes: Vec<u8>| -> Result<Vec<String>> {
        let mut lines: Vec<_> = String::from_utf8(bytes)?.lines().map(str::to_owned).collect();
        lines.sort();
        Ok(lines)
    };
    assert_eq!(lines(fs::read(repo.path().join("hook-input"))?)?, lines(expected)?);
    assert_eq!(fs::read(repo.path().join("hook-args"))?, args);
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, old);
    assert_eq!(git(remote.path(), &["rev-parse", "gone"])?, old);
    assert_eq!(
        git(
            remote.path(),
            &["for-each-ref", "--format=%(refname)", "refs/heads/created"]
        )?,
        ""
    );
    hook(repo.path(), "cat >hook-input")?;
    let output = brit(repo.path(), &specs)?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    assert_eq!(git(remote.path(), &["rev-parse", "created"])?, repo.head_id()?);
    assert_eq!(
        git(
            remote.path(),
            &["for-each-ref", "--format=%(refname)", "refs/heads/gone"]
        )?,
        ""
    );
    Ok(())
}

#[test]
fn dry_run_and_noop_run_hook_but_no_verify_skips_it() -> Result<()> {
    let repo = TestRepo::new("pre-push-dry-run")?;
    let remote = remote(repo.path())?;
    hook(repo.path(), "cat >hook-input; echo invoked >hook-ran")?;
    let output = brit(repo.path(), &["push", "--dry-run", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    ensure!(!fs::read(repo.path().join("hook-input"))?.is_empty());
    assert_eq!(git(remote.path(), &["for-each-ref"])?, "");
    git(repo.path(), &["push", "--no-verify", "origin", "main:main"])?;
    let output = brit(repo.path(), &["push", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(fs::read(repo.path().join("hook-input"))?, b"");
    fs::remove_file(repo.path().join("hook-ran"))?;
    let output = brit(repo.path(), &["push", "--no-verify", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    ensure!(!repo.path().join("hook-ran").exists());
    Ok(())
}

#[test]
fn direct_url_arguments_and_closed_input_match_git() -> Result<()> {
    let repo = TestRepo::new("pre-push-url")?;
    let remote = remote(repo.path())?;
    let url = remote.path().to_str().expect("temp path");
    hook(repo.path(), "printf '%s\\n' \"$1\" \"$2\" >hook-args; exit 0")?;
    let output = brit(repo.path(), &["push", url, "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(
        fs::read_to_string(repo.path().join("hook-args"))?,
        format!("{url}\n{url}\n")
    );
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    Ok(())
}

#[test]
fn nested_push_uses_pushurl_and_relative_hook_path() -> Result<()> {
    let repo = TestRepo::new("pre-push-pushurl")?;
    let fetch_remote = remote(repo.path())?;
    let push_remote = tempfile::tempdir()?;
    git(push_remote.path(), &["init", "--bare", "-q"])?;
    let url = push_remote.path().to_str().expect("temp path");
    git(repo.path(), &["config", "remote.origin.pushurl", url])?;
    hook(repo.path(), "printf '%s\\n' \"$1\" \"$2\" >hook-args; cat >hook-input")?;
    fs::create_dir(repo.path().join("nested"))?;
    let output = brit(&repo.path().join("nested"), &["push", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(
        fs::read_to_string(repo.path().join("hook-args"))?,
        format!("origin\n{url}\n")
    );
    assert_eq!(git(push_remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    assert_eq!(git(fetch_remote.path(), &["for-each-ref"])?, "");
    Ok(())
}

#[test]
fn literal_object_source_is_preserved_in_hook_input() -> Result<()> {
    let repo = TestRepo::new("pre-push-object")?;
    let remote = remote(repo.path())?;
    let head = repo.head_id()?;
    let spec = format!("{head}:refs/heads/from-object");
    hook(repo.path(), "cat >hook-input")?;
    let output = brit(repo.path(), &["push", "origin", &spec])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(
        fs::read_to_string(repo.path().join("hook-input"))?,
        format!("{head} {head} refs/heads/from-object {}\n", "0".repeat(head.len()))
    );
    assert_eq!(git(remote.path(), &["rev-parse", "from-object"])?, head);
    Ok(())
}

#[test]
fn linked_worktree_uses_shared_hook_and_its_own_context() -> Result<()> {
    let repo = TestRepo::new("pre-push-worktree")?;
    let remote = remote(repo.path())?;
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
    hook(
        repo.path(),
        "pwd >hook-cwd; git rev-parse --absolute-git-dir >hook-git-dir; cat >hook-input",
    )?;
    fs::copy(
        repo.path().join("hooks with spaces/pre-push"),
        repo.path().join(".git/hooks/pre-push"),
    )?;
    git(repo.path(), &["config", "--unset", "core.hooksPath"])?;
    fs::create_dir(linked.path().join("nested"))?;
    let output = brit(&linked.path().join("nested"), &["push", "origin", "linked:linked"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    // macOS may spell the same temporary directory /var or /private/var.
    // Resolve aliases while still requiring the exact worktree and Git directory.
    assert_eq!(
        fs::canonicalize(fs::read_to_string(linked.path().join("hook-cwd"))?.trim())?,
        fs::canonicalize(linked.path())?
    );
    assert_eq!(
        fs::canonicalize(fs::read_to_string(linked.path().join("hook-git-dir"))?.trim())?,
        fs::canonicalize(git(linked.path(), &["rev-parse", "--absolute-git-dir"])?)?
    );
    ensure!(!repo.path().join("hook-input").exists());
    assert_eq!(git(remote.path(), &["rev-parse", "linked"])?, repo.head_id()?);
    Ok(())
}

#[test]
fn early_close_accepts_large_input_and_rejection_preserves_remote() -> Result<()> {
    use std::{io::Write, process::Stdio};
    let repo = TestRepo::new("pre-push-closed-pipe")?;
    let remote = remote(repo.path())?;
    let head = repo.head_id()?;
    let mut create = gix_testtools::git_command(repo.path())
        .args(["update-ref", "--stdin"])
        .stdin(Stdio::piped())
        .spawn()?;
    {
        let mut input = create.stdin.take().expect("piped stdin");
        for n in 0..2000 {
            writeln!(input, "create refs/heads/bulk-{n} {head}")?;
        }
    }
    ensure!(create.wait()?.success());
    hook(repo.path(), "exit 1")?;
    let output = brit(repo.path(), &["push", "--all", "origin"])?;
    ensure!(!output.status.success());
    assert_eq!(git(remote.path(), &["for-each-ref"])?, "");
    hook(repo.path(), "exit 0")?;
    let output = brit(repo.path(), &["push", "--all", "origin"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(
        git(remote.path(), &["for-each-ref", "--format=%(refname)"])?
            .lines()
            .count(),
        2001
    );
    Ok(())
}

#[test]
fn lease_flags_and_non_fast_forward_fail_closed_even_without_hooks() -> Result<()> {
    let repo = TestRepo::new("pre-push-safety")?;
    let remote = remote(repo.path())?;
    let base = repo.head_id()?;
    let old = repo.commit_file("remote", "remote work")?;
    git(repo.path(), &["push", "origin", "main:main"])?;
    git(repo.path(), &["reset", "--hard", &base])?;
    repo.commit_file("local", "local work")?;
    for option in ["--force-with-lease", "--force-if-includes", "--no-verify"] {
        let output = brit(repo.path(), &["push", option, "origin", "main:main"])?;
        ensure!(
            !output.status.success(),
            "{option} must not silently overwrite remote history"
        );
        ensure!(output.stderr.contains("git push"), "{}", output.stderr);
        assert_eq!(git(remote.path(), &["rev-parse", "main"])?, old);
    }
    let output = brit(repo.path(), &["push", "--force", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    Ok(())
}
