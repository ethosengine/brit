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
    git(repo, &["config", "core.hooksPath", "hooks with spaces"])?;
    write_hook(&repo.join("hooks with spaces"), body)
}

fn write_hook(dir: &Path, body: &str) -> Result<()> {
    fs::create_dir_all(dir)?;
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
    let base = repo.head_id()?;
    git(repo.path(), &["push", "origin", "HEAD:dev"])?;
    let commit_id = repo.commit_file("payload", "landing commit")?;
    repo.commit_file("payload", "later commit")?;
    assert_eq!(
        git(repo.path(), &["for-each-ref", "--points-at", &commit_id])?,
        "",
        "the landing source is named only by its object ID"
    );
    let spec = format!("{commit_id}:dev");
    hook(repo.path(), "cat >hook-input")?;
    let output = brit(repo.path(), &["push", "origin", &spec])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(
        fs::read_to_string(repo.path().join("hook-input"))?,
        format!("{commit_id} {commit_id} refs/heads/dev {base}\n")
    );
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/dev"])?, commit_id);

    fs::remove_file(repo.path().join("hook-input"))?;
    let zero_spec = format!("{}:dev", "0".repeat(commit_id.len()));
    let output = brit(repo.path(), &["push", "origin", &zero_spec])?;
    ensure!(!output.status.success(), "a zero object ID must not delete dev");
    ensure!(
        !repo.path().join("hook-input").exists(),
        "invalid source reached the hook"
    );
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/dev"])?, commit_id);
    Ok(())
}

#[test]
fn head_landing_sends_the_object_the_hook_judged() -> Result<()> {
    let repo = TestRepo::new("pre-push-pinned-head")?;
    let remote = remote(repo.path())?;
    let base = repo.head_id()?;
    git(repo.path(), &["push", "origin", "HEAD:dev"])?;
    let judged = repo.commit_file("payload", "judged")?;
    hook(
        repo.path(),
        "set -eu
cat >hook-input
sleep 1
git commit --allow-empty -m 'advance the branch during pre-push'",
    )?;

    let output = brit(repo.path(), &["push", "origin", "HEAD:dev"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_eq!(
        git(repo.path(), &["rev-parse", "HEAD^"])?,
        judged,
        "the hook advanced the checked-out branch"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("hook-input"))?,
        format!("HEAD {judged} refs/heads/dev {base}\n")
    );
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/dev"])?, judged);
    Ok(())
}

#[test]
fn command_config_selects_the_linked_worktree_hook() -> Result<()> {
    let repo = TestRepo::new("pre-push-command-hooks-path")?;
    let remote = remote(repo.path())?;
    let linked = tempfile::tempdir()?;
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            "landing",
            linked.path().to_str().expect("temp path"),
        ],
    )?;
    hook(repo.path(), "echo shared >shared-hook-ran; exit 1")?;
    let shared_hooks = repo.path().join("hooks with spaces");
    git(
        repo.path(),
        &["config", "core.hooksPath", shared_hooks.to_str().expect("temp path")],
    )?;
    let landing_hooks = linked.path().join(".husky/_");
    write_hook(&landing_hooks, "cat >landing-hook-input")?;
    let config = format!("core.hooksPath={}", landing_hooks.display());

    let output = brit(linked.path(), &["-c", &config, "push", "origin", "HEAD:dev"])?;
    ensure!(output.status.success(), "{}", output.stderr);
    let commit_id = repo.head_id()?;
    assert_eq!(
        fs::read_to_string(linked.path().join("landing-hook-input"))?,
        format!("HEAD {commit_id} refs/heads/dev {}\n", "0".repeat(commit_id.len()))
    );
    ensure!(
        !linked.path().join("shared-hook-ran").exists() && !repo.path().join("shared-hook-ran").exists(),
        "the shared hook did not run"
    );
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/dev"])?, commit_id);
    Ok(())
}

/// Stated contract for landing agents: the hook's parent is the pushing brit, `BRIT_GUARD_HELD_BY`
/// reads `v1 <that pid> <its start ticks> …`, and that process sends and stays alive until the
/// remote ref has moved — so waiting on it is waiting on the landing.
#[cfg(target_os = "linux")]
#[test]
fn hook_parent_is_the_marked_brit_that_sends_and_outlives_the_ref_move() -> Result<()> {
    let repo = TestRepo::new("pre-push-pusher-identity")?;
    let remote = remote(repo.path())?;
    let pushed = repo.head_id()?;
    hook(
        repo.path(),
        r#"set -eu
cat >/dev/null
here=$PWD
printf '%s\n' "$BRIT_GUARD_HELD_BY" >"$here/marker"
printf '%s\n' "$PPID" >"$here/parent"
cat "/proc/$PPID/stat" >"$here/parent-stat"
origin=$(git remote get-url origin)
git --git-dir="$origin" rev-parse --verify -q refs/heads/dev >"$here/at-hook" || printf 'absent\n' >"$here/at-hook"
(
    while kill -0 "$PPID" 2>/dev/null; do sleep 0.02; done
    git --git-dir="$origin" rev-parse refs/heads/dev >"$here/at-exit.tmp"
    mv "$here/at-exit.tmp" "$here/at-exit"
) </dev/null >/dev/null 2>&1 &"#,
    )?;

    let output = brit(repo.path(), &["push", "origin", "HEAD:dev"])?;
    ensure!(output.status.success(), "{}", output.stderr);

    let at_exit = repo.path().join("at-exit");
    let started = Instant::now();
    while !at_exit.exists() {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "watcher never saw brit exit"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    let parent = fs::read_to_string(repo.path().join("parent"))?.trim().to_owned();
    let stat = fs::read_to_string(repo.path().join("parent-stat"))?;
    // Field 2 (comm) may hold spaces; fields after its closing parenthesis start at field 3.
    let after_comm = &stat[stat.rfind(')').expect("stat names comm") + 2..];
    let start_ticks = after_comm.split(' ').nth(22 - 3).expect("stat field 22").to_owned();
    let marker = fs::read_to_string(repo.path().join("marker"))?;
    let fields: Vec<&str> = marker.split_whitespace().collect();
    assert_eq!(fields.first(), Some(&"v1"), "marker version: {marker}");
    assert_eq!(
        fields.get(1),
        Some(&parent.as_str()),
        "marker pid is the hook's parent: {marker}"
    );
    assert_eq!(
        fields.get(2),
        Some(&start_ticks.as_str()),
        "marker start ticks: {marker}"
    );
    assert!(
        stat.contains("(brit)"),
        "the hook's parent is brit itself, not a shell: {stat}"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("at-hook"))?.trim(),
        "absent",
        "the remote moved before the hook returned"
    );
    assert_eq!(
        fs::read_to_string(at_exit)?.trim(),
        pushed,
        "brit exited before the remote ref moved"
    );
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/dev"])?, pushed);
    Ok(())
}

#[test]
fn hook_inherits_gate_environment_and_streams_both_outputs_live() -> Result<()> {
    let repo = TestRepo::new("pre-push-live-hook")?;
    let remote = remote(repo.path())?;
    hook(
        repo.path(),
        r#"set -eu
cat >/dev/null
printf '%s\n' "$GATE_EVIDENCE" "$GATE_LANDING_INTENT" "$GATE_CARRY_WAIT" "$HUSKY" >gate-env
printf '[pre-push] live\n'
printf '[gate-live] live\n' >&2
for attempt in 1 2 3 4 5 6 7 8 9 10; do
    [ ! -f hook-release ] || exit 0
    sleep 1
done
exit 1"#,
    )?;
    let output_dir = tempfile::tempdir()?;
    let stdout = output_dir.path().join("stdout");
    let stderr = output_dir.path().join("stderr");
    let mut command = std::process::Command::new(brit_bin().expect("required binary"));
    gix_testtools::configure_git_environment(&mut command, repo.path());
    let mut child = command
        .env("GIT_CONFIG_GLOBAL", output_dir.path().join("missing.gitconfig"))
        .envs([
            ("GATE_EVIDENCE", "reuse"),
            ("GATE_LANDING_INTENT", "land dev"),
            ("GATE_CARRY_WAIT", "37"),
            ("HUSKY", "0"),
        ])
        .current_dir(repo.path())
        .args(["push", "origin", "HEAD:dev"])
        .stdout(fs::File::create(&stdout)?)
        .stderr(fs::File::create(&stderr)?)
        .spawn()?;
    let observed = (|| -> Result<()> {
        let started = Instant::now();
        let mut released = false;
        loop {
            if let Some(status) = child.try_wait()? {
                let errors = fs::read_to_string(&stderr)?;
                ensure!(released, "push exited before both live lines were observed: {errors}");
                ensure!(status.success(), "{errors}");
                return Ok(());
            }
            if !released
                && fs::read_to_string(&stdout)?.contains("[pre-push] live\n")
                && fs::read_to_string(&stderr)?.contains("[gate-live] live\n")
            {
                fs::write(repo.path().join("hook-release"), b"")?;
                released = true;
            }
            ensure!(started.elapsed() < Duration::from_secs(20), "push timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    })();
    if observed.is_err() {
        child.kill().ok();
        child.wait().ok();
    }
    observed?;
    assert_eq!(
        fs::read_to_string(repo.path().join("gate-env"))?,
        "reuse\nland dev\n37\n0\n"
    );
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/dev"])?, repo.head_id()?);
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
    let commit_id = repo.head_id()?;
    // The path is named as the repository spells it (`<worktree>/./<core.hooksPath>`).
    let refused = |cause: &str, path: &str| -> Result<()> {
        git(repo.path(), &["config", "core.hooksPath", "hooks with spaces"])?;
        for (source, args) in [
            ("config", &["push", "origin", "main:main"][..]),
            (
                "-c",
                &["-c", "core.hooksPath=hooks with spaces", "push", "origin", "main:main"][..],
            ),
        ] {
            let output = brit(repo.path(), args)?;
            ensure!(!output.status.success(), "{source}, {cause}: the push must be refused");
            for needle in [path, cause, "--no-verify"] {
                ensure!(
                    output.stderr.contains(needle),
                    "{source}, {cause}: expected {needle:?} in\n{}",
                    output.stderr
                );
            }
            assert_eq!(git(remote.path(), &["for-each-ref"])?, "", "{source}: no ref moved");
            ensure!(
                git(remote.path(), &["cat-file", "-e", &commit_id]).is_err(),
                "{source}: the commit was not sent"
            );
            if source == "config" {
                git(repo.path(), &["config", "--unset", "core.hooksPath"])?;
            }
        }
        Ok(())
    };

    refused("not a directory", "hooks with spaces,")?;
    write_hook(&hooks, "exit 0")?;
    fs::set_permissions(&pre_push, fs::Permissions::from_mode(0o644))?;
    refused("not an executable file", "hooks with spaces/pre-push is")?;
    fs::remove_file(&pre_push)?;
    std::os::unix::fs::symlink(hooks.join("nowhere"), &pre_push)?;
    refused("not an executable file", "hooks with spaces/pre-push is")?;

    git(repo.path(), &["config", "core.hooksPath", "hooks with spaces"])?;
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

/// An existing tag's move is refused before the hook, and `--force` moves the tag.
#[test]
fn tags_stay_put_without_force() -> Result<()> {
    let repo = TestRepo::new("pre-push-tags")?;
    let remote = remote(repo.path())?;
    git(repo.path(), &["tag", "v1"])?;
    let output = brit(
        repo.path(),
        &["push", "origin", "main:main", "refs/tags/v1:refs/tags/v1"],
    )?;
    ensure!(output.status.success(), "creation: {}", output.stderr);

    repo.commit_file("payload", "second")?;
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

/// An annotated tag reaches the remote as its tag object, as Git sends it: the annotation, a tag
/// of a tag, and the commit history behind a tag whose commit the remote lacks all arrive. The
/// hook sees the tag object id, a repeated push is up to date, and `--tags` sends the same way.
#[test]
fn annotated_tags_arrive_as_tag_objects() -> Result<()> {
    let repo = TestRepo::new("pre-push-annotated-tags")?;
    let remote = remote(repo.path())?;
    git(repo.path(), &["push", "origin", "main:main"])?;
    let zero = "0".repeat(40);
    let annotate = |name: &str, message: &str, target: &str| {
        git(
            repo.path(),
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "tag",
                "-a",
                name,
                "-m",
                message,
                target,
            ],
        )
    };
    let assert_tag_arrived = |name: &str, message: &str| -> Result<()> {
        let local = git(repo.path(), &["rev-parse", &format!("refs/tags/{name}")])?;
        assert_eq!(
            git(remote.path(), &["rev-parse", &format!("refs/tags/{name}")])?,
            local,
            "{name}: the remote holds the tag object itself"
        );
        assert_eq!(git(remote.path(), &["cat-file", "-t", &local])?, "tag", "{name}");
        ensure!(
            git(remote.path(), &["cat-file", "tag", &local])?.contains(message),
            "{name}: the annotation arrived"
        );
        Ok(())
    };

    annotate("v1", "release notes v1", "HEAD")?;
    annotate("outer", "a tag of a tag", "v1")?;
    hook(repo.path(), "cat >hook-input")?;
    let specs = [
        "push",
        "origin",
        "refs/tags/v1:refs/tags/v1",
        "refs/tags/outer:refs/tags/outer",
    ];
    let output = brit(repo.path(), &specs)?;
    ensure!(output.status.success(), "{}", output.stderr);
    assert_tag_arrived("v1", "release notes v1")?;
    assert_tag_arrived("outer", "a tag of a tag")?;
    let v1 = git(repo.path(), &["rev-parse", "refs/tags/v1"])?;
    ensure!(
        fs::read_to_string(repo.path().join("hook-input"))?.contains(&format!("refs/tags/v1 {v1} refs/tags/v1 {zero}")),
        "the hook sees the tag object id"
    );
    let output = brit(repo.path(), &specs)?;
    ensure!(
        output.status.success(),
        "a repeated push is up to date: {}",
        output.stderr
    );

    // A tag whose commit the remote lacks brings that commit's history with it.
    let unpushed = repo.commit_file("payload", "only behind a tag")?;
    annotate("v2", "release notes v2", "HEAD")?;
    let output = brit(repo.path(), &["push", "--tags", "origin"])?;
    ensure!(output.status.success(), "--tags: {}", output.stderr);
    assert_tag_arrived("v2", "release notes v2")?;
    assert_eq!(git(remote.path(), &["rev-parse", "refs/tags/v2^{commit}"])?, unpushed);
    assert_eq!(
        git(remote.path(), &["show", "refs/tags/v2:payload"])?,
        "only behind a tag"
    );
    git(remote.path(), &["fsck", "--full", "--strict"])?;
    Ok(())
}

/// This repository's own reach adapter (`.githooks/pre-push`) lets a release tag through only as a
/// creation whose peeled commit the remote's `main` already holds; a branch push keeps the
/// evaluator path, and a tag move is refused even with `--force`.
#[cfg(unix)]
#[test]
fn reach_adapter_admits_tag_creation_at_a_reached_commit_only() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    const ADAPTER: &str = include_str!("../../../.githooks/pre-push");
    let executable = |path: &Path, text: &str| -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        Ok(())
    };
    let repo = TestRepo::new("pre-push-reach-tag")?;
    let remote = remote(repo.path())?;
    let base = repo.head_id()?;
    let bin = tempfile::tempdir()?;
    let calls = bin.path().join("epr-calls");
    executable(
        &bin.path().join("epr"),
        &format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n", calls.display()),
    )?;
    executable(&repo.path().join(".githooks/pre-push"), ADAPTER)?;
    executable(&repo.path().join("scripts/ci/rust-style.sh"), "#!/bin/sh\nexit 0\n")?;
    git(repo.path(), &["add", ".githooks/pre-push", "scripts/ci/rust-style.sh"])?;
    git(repo.path(), &["commit", "-q", "-m", "adapter"])?;
    git(repo.path(), &["config", "core.hooksPath", ".githooks"])?;
    git(repo.path(), &["config", "branch.main.britReachBase", &base])?;
    let path = format!("{}:{}", bin.path().display(), std::env::var("PATH")?);
    let push = |args: &[&str]| -> Result<Capture> {
        BritInvocation::new(brit_bin().expect("required binary"))
            .arg("push")
            .args(args.iter().copied())
            .env("PATH", &path)
            .current_dir(repo.path())
            .run()
    };
    let remote_tag = |name: &str| {
        git(
            remote.path(),
            &["for-each-ref", "--format=%(*objectname)%(objectname)", name],
        )
        .map(|ids| ids.get(..40).unwrap_or_default().to_owned())
    };

    // A branch push still goes through the evaluator.
    let output = push(&["origin", "main:main"])?;
    ensure!(output.status.success(), "first branch push: {}", output.stderr);
    ensure!(fs::read_to_string(&calls)?.contains("ready --target"), "evaluator ran");
    git(repo.path(), &["fetch", "-q", "origin"])?;
    fs::remove_file(&calls)?;

    // Annotated and lightweight tags at reached commits are created without the evaluator.
    git(repo.path(), &["tag", "-a", "v1", "-m", "release"])?;
    git(repo.path(), &["tag", "v0", &base])?;
    let output = push(&["origin", "refs/tags/v1:refs/tags/v1", "refs/tags/v0:refs/tags/v0"])?;
    ensure!(output.status.success(), "reached tags: {}", output.stderr);
    assert_eq!(
        remote_tag("refs/tags/v1")?,
        git(repo.path(), &["rev-parse", "refs/tags/v1^{commit}"])?
    );
    assert_eq!(
        git(remote.path(), &["rev-parse", "refs/tags/v1"])?,
        git(repo.path(), &["rev-parse", "refs/tags/v1"])?,
        "the release tag arrives as its annotated tag object"
    );
    assert_eq!(git(remote.path(), &["cat-file", "-t", "refs/tags/v1"])?, "tag");
    assert_eq!(remote_tag("refs/tags/v0")?, base);
    ensure!(!calls.exists(), "a tag creation needs no evaluator");

    // A tag at a commit the remote main does not hold is refused.
    let unreached = repo.commit_file("payload", "not yet on main")?;
    git(repo.path(), &["tag", "v2"])?;
    let output = push(&["origin", "refs/tags/v2:refs/tags/v2"])?;
    ensure!(!output.status.success(), "an unreached tag must be refused");
    ensure!(
        output.stderr.contains("Brit reach refused") && output.stderr.contains("reachable"),
        "{}",
        output.stderr
    );
    assert_eq!(remote_tag("refs/tags/v2")?, "");

    // Once main carries it, the same tag passes: the branch rule and the tag rule are separate.
    let output = push(&["origin", "main:main"])?;
    ensure!(output.status.success(), "branch update: {}", output.stderr);
    git(repo.path(), &["fetch", "-q", "origin"])?;
    let output = push(&["origin", "refs/tags/v2:refs/tags/v2"])?;
    ensure!(output.status.success(), "now-reached tag: {}", output.stderr);
    assert_eq!(remote_tag("refs/tags/v2")?, unreached);

    // Moving an existing remote tag is refused by the adapter, even when forced.
    let tagged = remote_tag("refs/tags/v1")?;
    git(repo.path(), &["tag", "-f", "v1", "HEAD"])?;
    let output = push(&["--force", "origin", "refs/tags/v1:refs/tags/v1"])?;
    ensure!(!output.status.success(), "a tag move must be refused");
    ensure!(
        output.stderr.contains("Brit reach refused") && output.stderr.contains("existing tag"),
        "{}",
        output.stderr
    );
    assert_eq!(remote_tag("refs/tags/v1")?, tagged);
    Ok(())
}
