//! Public pre-push contract: resolved updates, stock-Git input and rejection safety.
use std::{
    fs,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = gix_testtools::git_command(repo)
        .args(["-c", &format!("safe.directory={}", repo.display())])
        .args(args)
        .output()?;
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
fn delayed_passing_hook_delivers_branch_creation_and_update() -> Result<()> {
    let repo = TestRepo::new("pre-push-delayed-hook")?;
    let remote = remote(repo.path())?;
    hook(repo.path(), "cat >hook-input; sleep 2; printf passed >hook-passed")?;
    for payload in ["first payload", "updated payload"] {
        let commit_id = repo.commit_file("payload", payload)?;
        let started = Instant::now();
        let output = brit(repo.path(), &["push", "origin", "main:main"])?;
        ensure!(output.status.success(), "delayed hook push: {}", output.stderr);
        ensure!(started.elapsed() >= Duration::from_secs(2), "push waited for the hook");
        assert_eq!(fs::read(repo.path().join("hook-passed"))?, b"passed");
        ensure!(
            fs::read_to_string(repo.path().join("hook-input"))?.contains(&commit_id),
            "the delayed hook saw this commit's update"
        );
        assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/main"])?, commit_id);
        assert_eq!(git(remote.path(), &["show", "main:payload"])?, payload);
        assert_eq!(repo.head_id()?, commit_id, "push leaves the local branch unchanged");
        fs::remove_file(repo.path().join("hook-passed"))?;
    }
    git(remote.path(), &["fsck", "--full"])?;
    Ok(())
}

#[test]
fn explicit_head_source_preserves_git_hook_input_and_rejection() -> Result<()> {
    let repo = TestRepo::new("pre-push-head")?;
    let remote = remote(repo.path())?;
    for detached in [false, true] {
        if detached {
            git(repo.path(), &["checkout", "--detach"])?;
        }
        let destination = if detached {
            "refs/heads/detached"
        } else {
            "refs/heads/attached"
        };
        let spec = format!("HEAD:{destination}");
        let args = ["push", "origin", &spec];
        let before = repo.head_id()?;
        hook(repo.path(), "cat >hook-input; exit 1")?;
        let stock = gix_testtools::git_command(repo.path()).args(args).output()?;
        ensure!(!stock.status.success(), "stock hook rejects");
        let expected = fs::read(repo.path().join("hook-input"))?;
        fs::remove_file(repo.path().join("hook-input"))?;
        let output = brit(repo.path(), &args)?;
        ensure!(!output.status.success(), "Brit hook rejects");
        ensure!(
            repo.path().join("hook-input").is_file(),
            "hook must run: {}",
            output.stderr
        );
        assert_eq!(fs::read(repo.path().join("hook-input"))?, expected);
        assert_eq!(
            git(remote.path(), &["for-each-ref", "--format=%(refname)", destination])?,
            ""
        );
        assert_eq!(repo.head_id()?, before);
        hook(repo.path(), "cat >hook-input")?;
        let output = brit(repo.path(), &args)?;
        ensure!(output.status.success(), "{}", output.stderr);
        assert_eq!(fs::read(repo.path().join("hook-input"))?, expected);
        assert_eq!(git(remote.path(), &["rev-parse", destination])?, before);
        assert_eq!(repo.head_id()?, before);
    }
    Ok(())
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
fn new_branch_reuses_history_advertised_under_another_ref() -> Result<()> {
    let repo = TestRepo::new("pre-push-advertised-history")?;
    let remote = remote(repo.path())?;
    repo.commit_file("already-present", &"old content\n".repeat(200_000))?;
    git(repo.path(), &["push", "origin", "main:main"])?;
    let common = repo.head_id()?;
    for (destination, advance) in [("copy", false), ("next", true)] {
        if advance {
            repo.commit_file("new-small-file", "new content")?;
        }
        let spec = format!("HEAD:refs/heads/{destination}");
        let output = brit(
            repo.path(),
            &["-c", "gitoxide.objects.allocLimit=1024", "push", "origin", &spec],
        )?;
        ensure!(output.status.success(), "{}", output.stderr);
        assert_eq!(git(remote.path(), &["rev-parse", destination])?, repo.head_id()?);
        assert_eq!(git(remote.path(), &["rev-parse", "main"])?, common);
        git(remote.path(), &["fsck", "--full"])?;
    }
    Ok(())
}

#[test]
fn initial_push_reads_blobs_above_the_configured_allocation_limit() -> Result<()> {
    let repo = TestRepo::new("pre-push-bounded-blob")?;
    let remote = remote(repo.path())?;
    repo.commit_file("large", &"transfer content\n".repeat(8192))?;
    let output = brit(
        repo.path(),
        &["-c", "gitoxide.objects.allocLimit=1024", "push", "origin", "main:main"],
    )?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    assert_eq!(
        git(remote.path(), &["show", "main:large"])?,
        git(repo.path(), &["show", "HEAD:large"])?
    );
    git(remote.path(), &["fsck", "--full"])?;
    Ok(())
}

fn push_commit_metadata(size: usize, reduced: bool) -> Result<()> {
    let repo = TestRepo::new("pre-push-bounded-commit")?;
    let message = repo.path().join(".git/large-message");
    let mut file = fs::File::create(&message)?;
    file.write_all(b"short subject\n\n")?;
    let chunk = [b'x'; 8192];
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    file.write_all(b"\n")?;
    drop(file);
    git(repo.path(), &["commit", "--allow-empty", "-F", ".git/large-message"])?;
    let before = repo.head_id()?;
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(repo.path())?.uid() == 0, "requires chown authority");
            chown(repo.path(), Some(65534), None)?;
            chown(repo.path().join(".git"), Some(65534), None)?;
            let trust = brit(repo.path(), &["free", "trust", ".", ".git"])?;
            ensure!(trust.status.success(), "{}", trust.stderr);
            ensure!(trust.stdout.lines().filter(|line| line.starts_with("Reduced")).count() == 2);
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("ownership qualification runs on Linux");
    }
    for packed in [false, true] {
        if packed {
            git(repo.path(), &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("1073741824"), Some("16777216"), Some("1024")] {
            let remote = remote(repo.path())?;
            let mut args = Vec::new();
            let setting;
            if let Some(limit) = limit {
                setting = format!("gitoxide.objects.allocLimit={limit}");
                args.extend(["-c", &setting]);
            }
            // Reduced trust intentionally ignores foreign repository remote config.
            let destination = if reduced {
                remote.path().to_str().expect("receiver path")
            } else {
                "origin"
            };
            args.extend(["push", destination, "main:main"]);
            let output = brit(repo.path(), &args)?;
            ensure!(
                output.status.success(),
                "packed {packed}, limit {limit:?}: {}",
                output.stderr
            );
            assert_eq!(git(remote.path(), &["rev-parse", "main"])?, before);
            assert_eq!(
                git(remote.path(), &["cat-file", "commit", "main"])?,
                git(repo.path(), &["cat-file", "commit", "HEAD"])?
            );
            git(remote.path(), &["fsck", "--full"])?;
            git(repo.path(), &["remote", "remove", "origin"])?;
        }
    }
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD"])?, before);
    Ok(())
}

#[test]
fn initial_push_reads_commit_metadata_above_the_configured_allocation_limit() -> Result<()> {
    push_commit_metadata(128 * 1024, false)
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN"]
fn initial_push_twenty_mib_commit_metadata() -> Result<()> {
    push_commit_metadata(20 * 1024 * 1024, false)
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "resource qualification: requires BRIT_BIN and chown authority"]
fn initial_push_twenty_mib_commit_metadata_reduced_trust() -> Result<()> {
    push_commit_metadata(20 * 1024 * 1024, true)
}

#[test]
fn initial_push_compares_nested_trees_above_the_allocation_limit() -> Result<()> {
    let repo = TestRepo::new("pre-push-bounded-tree")?;
    let remote = remote(repo.path())?;
    fs::create_dir(repo.path().join("dir"))?;
    for n in 0..128 {
        fs::write(repo.path().join(format!("dir/file-{n:03}")), b"first\n")?;
    }
    git(repo.path(), &["add", "dir"])?;
    git(repo.path(), &["commit", "-m", "wide nested tree"])?;
    repo.commit_file("dir/file-064", "changed\n")?;
    let output = brit(
        repo.path(),
        &["-c", "gitoxide.objects.allocLimit=1024", "push", "origin", "main:main"],
    )?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    assert_eq!(
        git(remote.path(), &["ls-tree", "-r", "main"])?,
        git(repo.path(), &["ls-tree", "-r", "HEAD"])?
    );
    git(remote.path(), &["fsck", "--full"])?;
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
        "(pwd -W 2>/dev/null || pwd) >hook-cwd; git rev-parse --absolute-git-dir >hook-git-dir; cat >hook-input",
    )?;
    fs::copy(
        repo.path().join("hooks with spaces/pre-push"),
        repo.path().join(".git/hooks/pre-push"),
    )?;
    git(repo.path(), &["config", "--unset", "core.hooksPath"])?;
    fs::create_dir(linked.path().join("nested"))?;
    let output = brit(&linked.path().join("nested"), &["push", "origin", "linked:linked"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    // pwd -W records a native path under Git for Windows; other shells use pwd.
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
        let diagnostic = if option == "--force-with-lease" {
            "explicit lease requires refs/heads/<branch>:<full-expected-oid>"
        } else {
            "git push"
        };
        ensure!(output.stderr.contains(diagnostic), "{}", output.stderr);
        assert_eq!(git(remote.path(), &["rev-parse", "main"])?, old);
    }
    let output = brit(repo.path(), &["push", "--force", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    Ok(())
}

/// A push that delivered its refs exits 0 even when its reader closed the stream its report goes
/// to: stderr for the human report, stdout for `--porcelain`. The refs moved; a lost report does
/// not turn that into a failure. Runs with and without a hook, so both the first connection and
/// the re-validated one are covered.
#[cfg(target_os = "linux")]
#[test]
fn delivered_push_succeeds_when_its_reader_closed_the_report_stream() -> Result<()> {
    use std::os::fd::FromRawFd;
    let repo = TestRepo::new("pre-push-closed-reader")?;
    let remote = remote(repo.path())?;
    for (stream, porcelain, gated) in [(2, false, false), (2, false, true), (1, true, false), (1, true, true)] {
        if gated {
            hook(repo.path(), "cat >hook-input")?;
        } else {
            git(repo.path(), &["config", "--unset", "core.hooksPath"]).ok();
        }
        let new = repo.commit_file(
            "payload",
            &format!("closed {stream} porcelain {porcelain} hook {gated}"),
        )?;
        let mut fds = [0; 2];
        // SAFETY: `fds` has room for the two descriptors `pipe` writes.
        ensure!(unsafe { libc::pipe(fds.as_mut_ptr()) } == 0, "pipe(2) failed");
        // SAFETY: both descriptors were just created by `pipe` and are owned here exactly once.
        let (reader, writer) = unsafe { (fs::File::from_raw_fd(fds[0]), fs::File::from_raw_fd(fds[1])) };
        drop(reader);
        let mut command = std::process::Command::new(brit_bin().expect("required binary"));
        gix_testtools::configure_git_environment(&mut command, repo.path());
        command.current_dir(repo.path()).arg("push");
        if porcelain {
            command.arg("--porcelain");
        }
        command.args(["origin", "main:main"]);
        if stream == 1 {
            command.stdout(writer);
        } else {
            command.stderr(writer);
        }
        let output = command.output()?;
        let what = format!("fd {stream} closed, porcelain {porcelain}, hook {gated}");
        assert_eq!(output.status.code(), Some(0), "{what}: {output:?}");
        assert_eq!(git(remote.path(), &["rev-parse", "main"])?, new, "{what}: delivered");
    }
    Ok(())
}

/// Git pushes ungated, without a word, when `core.hooksPath` names a missing directory or the
/// `pre-push` hook cannot be executed. Brit refuses, naming the path and `--no-verify`, and sends
/// nothing; `--no-verify` is the explicit way to push without the hook.
#[cfg(unix)]
#[test]
fn configured_hooks_that_cannot_run_refuse_the_push() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let repo = TestRepo::new("pre-push-ungated")?;
    let remote = remote(repo.path())?;
    let hooks = repo.path().join("hooks with spaces");
    let pre_push = hooks.join("pre-push");
    // The path is named as the repository spells it (`<worktree>/./<core.hooksPath>`).
    let refused = |cause: &str, path: &str| -> Result<()> {
        let output = brit(repo.path(), &["push", "origin", "main:main"])?;
        ensure!(!output.status.success(), "{cause}: the push must be refused");
        for needle in [path, cause, "--no-verify"] {
            ensure!(
                output.stderr.contains(needle),
                "{cause}: expected {needle:?} in\n{}",
                output.stderr
            );
        }
        assert_eq!(git(remote.path(), &["for-each-ref"])?, "", "{cause}: nothing was sent");
        Ok(())
    };

    git(repo.path(), &["config", "core.hooksPath", "hooks with spaces"])?;
    refused("not a directory", "hooks with spaces,")?;
    hook(repo.path(), "exit 0")?;
    fs::set_permissions(&pre_push, fs::Permissions::from_mode(0o644))?;
    refused("not an executable file", "hooks with spaces/pre-push is")?;
    fs::remove_file(&pre_push)?;
    std::os::unix::fs::symlink(hooks.join("nowhere"), &pre_push)?;
    refused("not an executable file", "hooks with spaces/pre-push is")?;

    let output = brit(repo.path(), &["push", "--no-verify", "origin", "main:main"])?;
    ensure!(
        output.status.success(),
        "--no-verify pushes without the hook: {}",
        output.stderr
    );
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    // A configured, existing directory without a pre-push hook is not gated: nothing to refuse.
    fs::remove_file(&pre_push)?;
    let new = repo.commit_file("payload", "no hook installed")?;
    let output = brit(repo.path(), &["push", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, new);
    Ok(())
}

/// A notes ref fast-forwards beside a branch in one push; a notes rewrite and an existing tag's
/// move are refused before the hook, and `--force` moves the tag.
#[test]
fn notes_fast_forward_beside_a_branch_while_tags_stay_put() -> Result<()> {
    let repo = TestRepo::new("pre-push-notes")?;
    let remote = remote(repo.path())?;
    let notes = "refs/notes/brit/devices/d/build/x";
    let notes_spec = format!("{notes}:{notes}");
    git(
        repo.path(),
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "notes",
            "--ref",
            notes,
            "add",
            "-m",
            "first",
            "HEAD",
        ],
    )?;
    git(repo.path(), &["tag", "v1"])?;
    let output = brit(
        repo.path(),
        &["push", "origin", "main:main", &notes_spec, "refs/tags/v1:refs/tags/v1"],
    )?;
    ensure!(output.status.success(), "creation: {}", output.stderr);

    repo.commit_file("payload", "second")?;
    git(
        repo.path(),
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "notes",
            "--ref",
            notes,
            "add",
            "-m",
            "second",
            "HEAD",
        ],
    )?;
    let output = brit(repo.path(), &["push", "origin", "main:main", &notes_spec])?;
    ensure!(
        output.status.success(),
        "branch and notes fast-forward: {}",
        output.stderr
    );
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, repo.head_id()?);
    assert_eq!(
        git(remote.path(), &["rev-parse", notes])?,
        git(repo.path(), &["rev-parse", notes])?
    );

    let pushed_notes = git(remote.path(), &["rev-parse", notes])?;
    let tree = git(repo.path(), &["rev-parse", &format!("{notes}^{{tree}}")])?;
    let unrelated = git(
        repo.path(),
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit-tree",
            &tree,
            "-m",
            "rewritten notes",
        ],
    )?;
    git(repo.path(), &["update-ref", notes, &unrelated])?;
    let output = brit(repo.path(), &["push", "origin", &notes_spec])?;
    ensure!(!output.status.success(), "a notes rewrite is not a fast-forward");
    ensure!(
        output.stderr.contains("refusing unverified fast-forward update"),
        "{}",
        output.stderr
    );
    assert_eq!(git(remote.path(), &["rev-parse", notes])?, pushed_notes);

    let tagged = git(remote.path(), &["rev-parse", "refs/tags/v1"])?;
    git(repo.path(), &["tag", "-f", "v1", "HEAD"])?;
    let output = brit(repo.path(), &["push", "origin", "refs/tags/v1:refs/tags/v1"])?;
    ensure!(!output.status.success(), "an existing tag does not move without force");
    ensure!(
        output.stderr.contains("refusing to update existing tag refs/tags/v1"),
        "{}",
        output.stderr
    );
    assert_eq!(git(remote.path(), &["rev-parse", "refs/tags/v1"])?, tagged);
    let output = brit(repo.path(), &["push", "--force", "origin", "refs/tags/v1:refs/tags/v1"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "refs/tags/v1"])?, repo.head_id()?);
    Ok(())
}

/// `--atomic` asks the remote for all or nothing: one ref its `update` hook rejects keeps every
/// other ref where it was, where a plain push lands the rest. A remote that does not offer
/// `atomic` refuses the push before anything is sent, as Git does.
#[cfg(unix)]
#[test]
fn atomic_push_lands_every_ref_or_none() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let repo = TestRepo::new("pre-push-atomic")?;
    let remote = remote(repo.path())?;
    let base = repo.head_id()?;
    git(repo.path(), &["push", "origin", "main:main", "main:refused"])?;
    let update = remote.path().join("hooks/update");
    fs::write(&update, "#!/bin/sh\ntest \"$1\" != refs/heads/refused\n")?;
    fs::set_permissions(&update, fs::Permissions::from_mode(0o755))?;
    let new = repo.commit_file("payload", "atomic")?;
    let specs = ["origin", "main:main", "main:refused"];

    let output = brit(repo.path(), &[&["push", "--atomic"][..], &specs].concat())?;
    ensure!(!output.status.success(), "the rejected ref fails the atomic push");
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, base, "atomic: main stays");
    assert_eq!(git(remote.path(), &["rev-parse", "refused"])?, base);

    let output = brit(repo.path(), &[&["push"][..], &specs].concat())?;
    ensure!(!output.status.success(), "the rejected ref is reported");
    assert_eq!(
        git(remote.path(), &["rev-parse", "main"])?,
        new,
        "non-atomic: main lands"
    );
    assert_eq!(git(remote.path(), &["rev-parse", "refused"])?, base);

    git(remote.path(), &["config", "receive.advertiseAtomic", "false"])?;
    let next = repo.commit_file("payload", "no atomic offered")?;
    let output = brit(repo.path(), &["push", "--atomic", "origin", "main:main"])?;
    assert_eq!(output.status.code(), Some(128), "{}", output.stderr);
    ensure!(
        output
            .stderr
            .contains("the receiving end does not support --atomic push"),
        "{}",
        output.stderr
    );
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, new);
    let output = brit(repo.path(), &["push", "origin", "main:main"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "main"])?, next);
    Ok(())
}
