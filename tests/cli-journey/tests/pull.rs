//! Explicit fast-forward pull: stock Git oracles and durable integration boundaries.
#![cfg(target_os = "linux")]
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn command(path: &Path, native: bool) -> Command {
    let mut cmd = if native {
        let mut cmd = Command::new(brit_bin().expect("required Brit executable"));
        gix_testtools::configure_git_environment(&mut cmd, path);
        cmd.current_dir(path);
        cmd
    } else {
        gix_testtools::git_command(path)
    };
    cmd.env("GIT_COMMITTER_NAME", "Pull Journey")
        .env("GIT_COMMITTER_EMAIL", "pull@example.com")
        .env("GIT_COMMITTER_DATE", "2026-01-02 03:04:05 +0000");
    cmd
}
fn run(path: &Path, native: bool, args: &[&str]) -> Result<Output> {
    Ok(command(path, native).args(args).output()?)
}
fn git(path: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = run(path, false, args)?;
    ensure!(
        out.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}
fn success(path: &Path, args: &[&str]) -> Result<Output> {
    let out = run(path, true, args)?;
    ensure!(
        out.status.success(),
        "Brit {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out)
}
struct Fixture {
    remote: TestRepo,
    local: TestRepo,
    original: String,
    target: String,
}
fn fixture() -> Result<Fixture> {
    let remote = TestRepo::new("pull-remote")?;
    remote.commit_file("selected", "original\n")?;
    remote.commit_file("neighbor", "original neighbor\n")?;
    let original = remote.commit_file(".gitignore", "ignored\n")?;
    let local = TestRepo::new("pull-local")?;
    git(
        local.path(),
        &[
            "remote",
            "add",
            "origin",
            remote.path().to_str().expect("fixture UTF-8"),
        ],
    )?;
    git(local.path(), &["fetch", "origin"])?;
    git(local.path(), &["reset", "--hard", "origin/main"])?;
    remote.commit_file("selected", "destination\n")?;
    let target = remote.commit_file("added", "added\n")?;
    Ok(Fixture {
        remote,
        local,
        original,
        target,
    })
}
fn pull(path: &Path, native: bool) -> Result<Output> {
    run(path, native, &["pull", "--ff-only", "origin", "main"])
}
fn assert_success(out: Output) -> Result<()> {
    ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    Ok(())
}
fn index(path: &Path) -> Result<Vec<u8>> {
    Ok(fs::read(path.join(".git/index"))?)
}
fn fetched(f: &Fixture) -> Result<()> {
    assert_eq!(
        String::from_utf8(git(f.local.path(), &["rev-parse", "refs/remotes/origin/main"])?)?.trim(),
        f.target
    );
    ensure!(
        String::from_utf8(fs::read(f.local.path().join(".git/FETCH_HEAD"))?)?.starts_with(&f.target),
        "FETCH_HEAD pins fetched target"
    );
    Ok(())
}
fn compare(a: &Path, b: &Path) -> Result<()> {
    for args in [
        vec!["rev-parse", "HEAD"],
        vec!["rev-parse", "ORIG_HEAD"],
        vec!["ls-files", "--stage", "-z"],
        vec!["status", "--porcelain=v1", "-z"],
    ] {
        assert_eq!(git(a, &args)?, git(b, &args)?, "Git oracle {args:?}");
    }
    for name in ["selected", "neighbor", "added", "private", "ignored"] {
        assert_eq!(fs::read(a.join(name)).ok(), fs::read(b.join(name)).ok(), "file {name}");
    }
    Ok(())
}
#[test]
fn attached_and_detached_fast_forward_match_git() -> Result<()> {
    for detached in [false, true] {
        let actual = fixture()?;
        let oracle = fixture()?;
        for f in [&actual, &oracle] {
            if detached {
                git(f.local.path(), &["switch", "--detach"])?;
            }
        }
        assert_success(pull(actual.local.path(), true)?)?;
        assert_success(pull(oracle.local.path(), false)?)?;
        compare(actual.local.path(), oracle.local.path())?;
        for args in [
            vec!["symbolic-ref", "-q", "HEAD"],
            vec!["reflog", "show", "--format=%gs", "HEAD"],
            vec!["reflog", "show", "--format=%gs", "main"],
        ] {
            let a = run(actual.local.path(), false, &args)?;
            let b = run(oracle.local.path(), false, &args)?;
            assert_eq!(a.status.code(), b.status.code());
            assert_eq!(a.stdout, b.stdout, "Git oracle {args:?}");
        }
        fetched(&actual)?;
    }
    Ok(())
}
#[test]
fn staged_unstaged_and_private_neighbors_survive() -> Result<()> {
    let actual = fixture()?;
    let oracle = fixture()?;
    for f in [&actual, &oracle] {
        fs::write(f.local.path().join("neighbor"), "staged\n")?;
        git(f.local.path(), &["add", "neighbor"])?;
        fs::write(f.local.path().join("neighbor"), "unstaged\n")?;
        fs::write(f.local.path().join("private"), "private\n")?;
        fs::write(f.local.path().join("ignored"), "ignored private\n")?;
    }
    assert_success(pull(actual.local.path(), true)?)?;
    assert_success(pull(oracle.local.path(), false)?)?;
    compare(actual.local.path(), oracle.local.path())
}
#[test]
fn equal_and_older_remote_are_noops_with_orig_head_and_without_hook() -> Result<()> {
    for older in [false, true] {
        let actual = fixture()?;
        let oracle = fixture()?;
        for f in [&actual, &oracle] {
            git(f.local.path(), &["fetch", "origin"])?;
            git(f.local.path(), &["reset", "--hard", "origin/main"])?;
            if older {
                f.local.commit_file("local-only", "local\n")?;
            }
            hook(f.local.path(), "#!/bin/sh\necho called >> hook-count\n")?;
        }
        let log = fs::read(actual.local.path().join(".git/logs/HEAD"))?;
        let tracking_log = fs::read(actual.local.path().join(".git/logs/refs/remotes/origin/main"))?;
        assert_success(pull(actual.local.path(), true)?)?;
        assert_success(pull(oracle.local.path(), false)?)?;
        compare(actual.local.path(), oracle.local.path())?;
        assert_eq!(fs::read(actual.local.path().join(".git/logs/HEAD"))?, log);
        assert_eq!(
            fs::read(actual.local.path().join(".git/logs/refs/remotes/origin/main"))?,
            tracking_log,
            "unchanged tracking OID creates no reflog row"
        );
        ensure!(!actual.local.path().join("hook-count").exists());
    }
    Ok(())
}
#[test]
fn divergence_preserves_integration_and_keeps_completed_fetch() -> Result<()> {
    let f = fixture()?;
    f.local.commit_file("local-only", "local\n")?;
    let before = index(f.local.path())?;
    let head = f.local.head_id()?;
    ensure!(!pull(f.local.path(), true)?.status.success());
    assert_eq!(index(f.local.path())?, before);
    assert_eq!(f.local.head_id()?, head);
    assert_eq!(fs::read(f.local.path().join("selected"))?, b"original\n");
    fetched(&f)?;
    ensure!(!f.local.path().join(".git/brit-switch").exists());
    Ok(())
}
#[test]
fn dirty_and_untracked_ignored_overwrite_collisions_preserve_bytes() -> Result<()> {
    for shape in ["unstaged", "staged", "untracked", "ignored", "directory"] {
        let f = fixture()?;
        let name = if matches!(shape, "unstaged" | "staged") {
            "selected"
        } else {
            "added"
        };
        if shape == "directory" {
            fs::create_dir(f.local.path().join(name))?;
            fs::write(f.local.path().join("added/private"), "private\n")?;
        } else {
            fs::write(f.local.path().join(name), "private\n")?;
        }
        if shape == "staged" {
            git(f.local.path(), &["add", name])?;
        }
        if shape == "ignored" {
            fs::write(f.local.path().join(".git/info/exclude"), "added\n")?;
        }
        let before = index(f.local.path())?;
        ensure!(!pull(f.local.path(), true)?.status.success(), "refuse {shape}");
        assert_eq!(index(f.local.path())?, before);
        assert_eq!(f.local.head_id()?, f.original);
        let path = if shape == "directory" { "added/private" } else { name };
        assert_eq!(fs::read(f.local.path().join(path))?, b"private\n");
        fetched(&f)?;
    }
    Ok(())
}
#[test]
fn direct_paths_file_urls_and_full_branch_names_do_not_guess_tracking() -> Result<()> {
    for url in [false, true] {
        let f = fixture()?;
        git(f.local.path(), &["remote", "remove", "origin"])?;
        let remote = if url {
            format!("file://{}", f.remote.path().display())
        } else {
            f.remote.path().display().to_string()
        };
        success(f.local.path(), &["pull", "--ff-only", &remote, "refs/heads/main"])?;
        assert_eq!(f.local.head_id()?, f.target);
        assert_eq!(git(f.local.path(), &["for-each-ref", "refs/remotes"])?, b"");
    }
    Ok(())
}
fn hook(path: &Path, contents: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let file = path.join(".git/hooks/post-merge");
    fs::write(&file, contents)?;
    fs::set_permissions(file, fs::Permissions::from_mode(0o755))?;
    Ok(())
}
#[test]
fn post_merge_arguments_and_failure_match_git_without_rollback() -> Result<()> {
    for exit in [0, 19] {
        let actual = fixture()?;
        let oracle = fixture()?;
        for f in [&actual, &oracle] {
            hook(
                f.local.path(),
                &format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> hook-count\nexit {exit}\n"),
            )?;
        }
        assert_success(pull(actual.local.path(), true)?)?;
        assert_success(pull(oracle.local.path(), false)?)?;
        compare(actual.local.path(), oracle.local.path())?;
        assert_eq!(fs::read(actual.local.path().join("hook-count"))?, b"0\n");
        assert_eq!(actual.local.head_id()?, actual.target);
    }
    Ok(())
}
#[test]
fn unsafe_fetch_mappings_refuse_before_fetching() -> Result<()> {
    for mapping in ["+refs/heads/*:refs/heads/*", "+refs/heads/*:refs/tags/*"] {
        let f = fixture()?;
        git(f.local.path(), &["config", "remote.origin.fetch", mapping])?;
        let before = index(f.local.path())?;
        let fetch = fs::read(f.local.path().join(".git/FETCH_HEAD"))?;
        ensure!(!pull(f.local.path(), true)?.status.success());
        assert_eq!(f.local.head_id()?, f.original);
        assert_eq!(index(f.local.path())?, before);
        assert_eq!(fs::read(f.local.path().join(".git/FETCH_HEAD"))?, fetch);
    }
    Ok(())
}
#[test]
fn unsupported_forms_and_missing_branch_do_not_change_local_state() -> Result<()> {
    for args in [
        vec!["pull"],
        vec!["pull", "origin", "main"],
        vec!["pull", "--rebase", "origin", "main"],
        vec!["pull", "--ff-only", "origin", "missing"],
        vec!["pull", "--ff-only", "origin", "refs/tags/tag"],
        vec!["pull", "--ff-only", "origin", "main:other"],
    ] {
        let f = fixture()?;
        let before = index(f.local.path())?;
        ensure!(
            !run(f.local.path(), true, &args)?.status.success(),
            "unsupported {args:?}"
        );
        assert_eq!(index(f.local.path())?, before);
        assert_eq!(f.local.head_id()?, f.original);
    }
    Ok(())
}
#[test]
fn symlinks_modes_attributes_deletions_and_directory_transitions_match_git() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let actual = fixture()?;
    let oracle = fixture()?;
    for f in [&actual, &oracle] {
        fs::remove_file(f.remote.path().join("selected"))?;
        fs::create_dir(f.remote.path().join("selected"))?;
        fs::write(f.remote.path().join("selected/child"), "child\n")?;
        fs::remove_file(f.remote.path().join("neighbor"))?;
        symlink("added", f.remote.path().join("link"))?;
        fs::set_permissions(f.remote.path().join("added"), fs::Permissions::from_mode(0o755))?;
        fs::write(f.remote.path().join(".gitattributes"), "converted text eol=crlf\n")?;
        fs::write(f.remote.path().join("converted"), "one\ntwo\n")?;
        git(
            f.remote.path(),
            &[
                "add",
                "selected",
                "neighbor",
                "link",
                "added",
                ".gitattributes",
                "converted",
            ],
        )?;
        git(f.remote.path(), &["commit", "-qm", "transitions"])?;
    }
    assert_success(pull(actual.local.path(), true)?)?;
    assert_success(pull(oracle.local.path(), false)?)?;
    compare(actual.local.path(), oracle.local.path())?;
    for name in ["selected/child", "converted"] {
        assert_eq!(
            fs::read(actual.local.path().join(name))?,
            fs::read(oracle.local.path().join(name))?
        );
    }
    assert_eq!(
        fs::read_link(actual.local.path().join("link"))?,
        fs::read_link(oracle.local.path().join("link"))?
    );
    assert_ne!(
        fs::metadata(actual.local.path().join("added"))?.permissions().mode() & 0o111,
        0
    );
    Ok(())
}
#[test]
fn packed_current_branch_and_no_pack_fetch_preserve_ref_consistency() -> Result<()> {
    let actual = fixture()?;
    let oracle = fixture()?;
    for f in [&actual, &oracle] {
        git(f.local.path(), &["fetch", "origin"])?;
        git(f.local.path(), &["pack-refs", "--all", "--prune"])?;
        ensure!(!f.local.path().join(".git/refs/heads/main").exists());
    }
    assert_success(pull(actual.local.path(), true)?)?;
    assert_success(pull(oracle.local.path(), false)?)?;
    compare(actual.local.path(), oracle.local.path())?;
    assert_eq!(
        git(actual.local.path(), &["reflog", "show", "--format=%gs", "main"])?,
        git(oracle.local.path(), &["reflog", "show", "--format=%gs", "main"])?
    );
    Ok(())
}
#[test]
fn foreign_index_and_head_locks_are_never_removed() -> Result<()> {
    for name in ["index.lock", "HEAD.lock", "refs/heads/main.lock"] {
        let f = fixture()?;
        let lock = f.local.path().join(".git").join(name);
        fs::write(&lock, "foreign lock\n")?;
        let before = index(f.local.path())?;
        ensure!(!pull(f.local.path(), true)?.status.success(), "refuse {name}");
        assert_eq!(fs::read(lock)?, b"foreign lock\n");
        assert_eq!(index(f.local.path())?, before);
        assert_eq!(f.local.head_id()?, f.original);
    }
    Ok(())
}
#[test]
fn alternate_indexes_and_external_filters_refuse_without_integration() -> Result<()> {
    for kind in ["alternate", "filter", "sparse", "split", "shallow"] {
        let f = fixture()?;
        match kind {
            "filter" => {
                git(f.local.path(), &["config", "filter.unsafe.smudge", "cat"])?;
            }
            "sparse" => {
                git(f.local.path(), &["update-index", "--skip-worktree", "selected"])?;
            }
            "split" => {
                git(f.local.path(), &["update-index", "--split-index"])?;
            }
            "shallow" => {
                fs::write(f.local.path().join(".git/shallow"), format!("{}\n", f.original))?;
            }
            _ => {}
        }
        let before = index(f.local.path())?;
        let fetch_before = fs::read(f.local.path().join(".git/FETCH_HEAD"))?;
        let tracking_before = git(f.local.path(), &["rev-parse", "refs/remotes/origin/main"])?;
        let mut cmd = command(f.local.path(), true);
        cmd.args(["pull", "--ff-only", "origin", "main"]);
        if kind == "alternate" {
            let alt = f.local.path().join("alternate-index");
            fs::write(&alt, &before)?;
            cmd.env("GIT_INDEX_FILE", alt);
        }
        let out = cmd.output()?;
        ensure!(!out.status.success(), "refuse {kind}");
        assert_eq!(index(f.local.path())?, before);
        assert_eq!(f.local.head_id()?, f.original);
        assert_eq!(
            fs::read(f.local.path().join(".git/FETCH_HEAD"))?,
            fetch_before,
            "pre-fetch refusal {kind}"
        );
        assert_eq!(
            git(f.local.path(), &["rev-parse", "refs/remotes/origin/main"])?,
            tracking_before
        );
    }
    Ok(())
}
#[test]
fn no_mapping_and_no_automatic_tags_are_supported() -> Result<()> {
    let f = fixture()?;
    git(f.remote.path(), &["tag", "selected-tag"])?;
    git(f.local.path(), &["config", "--unset-all", "remote.origin.fetch"])?;
    git(
        f.local.path(),
        &[
            "config",
            "--add",
            "remote.origin.fetch",
            &format!("{}:refs/remotes/origin/object-only", f.original),
        ],
    )?;
    let tracking = git(f.local.path(), &["rev-parse", "refs/remotes/origin/main"])?;
    assert_success(pull(f.local.path(), true)?)?;
    assert_eq!(f.local.head_id()?, f.target);
    ensure!(!run(
        f.local.path(),
        false,
        &["show-ref", "--verify", "refs/remotes/origin/object-only"]
    )?
    .status
    .success());
    assert_eq!(
        git(f.local.path(), &["rev-parse", "refs/remotes/origin/main"])?,
        tracking
    );
    assert_eq!(git(f.local.path(), &["tag", "--list"])?, b"");
    Ok(())
}
#[cfg(feature = "switch-test-faults")]
mod recovery {
    use super::*;
    use std::{
        process::{Child, Stdio},
        time::{Duration, Instant},
    };
    struct Paused {
        child: Child,
        markers: tempfile::TempDir,
    }
    impl Drop for Paused {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
    fn pause(path: &Path, args: &[&str], point: &str) -> Result<Paused> {
        let markers = tempfile::tempdir()?;
        let child = command(path, true)
            .args(args)
            .env("BRIT_SWITCH_CHECKPOINT", point)
            .env("BRIT_SWITCH_CHECKPOINT_DIR", markers.path())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut paused = Paused { child, markers };
        let deadline = Instant::now() + Duration::from_secs(30);
        while !paused.markers.path().join(format!("{point}.reached")).exists() {
            ensure!(
                paused.child.try_wait()?.is_none(),
                "pull exited before checkpoint {point}"
            );
            ensure!(Instant::now() < deadline, "pull did not reach checkpoint {point}");
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(paused)
    }
    fn interrupt(paused: &mut Paused, signal: &str) -> Result<()> {
        ensure!(Command::new("kill")
            .args([signal, &paused.child.id().to_string()])
            .status()?
            .success());
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = paused.child.try_wait()? {
                ensure!(!status.success());
                return Ok(());
            }
            ensure!(Instant::now() < deadline, "interrupted pull did not terminate");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn interrupted(f: &Fixture, point: &str) -> Result<()> {
        let mut child = pause(f.local.path(), &["pull", "--ff-only", "origin", "main"], point)?;
        interrupt(&mut child, "-KILL")
    }
    #[test]
    fn real_signals_across_fetch_and_integration_require_explicit_idempotent_recovery() -> Result<()> {
        for signal in ["-INT", "-KILL"] {
            for point in [
                "fetch-pinned",
                "fetch-received",
                "objects-durable",
                "fetch-decision",
                "fetch-head-published",
                "fetch-ref-published",
                "fetch-completed",
                "journal-prepared",
                "file-published",
                "commit-decision",
                "index-published",
                "orig-head-published",
                "branch-published",
                "reflog-published",
                "branch-reflog-published",
                "hook-started",
                "cleanup",
            ] {
                let f = fixture()?;
                hook(f.local.path(), "#!/bin/sh\necho called >> hook-count\n")?;
                let mut child = pause(f.local.path(), &["pull", "--ff-only", "origin", "main"], point)?;
                interrupt(&mut child, signal)?;
                success(f.local.path(), &["pull", "--status"])?;
                ensure!(
                    !pull(f.local.path(), true)?.status.success(),
                    "new pull never resumes {point}"
                );
                success(f.local.path(), &["pull", "--continue"])?;
                assert_eq!(f.local.head_id()?, f.target, "continue {point}");
                fetched(&f)?;
                assert_eq!(fs::read(f.local.path().join("selected"))?, b"destination\n");
                let logs = fs::read(f.local.path().join(".git/logs/HEAD"))?;
                let branch = fs::read(f.local.path().join(".git/logs/refs/heads/main"))?;
                let hooks = fs::read(f.local.path().join("hook-count")).ok();
                let _ = run(f.local.path(), true, &["pull", "--continue"])?;
                assert_eq!(fs::read(f.local.path().join(".git/logs/HEAD"))?, logs);
                assert_eq!(fs::read(f.local.path().join(".git/logs/refs/heads/main"))?, branch);
                assert_eq!(fs::read(f.local.path().join("hook-count")).ok(), hooks);
                ensure!(
                    hooks.map_or(0, |h| h.iter().filter(|b| **b == b'\n').count()) <= 1,
                    "no hook replay"
                );
                ensure!(!f.local.path().join(".git/brit-switch").exists());
            }
        }
        Ok(())
    }
    #[test]
    fn abort_is_restartable_and_keeps_completed_fetch() -> Result<()> {
        for point in [
            "fetch-decision",
            "fetch-head-published",
            "fetch-completed",
            "journal-prepared",
            "file-published",
        ] {
            let f = fixture()?;
            let before = index(f.local.path())?;
            interrupted(&f, point)?;
            if point == "file-published" {
                let mut abort = pause(f.local.path(), &["pull", "--abort"], "abort-file")?;
                interrupt(&mut abort, "-KILL")?;
            }
            success(f.local.path(), &["pull", "--abort"])?;
            assert_eq!(f.local.head_id()?, f.original);
            assert_eq!(index(f.local.path())?, before);
            assert_eq!(fs::read(f.local.path().join("selected"))?, b"original\n");
            ensure!(!f.local.path().join("added").exists());
            fetched(&f)?;
        }
        Ok(())
    }
    #[test]
    fn decision_refuses_abort_and_offline_continuation_never_refetches() -> Result<()> {
        for point in ["fetch-completed", "commit-decision"] {
            let f = fixture()?;
            interrupted(&f, point)?;
            if point == "commit-decision" {
                ensure!(!run(f.local.path(), true, &["pull", "--abort"])?.status.success());
            }
            // Remove the advertised remote endpoint while retaining fixture ownership.
            let offline = f.remote.path().with_extension("offline");
            fs::rename(f.remote.path(), &offline)?;
            let out = run(f.local.path(), true, &["pull", "--continue"]);
            fs::rename(&offline, f.remote.path())?;
            assert_success(out?)?;
            assert_eq!(f.local.head_id()?, f.target);
        }
        Ok(())
    }
    #[test]
    fn foreign_files_indexes_refs_and_locks_refuse_without_overwriting_evidence() -> Result<()> {
        for kind in ["file", "index", "branch", "lock"] {
            let f = fixture()?;
            interrupted(&f, "journal-prepared")?;
            match kind {
                "file" => fs::write(f.local.path().join("selected"), "foreign\n")?,
                "index" => fs::write(f.local.path().join(".git/index"), "foreign index\n")?,
                "branch" => {
                    fs::write(f.local.path().join(".git/refs/heads/main"), format!("{}\n", f.target))?;
                }
                "lock" => {
                    let path = f.local.path().join(".git/index.lock");
                    fs::remove_file(&path)?;
                    fs::write(path, "foreign lock\n")?;
                }
                _ => unreachable!(),
            }
            let path = match kind {
                "file" => "selected",
                "index" => ".git/index",
                "branch" => ".git/refs/heads/main",
                _ => ".git/index.lock",
            };
            let bytes = fs::read(f.local.path().join(path))?;
            ensure!(
                !run(f.local.path(), true, &["pull", "--continue"])?.status.success(),
                "foreign {kind}"
            );
            assert_eq!(fs::read(f.local.path().join(path))?, bytes);
            ensure!(f.local.path().join(".git/brit-switch").exists());
        }
        Ok(())
    }
    #[test]
    fn pending_pull_blocks_mutations_but_keeps_inspection_and_names_recovery() -> Result<()> {
        let f = fixture()?;
        interrupted(&f, "journal-prepared")?;
        for args in [
            vec!["add", "--", "neighbor"],
            vec!["switch", "main"],
            vec!["switch", "--continue"],
            vec!["switch", "--abort"],
        ] {
            let out = run(f.local.path(), true, &args)?;
            ensure!(!out.status.success(), "blocked {args:?}");
            ensure!(
                String::from_utf8_lossy(&out.stderr).contains("pull"),
                "name owning recovery: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        success(f.local.path(), &["status"])?;
        success(f.local.path(), &["pull", "--status"])?;
        success(f.local.path(), &["switch", "--status"])?;
        success(f.local.path(), &["pull", "--abort"])?;
        Ok(())
    }
    #[test]
    fn corrupt_private_journal_is_retained_on_both_recovery_forms() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture()?;
        interrupted(&f, "journal-prepared")?;
        let journal = f.local.path().join(".git/brit-switch");
        assert_eq!(fs::metadata(&journal)?.permissions().mode() & 0o077, 0);
        let manifest = journal.join("manifest.json");
        fs::write(&manifest, "corrupt journal\n")?;
        for option in ["--continue", "--abort"] {
            ensure!(!run(f.local.path(), true, &["pull", option])?.status.success());
            assert_eq!(fs::read(&manifest)?, b"corrupt journal\n");
        }
        Ok(())
    }
    #[test]
    fn interrupted_keep_cleanup_leaves_committed_operation_without_active_journal() -> Result<()> {
        let f = fixture()?;
        interrupted(&f, "keeps-cleaned")?;
        assert_eq!(f.local.head_id()?, f.target);
        assert_eq!(fs::read(f.local.path().join("selected"))?, b"destination\n");
        ensure!(!f.local.path().join(".git/brit-switch").exists());
        let log = fs::read(f.local.path().join(".git/logs/HEAD"))?;
        success(f.local.path(), &["pull", "--status"])?;
        let _ = run(f.local.path(), true, &["pull", "--continue"])?;
        assert_eq!(fs::read(f.local.path().join(".git/logs/HEAD"))?, log);
        Ok(())
    }

    #[test]
    fn foreign_completed_fetch_metadata_refuses_continue_and_abort() -> Result<()> {
        for name in ["FETCH_HEAD", "refs/remotes/origin/main"] {
            for action in ["--continue", "--abort"] {
                let f = fixture()?;
                interrupted(&f, "fetch-completed")?;
                let path = f.local.path().join(".git").join(name);
                let foreign = if name == "FETCH_HEAD" {
                    b"foreign fetch receipt\n".to_vec()
                } else {
                    format!("{}\n", f.original).into_bytes()
                };
                fs::write(&path, &foreign)?;
                let out = run(f.local.path(), true, &["pull", action])?;
                ensure!(!out.status.success(), "foreign {name} must refuse {action}");
                assert_eq!(fs::read(&path)?, foreign);
                assert_eq!(f.local.head_id()?, f.original);
                ensure!(f.local.path().join(".git/brit-switch/manifest.json").exists());
            }
        }
        Ok(())
    }

    #[test]
    fn fetch_mapping_drift_refuses_recovery_and_retains_journal() -> Result<()> {
        let f = fixture()?;
        git(f.local.path(), &["config", "--unset-all", "remote.origin.fetch"])?;
        interrupted(&f, "fetch-pinned")?;
        git(
            f.local.path(),
            &["config", "remote.origin.fetch", "+refs/heads/*:refs/remotes/other/*"],
        )?;
        let out = run(f.local.path(), true, &["pull", "--continue"])?;
        ensure!(!out.status.success());
        ensure!(
            !String::from_utf8_lossy(&out.stderr).contains("panicked"),
            "configuration drift is a recovery conflict"
        );
        assert_eq!(f.local.head_id()?, f.original);
        ensure!(f.local.path().join(".git/brit-switch/manifest.json").exists());
        Ok(())
    }

    #[test]
    fn interrupted_early_abort_continuation_never_turns_into_integration() -> Result<()> {
        for point in ["fetch-pinned", "fetch-completed"] {
            let f = fixture()?;
            let before = index(f.local.path())?;
            let old_fetch = fs::read(f.local.path().join(".git/FETCH_HEAD"))?;
            interrupted(&f, point)?;
            let mut abort = pause(f.local.path(), &["pull", "--abort"], "cleanup")?;
            interrupt(&mut abort, "-KILL")?;
            success(f.local.path(), &["pull", "--continue"])?;
            assert_eq!(f.local.head_id()?, f.original);
            assert_eq!(index(f.local.path())?, before);
            assert_eq!(fs::read(f.local.path().join("selected"))?, b"original\n");
            if point == "fetch-completed" {
                fetched(&f)?;
            } else {
                assert_eq!(fs::read(f.local.path().join(".git/FETCH_HEAD"))?, old_fetch);
            }
        }
        Ok(())
    }

    #[test]
    fn version_one_switch_journal_remains_recoverable_without_discriminator() -> Result<()> {
        use std::io::Write;
        for action in ["--continue", "--abort"] {
            let f = fixture()?;
            git(f.local.path(), &["fetch", "origin"])?;
            git(f.local.path(), &["branch", "destination", "origin/main"])?;
            let before = index(f.local.path())?;
            let mut child = pause(f.local.path(), &["switch", "destination"], "journal-prepared")?;
            interrupt(&mut child, "-KILL")?;
            let path = f.local.path().join(".git/brit-switch/manifest.json");
            let mut envelope: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
            let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
            let mut manifest: serde_json::Value = serde_json::from_slice(&payload)?;
            manifest.as_object_mut().expect("manifest object").remove("pull");
            manifest["version"] = serde_json::json!(1);
            let payload = serde_json::to_vec(&manifest)?;
            let mut hash = Command::new("sha1sum")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;
            hash.stdin.take().expect("piped SHA1 input").write_all(&payload)?;
            let digest = hash.wait_with_output()?;
            ensure!(digest.status.success());
            let digest = String::from_utf8(digest.stdout)?;
            envelope["payload"] = serde_json::to_value(payload)?;
            envelope["checksum"] = serde_json::json!(digest.split_whitespace().next().expect("SHA1 digest"));
            fs::write(&path, serde_json::to_vec(&envelope)?)?;
            success(f.local.path(), &["switch", action])?;
            assert_eq!(
                f.local.head_id()?,
                if action == "--continue" {
                    f.target.clone()
                } else {
                    f.original.clone()
                }
            );
            if action == "--abort" {
                assert_eq!(index(f.local.path())?, before);
            }
            ensure!(!path.exists());
        }
        Ok(())
    }

    #[test]
    fn killed_running_hook_preserves_its_edits_and_is_never_replayed() -> Result<()> {
        struct Release(std::path::PathBuf);
        impl Drop for Release {
            fn drop(&mut self) {
                let _ = fs::write(&self.0, b"release\n");
            }
        }
        let f = fixture()?;
        let _release = Release(f.local.path().join("hook-release"));
        hook(f.local.path(), "#!/bin/sh\necho called >> hook-count\necho hook-edit > selected\necho ready > hook-ready\nwhile test ! -f hook-release; do sleep 0.01; done\n")?;
        let child = command(f.local.path(), true)
            .args(["pull", "--ff-only", "origin", "main"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let mut running = Paused {
            child,
            markers: tempfile::tempdir()?,
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        while !f.local.path().join("hook-ready").exists() {
            ensure!(running.child.try_wait()?.is_none(), "pull exited before hook");
            ensure!(Instant::now() < deadline, "hook never started");
            std::thread::sleep(Duration::from_millis(5));
        }
        interrupt(&mut running, "-KILL")?;
        fs::write(f.local.path().join("hook-release"), "release\n")?;
        let status = success(f.local.path(), &["pull", "--status"])?;
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr)
        );
        ensure!(diagnostic.contains("unknown"), "truthful hook outcome: {diagnostic}");
        success(f.local.path(), &["pull", "--continue"])?;
        assert_eq!(f.local.head_id()?, f.target);
        assert_eq!(fs::read(f.local.path().join("hook-count"))?, b"called\n");
        assert_eq!(fs::read(f.local.path().join("selected"))?, b"hook-edit\n");
        Ok(())
    }
}

#[test]
fn unrelated_histories_and_ambiguous_mappings_refuse_safely() -> Result<()> {
    for kind in ["unrelated", "ambiguous"] {
        let f = fixture()?;
        if kind == "unrelated" {
            git(f.remote.path(), &["checkout", "--orphan", "replacement"])?;
            git(f.remote.path(), &["commit", "-qm", "unrelated history"])?;
            git(f.remote.path(), &["branch", "-f", "main", "HEAD"])?;
        } else {
            git(
                f.local.path(),
                &[
                    "config",
                    "--add",
                    "remote.origin.fetch",
                    "+refs/heads/main:refs/remotes/second/main",
                ],
            )?;
        }
        let before = index(f.local.path())?;
        ensure!(!pull(f.local.path(), true)?.status.success(), "refuse {kind}");
        assert_eq!(index(f.local.path())?, before);
        assert_eq!(f.local.head_id()?, f.original);
        assert_eq!(fs::read(f.local.path().join("selected"))?, b"original\n");
    }
    Ok(())
}

#[test]
fn another_worktree_sharing_current_branch_blocks_integration() -> Result<()> {
    let f = fixture()?;
    let linked = tempfile::tempdir()?;
    git(
        f.local.path(),
        &[
            "worktree",
            "add",
            "--force",
            linked.path().to_str().expect("fixture UTF-8"),
            "main",
        ],
    )?;
    let before = index(f.local.path())?;
    ensure!(!pull(f.local.path(), true)?.status.success());
    assert_eq!(index(f.local.path())?, before);
    assert_eq!(f.local.head_id()?, f.original);
    assert_eq!(fs::read(linked.path().join("selected"))?, b"original\n");
    Ok(())
}

#[test]
fn shallow_remote_refuses_without_installing_local_shallow_state() -> Result<()> {
    let f = fixture()?;
    fs::write(f.remote.path().join(".git/shallow"), format!("{}\n", f.target))?;
    let before = index(f.local.path())?;
    let fetch = fs::read(f.local.path().join(".git/FETCH_HEAD"))?;
    let tracking = git(f.local.path(), &["rev-parse", "refs/remotes/origin/main"])?;
    let out = pull(f.local.path(), true)?;
    ensure!(!out.status.success(), "remote shallow history must refuse");
    ensure!(
        String::from_utf8_lossy(&out.stderr).contains("shallow"),
        "diagnostic: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    ensure!(
        !f.local.path().join(".git/shallow").exists(),
        "refusal never installs shallow state"
    );
    assert_eq!(index(f.local.path())?, before);
    assert_eq!(f.local.head_id()?, f.original);
    assert_eq!(fs::read(f.local.path().join(".git/FETCH_HEAD"))?, fetch);
    assert_eq!(
        git(f.local.path(), &["rev-parse", "refs/remotes/origin/main"])?,
        tracking
    );
    assert_eq!(fs::read(f.local.path().join("selected"))?, b"original\n");
    Ok(())
}

#[test]
fn metadata_ancestor_symlinks_refuse_without_touching_outside_resources() -> Result<()> {
    use std::{collections::BTreeMap, os::unix::fs::symlink, path::PathBuf};
    fn bytes(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let relative = entry.path().strip_prefix(root)?.to_owned();
            if entry.file_type()?.is_dir() {
                entries.insert(relative, None);
                bytes(root, &entry.path(), entries)?;
            } else {
                entries.insert(relative, Some(fs::read(entry.path())?));
            }
        }
        Ok(())
    }
    for ancestor in ["refs/remotes/origin", "refs/remotes", "logs/refs/heads", "logs"] {
        let f = fixture()?;
        let outside = tempfile::tempdir()?;
        let moved = outside.path().join("metadata");
        let ancestor_path = f.local.path().join(".git").join(ancestor);
        fs::rename(&ancestor_path, &moved)?;
        fs::write(moved.join("outside-sentinel"), "must remain private\n")?;
        symlink(&moved, &ancestor_path)?;
        let mut external_before = BTreeMap::new();
        bytes(&moved, &moved, &mut external_before)?;
        let before = index(f.local.path())?;
        let fetch = fs::read(f.local.path().join(".git/FETCH_HEAD"))?;
        let out = pull(f.local.path(), true)?;
        ensure!(!out.status.success(), "metadata symlink {ancestor} must refuse");
        let mut external_after = BTreeMap::new();
        bytes(&moved, &moved, &mut external_after)?;
        assert_eq!(
            external_after, external_before,
            "outside bytes and names untouched for {ancestor}"
        );
        assert_eq!(fs::read_link(&ancestor_path)?, moved);
        assert_eq!(index(f.local.path())?, before);
        assert_eq!(f.local.head_id()?, f.original);
        assert_eq!(fs::read(f.local.path().join(".git/FETCH_HEAD"))?, fetch);
    }
    Ok(())
}

#[test]
fn advertised_branch_pointing_at_blob_refuses_integration() -> Result<()> {
    let f = fixture()?;
    let blob = git(f.remote.path(), &["rev-parse", "HEAD:added"])?;
    // Deliberately malformed external repository: Git update-ref itself prevents
    // branch refs to noncommits, so write only this disposable remote's loose ref.
    fs::write(f.remote.path().join(".git/refs/heads/main"), &blob)?;
    let before = index(f.local.path())?;
    let fetch = fs::read(f.local.path().join(".git/FETCH_HEAD"))?;
    ensure!(!pull(f.local.path(), true)?.status.success());
    assert_eq!(f.local.head_id()?, f.original);
    assert_eq!(index(f.local.path())?, before);
    assert_eq!(fs::read(f.local.path().join(".git/FETCH_HEAD"))?, fetch);
    assert_eq!(fs::read(f.local.path().join("selected"))?, b"original\n");
    Ok(())
}

#[test]
fn corrupt_existing_loose_object_refuses_no_pack_pull_without_integration() -> Result<()> {
    use std::io::Write;
    let f = fixture()?;
    git(f.local.path(), &["fetch", "origin"])?;
    let target_blob = String::from_utf8(git(f.local.path(), &["rev-parse", "origin/main:selected"])?)?;
    let mut writer = command(f.local.path(), false)
        .args(["hash-object", "-w", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    writer
        .stdin
        .take()
        .expect("piped object input")
        .write_all(b"different valid blob bytes\n")?;
    let output = writer.wait_with_output()?;
    ensure!(output.status.success(), "create valid source blob");
    let wrong_blob = String::from_utf8(output.stdout)?;
    let loose = |id: &str| f.local.path().join(".git/objects").join(&id[..2]).join(&id[2..]);
    let destination = loose(target_blob.trim());
    fs::create_dir_all(destination.parent().expect("loose object fanout"))?;
    // Valid compressed Git object bytes under the wrong object name: decoding
    // alone succeeds, but verification against the requested OID must refuse.
    let replacement = destination.with_extension("replacement");
    fs::copy(loose(wrong_blob.trim()), &replacement)?;
    fs::rename(&replacement, &destination)?;
    let before = index(f.local.path())?;
    let fetch = fs::read(f.local.path().join(".git/FETCH_HEAD"))?;
    let head_log = fs::read(f.local.path().join(".git/logs/HEAD"))?;
    let out = pull(f.local.path(), true)?;
    ensure!(!out.status.success(), "incorrect object content hash must refuse");
    assert_eq!(f.local.head_id()?, f.original);
    assert_eq!(index(f.local.path())?, before);
    assert_eq!(fs::read(f.local.path().join("selected"))?, b"original\n");
    assert_eq!(fs::read(f.local.path().join(".git/FETCH_HEAD"))?, fetch);
    assert_eq!(fs::read(f.local.path().join(".git/logs/HEAD"))?, head_log);
    ensure!(
        f.local.path().join(".git/brit-switch/manifest.json").exists(),
        "retain recovery evidence"
    );
    success(f.local.path(), &["pull", "--abort"])?;
    assert_eq!(f.local.head_id()?, f.original);
    assert_eq!(index(f.local.path())?, before);
    Ok(())
}
